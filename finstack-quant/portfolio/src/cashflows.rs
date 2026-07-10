//! Portfolio-level cashflow aggregation.
//!
//! This module provides utilities to build a **cashflow ladder** across all
//! positions in a portfolio. Cashflows are aggregated by payment date and
//! currency using signed canonical schedules from the underlying instruments.
//!
//! The aggregation is **currency-preserving**: no implicit FX conversion is
//! applied. Consumers can apply explicit FX policies on top if a base-currency
//! ladder is required. Use
//! `PortfolioCashflows::collapse_to_base_by_date_kind` for a
//! base-currency projection that preserves `CFKind` classification.
//!
//! Spot-equivalent FX is used for every cashflow date in base-currency
//! projections, which is **not** the same as discounting future
//! foreign-currency cashflows at the appropriate forward FX rate. For
//! NPV-grade accuracy, derive forward FX rates from the relevant discount
//! curves instead.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use finstack_quant_cashflows::builder::{CashFlowSchedule, CashflowRepresentation};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use indexmap::IndexMap;
use std::collections::HashSet;

/// Add a number of whole years to a date, clamping to the end of the month
/// when the calendar day does not exist in the target year (e.g.
/// `Feb 29 + 1Y → Feb 28`).
///
/// Returns `None` if the resulting year overflows `i32` or is otherwise not
/// representable as a `time::Date` even after day-clamping. This lets callers
/// disable any far-date-dependent behaviour (e.g. FX warnings) rather than
/// silently looping or panicking on pathological inputs.
fn add_years_clamped(date: Date, years: i32) -> Option<Date> {
    let target_year = date.year().checked_add(years)?;
    let month = date.month();
    let mut day = date.day();
    while day > 0 {
        if let Ok(result) = Date::from_calendar_date(target_year, month, day) {
            return Some(result);
        }
        day -= 1;
    }
    None
}

/// Default threshold (in years) past which spot-equivalent FX is flagged as
/// economically unjustifiable for cashflow conversion. Override at call
/// sites that have a different mandate (e.g. ALM books with 50-year
/// liabilities) by using the internal horizon-aware helper.
const DEFAULT_FAR_FUTURE_FX_HORIZON_YEARS: i32 = 30;

/// Decide whether to warn about spot-equivalent FX being used beyond the
/// caller's valuation horizon, using the default 30-year mandate.
///
/// `as_of` is the analytical "today" of the run (typically the portfolio's
/// valuation date). Payments beyond `as_of + 30Y` are flagged because
/// spot-equivalent FX becomes economically unjustifiable at those tenors and
/// callers should derive forward FX from the relevant discount curves
/// instead.
fn should_warn_far_future_fx_conversion(
    as_of: Date,
    payment_date: Date,
    from_ccy: Currency,
    base_ccy: Currency,
) -> bool {
    should_warn_far_future_fx_conversion_with_horizon(
        as_of,
        payment_date,
        from_ccy,
        base_ccy,
        DEFAULT_FAR_FUTURE_FX_HORIZON_YEARS,
    )
}

/// Like [`should_warn_far_future_fx_conversion`] but with a caller-supplied
/// horizon. Use this for ALM / LDI books that legitimately price cashflows
/// further out than the 30-year default mandate.
///
/// Returns `false` (no warning) when:
/// - the source and base currencies match, so no FX is needed; or
/// - `as_of + horizon_years` overflows the supported date range, in which
///   case no useful threshold can be computed (callers see no warning
///   rather than a panic).
fn should_warn_far_future_fx_conversion_with_horizon(
    as_of: Date,
    payment_date: Date,
    from_ccy: Currency,
    base_ccy: Currency,
    horizon_years: i32,
) -> bool {
    if from_ccy == base_ccy {
        return false;
    }
    let Some(threshold) = add_years_clamped(as_of, horizon_years) else {
        return false;
    };
    payment_date > threshold
}

