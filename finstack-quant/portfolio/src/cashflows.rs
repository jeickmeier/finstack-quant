//! Portfolio-level cashflow aggregation.
//!
//! Builds a cashflow ladder across all positions, aggregated by payment date
//! and currency from signed canonical instrument schedules. Aggregation is
//! currency-preserving: no implicit FX conversion is applied. Use
//! `PortfolioCashflows::collapse_to_base_by_date_kind` for a base-currency
//! projection that preserves `CFKind` classification.
//!
//! Base-currency collapse uses spot FX at `as_of` for payments on or before
//! the valuation date, and the covered-interest-parity forward
//! `F(T) = S × DF_from(T) / DF_base(T)` for later dates. Missing discount
//! curves or discount factors fail closed.

use crate::error::{Error, Result};
#[cfg(not(target_arch = "wasm32"))]
use crate::evaluation::POSITION_PARALLEL_MIN_POSITIONS;
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use finstack_quant_cashflows::builder::{CashFlowSchedule, CashflowRepresentation};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::str::FromStr;

/// Options for [`aggregate_full_cashflows`].
///
/// The default is fail-closed: any schedule-construction issue aborts the
/// call. Set [`allow_partial`](Self::allow_partial) to keep a partial
/// ladder with those issues recorded on [`PortfolioCashflows::issues`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CashflowAggregationOptions {
    /// When `false` (default), a non-empty [`PortfolioCashflows::issues`]
    /// list fails the call. When `true`, remaining positions still
    /// contribute to the ladder and issues are returned on the result.
    pub allow_partial: bool,
}

/// How [`PortfolioCashflows::collapse_to_base_by_date_kind`] converts
/// foreign-currency flows into the reporting currency.
///
/// This is not [`finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate`]:
/// that policy names a spot-equivalent provider lookup on the payment date.
/// Collapse uses spot at `as_of` for due-or-past flows and CIP forwards for
/// later dates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CashflowFxPolicy {
    /// Spot FX at `as_of` when `payment_date <= as_of`; otherwise the CIP
    /// forward `F(T) = S × DF_from(T) / DF_base(T)`.
    #[default]
    CipForward,
}

/// Why a position did not contribute classified cashflows to a portfolio ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
    pub instrument_type: InstrumentType,
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
    pub instrument_type: InstrumentType,
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
    pub instrument_type: InstrumentType,
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

    /// FX policy applied by [`Self::collapse_to_base_by_date_kind`].
    ///
    /// Stamped so callers do not infer a spot-on-payment-date conversion from
    /// [`finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate`].
    #[serde(default)]
    pub fx_collapse_policy: CashflowFxPolicy,
}

impl PortfolioCashflows {
    /// Collapse classified multi-currency flows into base currency bucketed by
    /// (date, `CFKind`).
    ///
    /// ### FX convention
    ///
    /// Each foreign-currency flow on payment date `T` is converted as:
    ///
    /// - `T <= as_of`: spot FX at `as_of`
    /// - `T > as_of`: CIP forward `F(T) = S × DF_from(T) / DF_base(T)`
    ///
    /// Discount curves come from `discount_curves` when a currency is mapped;
    /// otherwise from `market.get_discount(currency.to_string())`. Missing
    /// curves or missing/zero discount factors return an error.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context supplying the FX matrix and discount curves.
    /// * `base_currency` - Reporting currency for the collapsed ladder.
    /// * `as_of` - Valuation date for spot FX and as the start of each
    ///   discount-factor interval.
    /// * `discount_curves` - Optional `Currency → CurveId` map. A missing map
    ///   or missing currency entry uses the ISO currency code as the curve id.
    ///
    /// # Errors
    ///
    /// Returns an error when FX conversion, discount-curve resolution, or
    /// monetary aggregation fails.
    pub fn collapse_to_base_by_date_kind(
        &self,
        market: &MarketContext,
        base_currency: Currency,
        as_of: Date,
        discount_curves: Option<&HashMap<Currency, CurveId>>,
    ) -> Result<IndexMap<Date, IndexMap<CFKind, Money>>> {
        let mut by_date_base: IndexMap<Date, IndexMap<CFKind, Money>> = IndexMap::new();

        for (date, per_currency) in &self.by_date {
            let mut per_kind_base: IndexMap<CFKind, Money> = IndexMap::new();

            for per_kind in per_currency.values() {
                for (kind, money) in per_kind {
                    let converted = convert_money_to_base_on_date(
                        *money,
                        *date,
                        market,
                        base_currency,
                        as_of,
                        discount_curves,
                    )?;
                    let entry = per_kind_base
                        .entry(*kind)
                        .or_insert_with(|| Money::from((0_i64, base_currency)));
                    *entry = entry.checked_add(converted).map_err(Error::Core)?;
                }
            }

            if !per_kind_base.is_empty() {
                by_date_base.insert(*date, per_kind_base);
            }
        }

        Ok(by_date_base)
    }