/// Why a position did not contribute classified cashflows to a portfolio ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CashflowExtractionIssueKind {
    /// The instrument exposes `CashflowProvider`, but schedule construction failed.
    BuildFailed,
}

/// Structured issue captured while extracting full cashflow schedules.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CashflowExtractionIssue {
    /// Position whose cashflow extraction was attempted.
    pub position_id: PositionId,
    /// Underlying instrument identifier.
    pub instrument_id: String,
    /// Underlying instrument type key.
    pub instrument_type: String,
    /// Failure category.
    pub kind: CashflowExtractionIssueKind,
    /// Human-readable failure detail.
    pub message: String,
}

/// Per-position cashflow summary, including empty-schedule intent metadata.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PortfolioCashflowPositionSummary {
    /// Position identifier.
    pub position_id: PositionId,
    /// Underlying instrument identifier.
    pub instrument_id: String,
    /// Underlying instrument type key.
    pub instrument_type: String,
    /// Schedule representation carried by the instrument.
    pub representation: CashflowRepresentation,
    /// Number of emitted dated events after schedule construction.
    pub event_count: usize,
}

/// One scaled portfolio cashflow event derived from an instrument schedule.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PortfolioCashflowEvent {
    /// Position contributing the event.
    pub position_id: PositionId,
    /// Underlying instrument identifier.
    pub instrument_id: String,
    /// Underlying instrument type key.
    pub instrument_type: String,
    /// Payment date.
    pub date: Date,
    /// Position-scaled amount.
    pub amount: Money,
    /// Cashflow classification preserved from the instrument schedule.
    pub kind: CFKind,
    /// Optional reset date for floating coupons.
    pub reset_date: Option<Date>,
    /// Accrual factor used to compute the event when available.
    pub accrual_factor: f64,
    /// Effective rate used to compute the event when available.
    pub rate: Option<f64>,
}

/// Rich portfolio cashflow ladder preserving event classifications.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PortfolioCashflows {
    /// Scaled cashflow events for all supported positions, sorted by payment date.
    pub events: Vec<PortfolioCashflowEvent>,

    /// Per-position event drill-down keyed by position ID.
    pub by_position: IndexMap<PositionId, Vec<PortfolioCashflowEvent>>,

    /// Aggregated totals by date, currency, and `CFKind`.
    pub by_date: IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>>,

    /// Per-position schedule metadata, including placeholder/no-residual intent.
    pub position_summaries: IndexMap<PositionId, PortfolioCashflowPositionSummary>,

    /// Extraction issues for unsupported instruments and provider failures.
    pub issues: Vec<CashflowExtractionIssue>,
}

/// Build the canonical signed schedule for a single instrument.
fn instrument_cashflow_schedule(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
) -> std::result::Result<CashFlowSchedule, finstack_quant_core::Error> {
    instrument.cashflow_schedule(market, as_of)
}

impl PortfolioCashflows {
    /// Collapse classified multi-currency flows into base currency bucketed by
    /// (date, `CFKind`).
    ///
    /// ### FX convention
    ///
    /// Each foreign-currency flow on payment date `T` is converted to
    /// `base_ccy` using whatever rate the `FxMatrix` resolves for
    /// `(from → base_ccy, T)`. For most market setups this will be a
    /// spot-equivalent rate rather than a true forward FX rate derived from
    /// discount curves; the module-level docstring explains the trade-off.
    /// For NPV-grade accuracy, convert via forward FX on the calling side and
    /// pass already-base-currency flows.
    ///
    /// ### `as_of`
    ///
    /// `as_of` is the valuation / reporting date of the caller. It is used
    /// solely to gate a warning when converting flows beyond `as_of + 30Y`,
    /// where spot-equivalent FX is no longer defensible. It does **not**
    /// select a curve or alter the numerical result.
    ///
    /// # Errors
    ///
    /// Returns an error when FX conversion or monetary aggregation fails.
    pub fn collapse_to_base_by_date_kind(
        &self,
        market: &MarketContext,
        base_ccy: Currency,
        as_of: Date,
    ) -> Result<IndexMap<Date, IndexMap<CFKind, Money>>> {
        let mut by_date_base: IndexMap<Date, IndexMap<CFKind, Money>> = IndexMap::new();
        let mut warned_pairs: HashSet<(Currency, Currency, Date)> = HashSet::new();

        for (date, per_ccy) in &self.by_date {
            let mut per_kind_base: IndexMap<CFKind, Money> = IndexMap::new();

            for per_kind in per_ccy.values() {
                for (kind, money) in per_kind {
                    let converted = convert_money_to_base_on_date(
                        *money,
                        *date,
                        market,
                        base_ccy,
                        as_of,
                        &mut warned_pairs,
                    )?;
                    let entry = per_kind_base
                        .entry(*kind)
                        .or_insert_with(|| Money::new(0.0, base_ccy));
                    *entry = entry.checked_add(converted).map_err(Error::Core)?;
                }
            }

            if !per_kind_base.is_empty() {
                by_date_base.insert(*date, per_kind_base);
            }
        }

        Ok(by_date_base)
    }
}

/// Portfolios below this position count run cashflow scheduling serially:
/// Rayon's fan-out overhead exceeds the per-position scheduling cost for
/// small books. Mirrors `VALUE_PORTFOLIO_PARALLEL_MIN_POSITIONS` in
/// `valuation.rs` so the crate uses one consistent parallel cutover.
const AGGREGATE_CASHFLOWS_PARALLEL_MIN_POSITIONS: usize = 64;