    /// Net same-currency cashflow amounts across kinds for each payment date.
    ///
    /// Dates with no flows in `currency` are omitted. Non-finite amounts are
    /// skipped. Totals use Neumaier compensated summation.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO currency whose kind buckets are summed at each date.
    #[must_use]
    pub fn net_in_currency_by_date(&self, currency: Currency) -> Vec<(Date, f64)> {
        net_amounts_by_date(&self.by_date, currency)
    }
}

/// Net same-currency cashflow amounts across kinds for each payment date.
///
/// # Arguments
///
/// * `by_date` - Classified totals keyed by payment date, currency, and kind.
/// * `currency` - ISO currency whose kind buckets are summed at each date.
#[must_use]
pub fn net_amounts_by_date(
    by_date: &IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>>,
    currency: Currency,
) -> Vec<(Date, f64)> {
    let mut out = Vec::new();
    for (date, per_currency) in by_date {
        let Some(per_kind) = per_currency.get(&currency) else {
            continue;
        };
        let mut acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        let mut saw_finite = false;
        for money in per_kind.values() {
            let amount = money.amount();
            if amount.is_finite() {
                acc.add(amount);
                saw_finite = true;
            }
        }
        if saw_finite {
            out.push((*date, acc.total()));
        }
    }
    out
}