/// Aggregate full portfolio cashflows while preserving `CFKind` classification.
pub fn aggregate_full_cashflows(
    portfolio: &Portfolio,
    market: &MarketContext,
) -> Result<PortfolioCashflows> {
    // Phase A: build per-position cashflow schedules. Each call to
    // `instrument_cashflow_schedule` is an independent, read-only function of
    // the shared `MarketContext` and the per-position instrument, so scheduling
    // it in parallel yields near-linear speedup for portfolios with many
    // instruments. Small books stay serial to dodge Rayon overhead. Results
    // are collected in positional order either way, preserving the
    // deterministic event/merge ordering that the existing tests encode.
    struct PositionCashflowResult {
        position_id: PositionId,
        instrument_id: String,
        instrument_type: String,
        schedule: std::result::Result<CashFlowSchedule, finstack_quant_core::Error>,
        scaled_flows: Vec<(finstack_quant_core::cashflow::CashFlow, Money)>,
    }

    use rayon::prelude::*;
    let schedule_position = |position: &crate::position::Position| -> PositionCashflowResult {
        let instrument_id = position.instrument.id().to_string();
        let instrument_type = format!("{:?}", position.instrument.key());
        match instrument_cashflow_schedule(position.instrument.as_ref(), market, portfolio.as_of) {
            Ok(schedule) => {
                let scaled_flows: Vec<_> = schedule
                    .flows
                    .iter()
                    .map(|flow| (*flow, position.scale_value(flow.amount)))
                    .collect();
                PositionCashflowResult {
                    position_id: position.position_id.clone(),
                    instrument_id,
                    instrument_type,
                    schedule: Ok(schedule),
                    scaled_flows,
                }
            }
            Err(err) => PositionCashflowResult {
                position_id: position.position_id.clone(),
                instrument_id,
                instrument_type,
                schedule: Err(err),
                scaled_flows: Vec::new(),
            },
        }
    };

    let per_position: Vec<PositionCashflowResult> =
        if portfolio.positions.len() >= AGGREGATE_CASHFLOWS_PARALLEL_MIN_POSITIONS {
            portfolio
                .positions
                .par_iter()
                .map(schedule_position)
                .collect()
        } else {
            portfolio.positions.iter().map(schedule_position).collect()
        };

    // Phase B (serial): merge per-position results into the aggregated
    // structures. Serial keeps `events` / `by_position` / `by_date` ordering
    // deterministic and preserves the existing tracing log order.
    let mut events = Vec::new();
    let mut by_position: IndexMap<PositionId, Vec<PortfolioCashflowEvent>> = IndexMap::new();
    let mut position_summaries: IndexMap<PositionId, PortfolioCashflowPositionSummary> =
        IndexMap::new();
    let mut issues = Vec::new();

    for result in per_position {
        match result.schedule {
            Ok(schedule) => {
                let event_count = schedule.flows.len();
                let representation = schedule.meta.representation;
                let mut position_events = Vec::with_capacity(schedule.flows.len());
                for (flow, scaled_amount) in result.scaled_flows {
                    let event = PortfolioCashflowEvent {
                        position_id: result.position_id.clone(),
                        instrument_id: result.instrument_id.clone(),
                        instrument_type: result.instrument_type.clone(),
                        date: flow.date,
                        amount: scaled_amount,
                        kind: flow.kind,
                        reset_date: flow.reset_date,
                        accrual_factor: flow.accrual_factor,
                        rate: flow.rate,
                    };
                    events.push(event.clone());
                    position_events.push(event);
                }
                by_position.insert(result.position_id.clone(), position_events);
                position_summaries.insert(
                    result.position_id.clone(),
                    PortfolioCashflowPositionSummary {
                        position_id: result.position_id.clone(),
                        instrument_id: result.instrument_id.clone(),
                        instrument_type: result.instrument_type.clone(),
                        representation,
                        event_count,
                    },
                );
            }
            Err(err) => {
                tracing::warn!(
                    position_id = %result.position_id,
                    instrument_id = %result.instrument_id,
                    instrument_type = %result.instrument_type,
                    error = %err,
                    "Skipping position during portfolio cashflow aggregation because contractual cashflows could not be built"
                );
                issues.push(CashflowExtractionIssue {
                    position_id: result.position_id,
                    instrument_id: result.instrument_id,
                    instrument_type: result.instrument_type,
                    kind: CashflowExtractionIssueKind::BuildFailed,
                    message: err.to_string(),
                });
            }
        }
    }

    events.sort_by_key(|event| event.date);

    let mut by_date: IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>> = IndexMap::new();
    for event in &events {
        let per_ccy = by_date.entry(event.date).or_default();
        let per_kind = per_ccy.entry(event.amount.currency()).or_default();
        let entry = per_kind
            .entry(event.kind)
            .or_insert_with(|| Money::new(0.0, event.amount.currency()));
        *entry = entry.checked_add(event.amount).map_err(Error::Core)?;
    }

    Ok(PortfolioCashflows {
        events,
        by_position,
        by_date,
        position_summaries,
        issues,
    })
}

/// Convert one dated cashflow into the requested base currency.
fn convert_money_to_base_on_date(
    money: Money,
    payment_date: Date,
    market: &MarketContext,
    base_ccy: Currency,
    as_of: Date,
    warned_pairs: &mut HashSet<(Currency, Currency, Date)>,
) -> Result<Money> {
    let ccy = money.currency();
    if ccy == base_ccy {
        return Ok(money);
    }

    // Emit the cashflow-specific far-future warning before delegating the
    // actual FX lookup/conversion to the shared `crate::fx::convert_to_base`
    // helper so the rate application and error mapping stay consistent across
    // the portfolio crate.
    if should_warn_far_future_fx_conversion(as_of, payment_date, ccy, base_ccy)
        && warned_pairs.insert((ccy, base_ccy, payment_date))
    {
        tracing::warn!(
            from = %ccy,
            to = %base_ccy,
            payment_date = %payment_date,
            "Converting cashflow beyond market as-of + 30Y using spot-equivalent FX; prefer forward FX for long-dated reporting"
        );
    }

    crate::fx::convert_to_base(money, payment_date, market, base_ccy).map_err(|e| match e {
        // Pin the offending payment date onto the FX failure: the bare
        // `FxConversionFailed` only names the currency pair, which is not
        // enough to find the missing rate when a collapse spans many dates.
        Error::FxConversionFailed { from, to } => Error::MissingMarketData(format!(
            "no FX rate for {from}/{to} at cashflow payment date {payment_date}"
        )),
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market_at;
    use crate::types::Entity;
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::types::Attributes;
    use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
    use finstack_quant_valuations::instruments::credit_derivatives::CDSIndex;
    use finstack_quant_valuations::instruments::fixed_income::bond;
    use finstack_quant_valuations::instruments::fixed_income::AgencyMbsPassthrough;
    use finstack_quant_valuations::instruments::rates::Swaption;
    use finstack_quant_valuations::instruments::Instrument as InternalInstrument;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::Arc;
    use std::sync::OnceLock;
    use time::macros::date;

    #[derive(Clone)]
    struct UnsupportedInstrument;

    impl finstack_quant_cashflows::CashflowProvider for UnsupportedInstrument {
        fn cashflow_schedule(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Err(finstack_quant_core::Error::Validation(
                "unsupported test instrument".to_string(),
            ))
        }
    }

    impl InternalInstrument for UnsupportedInstrument {
        fn id(&self) -> &str {
            "UNSUPPORTED"
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Swaption
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(0.0, Currency::USD))
        }

        fn attributes(&self) -> &Attributes {
            static ATTRS: OnceLock<Attributes> = OnceLock::new();
            ATTRS.get_or_init(Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            unreachable!("test dummy should not mutate attributes")
        }

        fn clone_box(&self) -> Box<dyn InternalInstrument> {
            Box::new(self.clone())
        }
    }

    fn market_with_eurusd_fx(as_of: Date, eurusd: f64) -> MarketContext {
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, eurusd)
            .expect("test FX quote should be valid");
        build_test_market_at(as_of).insert_fx(FxMatrix::new(provider))
    }

    fn full_cashflow_ladder_fixture() -> PortfolioCashflows {
        let mut by_date: IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>> =
            IndexMap::new();

        by_date.insert(
            date!(2025 - 03 - 15),
            IndexMap::from([
                (
                    Currency::EUR,
                    IndexMap::from([
                        (CFKind::Fixed, Money::new(100.0, Currency::EUR)),
                        (CFKind::Notional, Money::new(200.0, Currency::EUR)),
                    ]),
                ),
                (
                    Currency::USD,
                    IndexMap::from([(CFKind::Fee, Money::new(-10.0, Currency::USD))]),
                ),
            ]),
        );

        by_date.insert(
            date!(2025 - 08 - 01),
            IndexMap::from([
                (
                    Currency::USD,
                    IndexMap::from([(CFKind::Fixed, Money::new(50.0, Currency::USD))]),
                ),
                (
                    Currency::EUR,
                    IndexMap::from([(CFKind::Fee, Money::new(-5.0, Currency::EUR))]),
                ),
            ]),
        );

        by_date.insert(
            date!(2026 - 02 - 01),
            IndexMap::from([(
                Currency::EUR,
                IndexMap::from([(CFKind::Fixed, Money::new(25.0, Currency::EUR))]),
            )]),
        );

        PortfolioCashflows {
            events: Vec::new(),
            by_position: IndexMap::new(),
            by_date,
            position_summaries: IndexMap::new(),
            issues: Vec::new(),
        }
    }

    #[test]
    fn aggregate_full_cashflows_bond_ladder_has_usd_flows_and_contractual_summary() {
        let as_of = date!(2025 - 01 - 01);
        let bond = bond::Bond::fixed(
            "BOND_001",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            as_of,
            date!(2027 - 01 - 01),
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "BOND_001",
            Arc::new(bond),
            1.0,
            PositionUnit::FaceValue,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of))
            .expect("cashflow aggregation");

        assert!(!full.events.is_empty(), "expected non-empty events");
        assert!(
            full.events
                .iter()
                .any(|e| e.amount.currency() == Currency::USD),
            "expected at least one USD cashflow"
        );
        assert_eq!(full.by_position.len(), 1);
        assert!(full.by_position.contains_key("POS_001"));
        assert_eq!(full.position_summaries.len(), 1);
        assert_eq!(
            full.position_summaries["POS_001"].representation,
            CashflowRepresentation::Contractual
        );
        assert!(full.issues.is_empty(), "expected no extraction issues");
    }

    #[test]
    fn far_future_fx_conversions_are_flagged_relative_to_as_of() {
        let as_of = date!(2025 - 01 - 01);
        let payment_date = date!(2055 - 01 - 02);

        assert!(should_warn_far_future_fx_conversion(
            as_of,
            payment_date,
            Currency::EUR,
            Currency::USD
        ));

        let near = date!(2030 - 01 - 01);
        assert!(!should_warn_far_future_fx_conversion(
            as_of,
            near,
            Currency::EUR,
            Currency::USD
        ));

        assert!(!should_warn_far_future_fx_conversion(
            as_of,
            payment_date,
            Currency::USD,
            Currency::USD
        ));
    }

    #[test]
    fn aggregate_full_cashflows_surfaces_provider_failures_as_issues() {
        let as_of = date!(2025 - 01 - 01);
        let position = Position::new(
            "POS_SWAP",
            "ENTITY_A",
            "NG-SWAP-2025",
            Arc::new(CommoditySwap::example()),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("WARNINGS")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &MarketContext::new())
            .expect("aggregation should succeed with issues");

        assert!(full.events.is_empty(), "failed cashflows should be skipped");
        assert!(
            full.by_position.is_empty(),
            "failed position should not emit flows"
        );
        assert_eq!(full.issues.len(), 1, "expected one extraction issue");
        assert_eq!(full.issues[0].position_id.as_str(), "POS_SWAP");
        assert_eq!(
            full.issues[0].kind,
            CashflowExtractionIssueKind::BuildFailed
        );
        assert!(
            full.issues[0].message.contains("NG-SPOT-AVG"),
            "unexpected issue message: {}",
            full.issues[0].message
        );
    }

    #[test]
    fn aggregate_full_cashflows_preserves_empty_placeholder_position_summaries() {
        let as_of = date!(2025 - 01 - 01);
        let position = Position::new(
            "POS_SWAPTION",
            "ENTITY_A",
            "SWAPTION_001",
            Arc::new(Swaption::example()),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("PLACEHOLDER")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of))
            .expect("placeholder aggregation");

        assert!(full.events.is_empty(), "empty placeholder emits no events");
        assert!(full.by_position["POS_SWAPTION"].is_empty());
        assert_eq!(
            full.position_summaries["POS_SWAPTION"].representation,
            CashflowRepresentation::Placeholder
        );
        assert_eq!(full.position_summaries["POS_SWAPTION"].event_count, 0);
        assert!(
            full.issues.is_empty(),
            "placeholder schedules should not raise issues"
        );
    }

    #[test]
    fn aggregate_full_cashflows_includes_deferred_agency_provider() {
        let as_of = date!(2025 - 01 - 01);
        let position = Position::new(
            "POS_MBS",
            "ENTITY_A",
            "FN-MA1234",
            Arc::new(AgencyMbsPassthrough::example().expect("agency mbs example")),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("AGENCY")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of))
            .expect("agency cashflow aggregation");

        assert!(
            !full.events.is_empty(),
            "agency provider should emit flows, issues={:?}",
            full.issues
        );
        assert!(
            full.issues.is_empty(),
            "agency provider should not raise issues"
        );
    }

    #[test]
    fn aggregate_full_cashflows_includes_deferred_credit_composite_provider() {
        let as_of = date!(2025 - 01 - 01);
        let position = Position::new(
            "POS_CDX",
            "ENTITY_A",
            "CDX-IG-42",
            Arc::new(CDSIndex::example()),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("CDX")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");
        let market = build_test_market_at(as_of).insert(
            HazardCurve::builder("CDX.NA.IG.HAZARD")
                .base_date(as_of)
                .currency(Currency::USD)
                .recovery_rate(0.40)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard curve should build"),
        );

        let full = aggregate_full_cashflows(&portfolio, &market).expect("cdx cashflow aggregation");

        assert!(
            !full.events.is_empty(),
            "credit composite provider should emit flows"
        );
        assert!(
            full.issues.is_empty(),
            "credit composite provider should not raise issues"
        );
    }

    #[test]
    fn aggregate_full_cashflows_preserves_kinds_and_position_detail() {
        let as_of = date!(2025 - 01 - 01);
        let issue = as_of;
        let maturity = date!(2027 - 01 - 01);
        let bond = bond::Bond::fixed(
            "BOND_FULL",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");
        let position = Position::new(
            "POS_FULL",
            "ENTITY_A",
            "BOND_FULL",
            Arc::new(bond),
            1.0,
            PositionUnit::FaceValue,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("FULL")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of))
            .expect("full cashflow aggregation");

        assert!(
            !full.events.is_empty(),
            "expected classified cashflow events"
        );
        assert!(full.issues.is_empty(), "expected no extraction issues");
        assert_eq!(
            full.by_position.len(),
            1,
            "expected one position drill-down"
        );
        assert!(
            full.events
                .iter()
                .any(|event| matches!(event.kind, CFKind::Fixed | CFKind::Notional)),
            "expected coupon or principal classifications"
        );

        let has_kind_bucket = full.by_date.values().any(|per_ccy| {
            per_ccy.values().any(|per_kind| {
                per_kind.contains_key(&CFKind::Fixed) || per_kind.contains_key(&CFKind::Notional)
            })
        });
        assert!(
            has_kind_bucket,
            "expected date aggregation to preserve CFKind buckets"
        );
    }

    #[test]
    fn aggregate_full_cashflows_records_unsupported_instruments() {
        let as_of = date!(2025 - 01 - 01);
        let position = Position::new(
            "POS_UNSUPPORTED",
            "ENTITY_A",
            "UNSUPPORTED",
            Arc::new(UnsupportedInstrument),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let portfolio = PortfolioBuilder::new("UNSUPPORTED")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full = aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of))
            .expect("unsupported instruments should produce issues, not fail the aggregation");

        assert!(
            full.events.is_empty(),
            "unsupported instrument should not emit events"
        );
        assert_eq!(full.issues.len(), 1, "expected one unsupported issue");
        assert_eq!(
            full.issues[0].kind,
            CashflowExtractionIssueKind::BuildFailed
        );
    }

    #[test]
    fn full_cashflows_collapse_to_base_by_date_kind_preserves_cfkind() {
        let as_of = date!(2025 - 01 - 01);
        let full = full_cashflow_ladder_fixture();
        let market = market_with_eurusd_fx(as_of, 1.20);

        let by_date_kind = full
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of)
            .expect("base currency conversion by kind");

        let march = by_date_kind
            .get(&date!(2025 - 03 - 15))
            .expect("march bucket should exist");
        assert_eq!(march[&CFKind::Fixed], Money::new(120.0, Currency::USD));
        assert_eq!(march[&CFKind::Notional], Money::new(240.0, Currency::USD));
        assert_eq!(march[&CFKind::Fee], Money::new(-10.0, Currency::USD));

        let august = by_date_kind
            .get(&date!(2025 - 08 - 01))
            .expect("august bucket should exist");
        assert_eq!(august[&CFKind::Fixed], Money::new(50.0, Currency::USD));
        assert_eq!(august[&CFKind::Fee], Money::new(-6.0, Currency::USD));
    }
}