/// Net same-currency cashflow amounts from a classified `by_date` JSON object.
///
/// Accepts either a full [`PortfolioCashflows`] payload or a bare `by_date`
/// map. Kind keys are opaque strings (reporting fixtures may use mixed-case
/// labels such as `"Notional"`). Amounts may be JSON numbers or decimal
/// strings.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when `cashflows_json` is not JSON, when
/// `currency` is not a known ISO code, or when the `by_date` value is not an
/// object.
///
/// # Arguments
///
/// * `cashflows_json` - Full cashflow-ladder JSON or a `{date: {ccy: {kind: money}}}`
///   object (optionally wrapped as `{"by_date": ...}`).
/// * `currency` - ISO-4217 code selecting which per-date currency bucket to net.
pub fn net_in_currency_by_date_json(
    cashflows_json: &str,
    currency: &str,
) -> Result<Vec<(String, f64)>> {
    let currency = Currency::from_str(currency).map_err(|e| Error::InvalidInput(e.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(cashflows_json)
        .map_err(|e| Error::InvalidInput(format!("invalid cashflow JSON: {e}")))?;
    let by_date = value.get("by_date").unwrap_or(&value);
    let Some(by_date_obj) = by_date.as_object() else {
        return Err(Error::InvalidInput(
            "cashflow JSON must contain a by_date object".to_string(),
        ));
    };

    let currency_code = currency.to_string();
    let mut out = Vec::new();
    for (date, per_currency) in by_date_obj {
        let Some(ccy_map) = per_currency.get(&currency_code).and_then(|v| v.as_object()) else {
            continue;
        };
        let mut acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        let mut saw_finite = false;
        for kind_money in ccy_map.values() {
            if let Some(amount) = json_money_amount(kind_money) {
                if amount.is_finite() {
                    acc.add(amount);
                    saw_finite = true;
                }
            }
        }
        if saw_finite {
            out.push((date.clone(), acc.total()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn json_money_amount(value: &serde_json::Value) -> Option<f64> {
    let amount = value.get("amount")?;
    match amount {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Aggregate contractual portfolio cashflows while preserving `CFKind` classification.
///
/// Successful positions contribute scaled events, deterministic date/currency/
/// kind aggregates, and per-position summaries. A position whose instrument
/// cannot build a contractual schedule is recorded as a `BuildFailed` issue.
/// By default those issues fail the call; pass
/// [`CashflowAggregationOptions::allow_partial`] to keep the partial ladder.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when `options.allow_partial` is `false`
/// (the default) and at least one position failed schedule construction.
/// Also returns an error if successful same-date, same-currency, same-kind
/// amounts cannot be added (for example, a monetary overflow).
///
/// # Arguments
///
/// * `portfolio` - Portfolio whose position quantities scale contractual
///   instrument cashflows and whose `as_of` date anchors schedule generation.
/// * `market` - Market data used by instruments that require it to construct
///   contractual schedules, such as floating-rate or indexed cashflows.
/// * `options` - Fail-closed vs partial-ladder policy. Default
///   [`CashflowAggregationOptions::default`] rejects a non-empty `issues`
///   list; `allow_partial` keeps those issues on the result.
pub fn aggregate_full_cashflows(
    portfolio: &Portfolio,
    market: &MarketContext,
    options: &CashflowAggregationOptions,
) -> Result<PortfolioCashflows> {
    struct PositionCashflowResult {
        position_id: PositionId,
        instrument_id: String,
        instrument_type: InstrumentType,
        schedule: std::result::Result<CashFlowSchedule, finstack_quant_core::Error>,
        scaled_flows: Vec<(finstack_quant_core::cashflow::CashFlow, Money)>,
    }

    let schedule_position = |position: &crate::position::Position| -> PositionCashflowResult {
        let instrument_id = position.instrument.id().to_string();
        let instrument_type = position.instrument.key();
        match position
            .instrument
            .as_ref()
            .cashflow_schedule(market, portfolio.as_of)
        {
            Ok(schedule) => {
                let scaled_flows = schedule
                    .get_flows()
                    .iter()
                    .map(|flow| {
                        position
                            .scale_value(flow.amount)
                            .map(|amount| (flow.clone(), amount))
                    })
                    .collect::<finstack_quant_core::Result<Vec<_>>>();
                let (schedule, scaled_flows) = match scaled_flows {
                    Ok(flows) => (Ok(schedule), flows),
                    Err(error) => (Err(error), Vec::new()),
                };
                PositionCashflowResult {
                    position_id: position.position_id.clone(),
                    instrument_id,
                    instrument_type,
                    schedule,
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

    #[cfg(not(target_arch = "wasm32"))]
    let per_position: Vec<PositionCashflowResult> =
        if portfolio.positions.len() >= POSITION_PARALLEL_MIN_POSITIONS {
            use rayon::prelude::*;
            portfolio
                .positions
                .par_iter()
                .map(schedule_position)
                .collect()
        } else {
            portfolio.positions.iter().map(schedule_position).collect()
        };

    #[cfg(target_arch = "wasm32")]
    let per_position: Vec<PositionCashflowResult> =
        portfolio.positions.iter().map(schedule_position).collect();

    let mut events = Vec::new();
    let mut by_position: IndexMap<PositionId, Vec<PortfolioCashflowEvent>> = IndexMap::new();
    let mut position_summaries: IndexMap<PositionId, PortfolioCashflowPositionSummary> =
        IndexMap::new();
    let mut issues = Vec::new();

    for result in per_position {
        match result.schedule {
            Ok(schedule) => {
                let event_count = schedule.get_flows().len();
                let representation = schedule.get_meta().representation;
                let mut position_events = Vec::with_capacity(event_count);
                for (flow, scaled_amount) in result.scaled_flows {
                    position_events.push(PortfolioCashflowEvent {
                        position_id: result.position_id.clone(),
                        instrument_id: result.instrument_id.clone(),
                        instrument_type: result.instrument_type,
                        date: flow.date,
                        amount: scaled_amount,
                        kind: flow.kind,
                        reset_date: flow.reset_date,
                        accrual_factor: flow.accrual_factor,
                        rate: flow.rate,
                    });
                }
                events.extend(position_events.iter().cloned());
                by_position.insert(result.position_id.clone(), position_events);
                position_summaries.insert(
                    result.position_id.clone(),
                    PortfolioCashflowPositionSummary {
                        position_id: result.position_id.clone(),
                        instrument_id: result.instrument_id.clone(),
                        instrument_type: result.instrument_type,
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

    if !options.allow_partial && !issues.is_empty() {
        let detail = issues
            .iter()
            .map(|issue| format!("{}: {}", issue.position_id, issue.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Error::invalid_input(format!(
            "cashflow extraction failed for {} position(s); \
             set CashflowAggregationOptions.allow_partial to keep a partial ladder: {detail}",
            issues.len()
        )));
    }

    events.sort_by_key(|event| event.date);

    let mut by_date: IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>> = IndexMap::new();
    for event in &events {
        let per_currency = by_date.entry(event.date).or_default();
        let per_kind = per_currency.entry(event.amount.currency()).or_default();
        let entry = per_kind
            .entry(event.kind)
            .or_insert_with(|| Money::from((0_i64, event.amount.currency())));
        *entry = entry.checked_add(event.amount).map_err(Error::Core)?;
    }

    Ok(PortfolioCashflows {
        events,
        by_position,
        by_date,
        position_summaries,
        issues,
        fx_collapse_policy: CashflowFxPolicy::CipForward,
    })
}

/// Convert one dated cashflow into the requested base currency.
fn convert_money_to_base_on_date(
    money: Money,
    payment_date: Date,
    market: &MarketContext,
    base_currency: Currency,
    as_of: Date,
    discount_curves: Option<&HashMap<Currency, CurveId>>,
) -> Result<Money> {
    crate::fx::convert_to_base_forward(
        money,
        as_of,
        payment_date,
        market,
        base_currency,
        discount_curves,
    )
    .map_err(|e| match e {
        // Pin the valuation date onto the FX failure: CIP and spot both look
        // up the matrix at `as_of`, not at the payment date.
        Error::FxConversionFailed { from, to } => Error::MissingMarketData(format!(
            "no FX rate for {from}/{to} at cashflow as-of {as_of}"
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
    use finstack_quant_core::market_data::term_structures::{
        DiscountCurve, HazardCurve, ValidationMode,
    };
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::types::{Attributes, CurveId};
    use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
    use finstack_quant_valuations::instruments::credit_derivatives::CDSIndex;
    use finstack_quant_valuations::instruments::fixed_income::bond;
    use finstack_quant_valuations::instruments::fixed_income::AgencyMbsPassthrough;
    use finstack_quant_valuations::instruments::rates::Swaption;
    use finstack_quant_valuations::instruments::Instrument as InternalInstrument;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::OnceLock;
    use time::macros::date;

    fn fail_closed() -> CashflowAggregationOptions {
        CashflowAggregationOptions::default()
    }

    fn allow_partial() -> CashflowAggregationOptions {
        CashflowAggregationOptions {
            allow_partial: true,
        }
    }

    #[derive(Clone)]
    struct UnsupportedInstrument;

    impl finstack_quant_cashflows::CashflowScheduleSource for UnsupportedInstrument {
        fn raw_cashflow_schedule(
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
        /// Test mock: reads no market data.
        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
        }

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
            Ok(Money::from((0_i64, Currency::USD)))
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
        build_test_market_at(as_of)
            .insert(flat_discount("EUR", as_of, 1.0))
            .insert(flat_discount("USD", as_of, 1.0))
            .insert_fx(FxMatrix::new(provider))
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
                        (CFKind::Fixed, Money::from((100_i64, Currency::EUR))),
                        (CFKind::Notional, Money::from((200_i64, Currency::EUR))),
                    ]),
                ),
                (
                    Currency::USD,
                    IndexMap::from([(CFKind::Fee, Money::from((-10_i64, Currency::USD)))]),
                ),
            ]),
        );

        by_date.insert(
            date!(2025 - 08 - 01),
            IndexMap::from([
                (
                    Currency::USD,
                    IndexMap::from([(CFKind::Fixed, Money::from((50_i64, Currency::USD)))]),
                ),
                (
                    Currency::EUR,
                    IndexMap::from([(CFKind::Fee, Money::from((-5_i64, Currency::EUR)))]),
                ),
            ]),
        );

        by_date.insert(
            date!(2026 - 02 - 01),
            IndexMap::from([(
                Currency::EUR,
                IndexMap::from([(CFKind::Fixed, Money::from((25_i64, Currency::EUR)))]),
            )]),
        );

        PortfolioCashflows {
            events: Vec::new(),
            by_position: IndexMap::new(),
            by_date,
            position_summaries: IndexMap::new(),
            issues: Vec::new(),
            fx_collapse_policy: CashflowFxPolicy::CipForward,
        }
    }

    #[test]
    fn aggregate_full_cashflows_bond_ladder_has_usd_flows_and_contractual_summary() {
        let as_of = date!(2025 - 01 - 01);
        let bond = bond::Bond::fixed(
            "BOND_001",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            as_of,
            date!(2027 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &fail_closed())
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
        assert_eq!(full.fx_collapse_policy, CashflowFxPolicy::CipForward);
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let err = aggregate_full_cashflows(&portfolio, &MarketContext::new(), &fail_closed())
            .expect_err("default aggregation must fail closed on extraction issues");
        assert!(
            err.to_string().contains("POS_SWAP"),
            "fail-closed error should name the failed position: {err}"
        );

        let full = aggregate_full_cashflows(&portfolio, &MarketContext::new(), &allow_partial())
            .expect("allow_partial should succeed with issues");

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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &fail_closed())
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &fail_closed())
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
            .base_currency(Currency::USD)
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

        let full = aggregate_full_cashflows(&portfolio, &market, &fail_closed())
            .expect("cdx cashflow aggregation");

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
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let full =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &fail_closed())
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

        let has_kind_bucket = full.by_date.values().any(|per_currency| {
            per_currency.values().any(|per_kind| {
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
            .base_currency(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let err =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &fail_closed())
                .expect_err("default aggregation must fail closed on unsupported instruments");
        assert!(
            err.to_string().contains("POS_UNSUPPORTED"),
            "fail-closed error should name the failed position: {err}"
        );

        let full =
            aggregate_full_cashflows(&portfolio, &build_test_market_at(as_of), &allow_partial())
                .expect("allow_partial should produce issues, not fail the aggregation");

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
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of, None)
            .expect("base currency conversion by kind");

        let march = by_date_kind
            .get(&date!(2025 - 03 - 15))
            .expect("march bucket should exist");
        assert_eq!(march[&CFKind::Fixed], Money::from((120_i64, Currency::USD)));
        assert_eq!(
            march[&CFKind::Notional],
            Money::from((240_i64, Currency::USD))
        );
        assert_eq!(march[&CFKind::Fee], Money::from((-10_i64, Currency::USD)));

        let august = by_date_kind
            .get(&date!(2025 - 08 - 01))
            .expect("august bucket should exist");
        assert_eq!(august[&CFKind::Fixed], Money::from((50_i64, Currency::USD)));
        assert_eq!(august[&CFKind::Fee], Money::from((-6_i64, Currency::USD)));
    }

    fn single_kind_flow(date: Date, money: Money, kind: CFKind) -> PortfolioCashflows {
        let mut by_date: IndexMap<Date, IndexMap<Currency, IndexMap<CFKind, Money>>> =
            IndexMap::new();
        by_date.insert(
            date,
            IndexMap::from([(money.currency(), IndexMap::from([(kind, money)]))]),
        );
        PortfolioCashflows {
            events: Vec::new(),
            by_position: IndexMap::new(),
            by_date,
            position_summaries: IndexMap::new(),
            issues: Vec::new(),
            fx_collapse_policy: CashflowFxPolicy::CipForward,
        }
    }

    fn flat_discount(id: &str, as_of: Date, df_1y: f64) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, df_1y)])
            .interp(InterpStyle::Linear)
            .validation(ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            })
            .build()
            .expect("test discount curve should build")
    }

    fn market_with_cip_eurusd(as_of: Date, eurusd: f64, df_eur: f64, df_usd: f64) -> MarketContext {
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, eurusd)
            .expect("test FX quote should be valid");
        MarketContext::new()
            .insert(flat_discount("EUR", as_of, df_eur))
            .insert(flat_discount("USD", as_of, df_usd))
            .insert_fx(FxMatrix::new(provider))
    }

    #[test]
    fn collapse_converts_1y_eur_flow_with_cip_forward() {
        let as_of = date!(2025 - 01 - 01);
        let payment = date!(2026 - 01 - 01);
        let full = single_kind_flow(payment, Money::from((1_i64, Currency::EUR)), CFKind::Fixed);
        let market = market_with_cip_eurusd(as_of, 1.10, 0.99, 0.95);

        let by_date_kind = full
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of, None)
            .expect("CIP collapse should succeed when both discount curves exist");

        let expected = 1.10 * 0.99 / 0.95;
        assert_eq!(
            by_date_kind[&payment][&CFKind::Fixed],
            Money::new(expected, Currency::USD).expect("valid money fixture")
        );
    }

    #[test]
    fn collapse_uses_spot_for_payment_on_as_of() {
        let as_of = date!(2025 - 01 - 01);
        let full = single_kind_flow(as_of, Money::from((1_i64, Currency::EUR)), CFKind::Fixed);
        let market = market_with_cip_eurusd(as_of, 1.10, 0.99, 0.95);

        let by_date_kind = full
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of, None)
            .expect("same-date collapse should use spot");

        assert_eq!(
            by_date_kind[&as_of][&CFKind::Fixed],
            Money::new(1.10, Currency::USD).expect("valid money fixture")
        );
    }

    #[test]
    fn collapse_errors_when_eur_discount_is_missing() {
        let as_of = date!(2025 - 01 - 01);
        let payment = date!(2026 - 01 - 01);
        let full = single_kind_flow(payment, Money::from((1_i64, Currency::EUR)), CFKind::Fixed);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("test FX quote should be valid");
        let market = MarketContext::new()
            .insert(flat_discount("USD", as_of, 0.95))
            .insert_fx(FxMatrix::new(provider));

        let err = full
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of, None)
            .expect_err("future EUR flow must fail without an EUR discount curve");
        let message = err.to_string();
        assert!(
            message.contains("EUR") && message.to_ascii_lowercase().contains("discount"),
            "unexpected missing-curve error: {message}"
        );
    }

    #[test]
    fn collapse_uses_explicit_discount_curve_ids() {
        let as_of = date!(2025 - 01 - 01);
        let payment = date!(2026 - 01 - 01);
        let full = single_kind_flow(payment, Money::from((1_i64, Currency::EUR)), CFKind::Fixed);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("test FX quote should be valid");
        let market = MarketContext::new()
            .insert(flat_discount("EUR-OIS", as_of, 0.99))
            .insert(flat_discount("USD-OIS", as_of, 0.95))
            .insert_fx(FxMatrix::new(provider));
        let mut curves = HashMap::new();
        curves.insert(Currency::EUR, CurveId::new("EUR-OIS"));
        curves.insert(Currency::USD, CurveId::new("USD-OIS"));

        let by_date_kind = full
            .collapse_to_base_by_date_kind(&market, Currency::USD, as_of, Some(&curves))
            .expect("explicit curve-id map should resolve CIP discounts");

        let expected = 1.10 * 0.99 / 0.95;
        assert_eq!(
            by_date_kind[&payment][&CFKind::Fixed],
            Money::new(expected, Currency::USD).expect("valid money fixture")
        );
    }

    #[test]
    fn net_in_currency_by_date_json_sums_kinds_and_sorts_dates() {
        let json = r#"{
            "by_date": {
                "2025-04-15": {"USD": {"fixed": {"amount": "1011250", "currency": "USD"}}},
                "2025-01-15": {"USD": {"Notional": {"amount": "-3000000", "currency": "USD"}}},
                "2025-07-15": {"EUR": {"fixed": {"amount": "1", "currency": "EUR"}}}
            }
        }"#;
        let rows = net_in_currency_by_date_json(json, "USD").expect("net ladder");
        assert_eq!(
            rows,
            vec![
                ("2025-01-15".to_string(), -3_000_000.0),
                ("2025-04-15".to_string(), 1_011_250.0),
            ]
        );
    }
}
