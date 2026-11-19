//! Revalue an admitted netting set on dated market factor paths.

use crate::valuation::{PortfolioValuationOptions, RequestedMetrics, value_portfolio_at};
use crate::{Error, Portfolio, Result};
use finstack_quant_core::canonical::content_hash;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxQuery};
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_margin::types::NettingSetId;
use finstack_quant_margin::xva::types::{ExposureBasis, ExposureDiagnostics, ExposureProfile};
use finstack_quant_models::monte_carlo::simulate::{
    PathSimulationSpec, TimeGridSpec, simulate_paths,
};
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec, TimeRollMode,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Mapping from an admitted simulation state component to an exact market target.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExposureFactor {
    /// A parallel zero or forward rate movement. State is in decimal annual rate units.
    Rate {
        /// Index into the selected process state vector.
        state: usize,
        /// Discount or forward curve selected for the parallel move.
        curve_kind: CurveKind,
        /// Exact market curve identifier.
        curve_id: CurveId,
        /// Explicit discount dependency when the curve's shock recipe requires it.
        discount_curve_id: Option<CurveId>,
    },
    /// An FX spot level in quote currency per one unit of base currency.
    Fx {
        /// Index into the selected process state vector.
        state: usize,
        /// Base currency in the simulation quote.
        base: Currency,
        /// Quote currency in the simulation quote.
        quote: Currency,
    },
    /// An equity spot level in the contract's declared quote currency.
    Equity {
        /// Index into the selected process state vector.
        state: usize,
        /// Exact market price identifier.
        price_id: PriceId,
    },
}

impl ExposureFactor {
    fn state_index(&self) -> usize {
        match self {
            Self::Rate { state, .. } | Self::Fx { state, .. } | Self::Equity { state, .. } => {
                *state
            }
        }
    }

    fn target(&self) -> String {
        match self {
            Self::Rate {
                curve_kind,
                curve_id,
                ..
            } => format!("rate:{curve_kind:?}:{curve_id}"),
            Self::Fx { base, quote, .. } if base < quote => format!("fx:{base}:{quote}"),
            Self::Fx { base, quote, .. } => format!("fx:{quote}:{base}"),
            Self::Equity { price_id, .. } => format!("equity:{price_id}"),
        }
    }

    fn check_state(&self, market: &MarketContext, date: Date, state: &[f64]) -> Result<()> {
        let observed = match self {
            Self::Equity { price_id, .. } => monetary_spot(market, price_id)?,
            Self::Fx { base, quote, .. } => {
                if base == quote {
                    return Err(Error::invalid_input(
                        "Exposure FX factor must use two distinct currencies",
                    ));
                }
                market
                    .fx_required()?
                    .rate(FxQuery::new(
                        *base,
                        *quote,
                        date,
                        FxConversionPolicy::CashflowDate,
                    ))?
                    .rate
            }
            Self::Rate {
                curve_kind: CurveKind::Discount,
                curve_id,
                ..
            } => {
                market.get_discount(curve_id.as_str())?;
                return Ok(());
            }
            Self::Rate {
                curve_kind: CurveKind::Forward,
                curve_id,
                ..
            } => {
                market.get_forward(curve_id.as_str())?;
                return Ok(());
            }
            Self::Rate { .. } => {
                return Err(Error::invalid_input(
                    "Exposure rate factor must reference a discount or forward curve",
                ));
            }
        };
        let expected = state.get(self.state_index()).copied().ok_or_else(|| {
            Error::invalid_input("Exposure factor state index is outside the process output")
        })?;
        if expected <= 0.0 || !agreement(observed, expected) {
            return Err(Error::invalid_input(format!(
                "Observation on {date} for {} does not match the simulated level",
                self.target()
            )));
        }
        Ok(())
    }

    fn operation(
        &self,
        market: &MarketContext,
        date: Date,
        previous: &[f64],
        next: &[f64],
    ) -> Result<OperationSpec> {
        let state = self.state_index();
        let previous = previous
            .get(state)
            .copied()
            .ok_or_else(|| Error::invalid_input("missing previous simulation state"))?;
        let next = next
            .get(state)
            .copied()
            .ok_or_else(|| Error::invalid_input("missing next simulation state"))?;
        let relative_move = |previous: f64| {
            if previous <= 0.0 || next <= 0.0 {
                return Err(Error::invalid_input(
                    "exposure FX and equity levels must be positive",
                ));
            }
            Ok(100.0 * ((next - previous) / previous))
        };
        Ok(match self {
            Self::Rate {
                curve_kind,
                curve_id,
                discount_curve_id,
                ..
            } => {
                if !matches!(curve_kind, CurveKind::Discount | CurveKind::Forward) {
                    return Err(Error::invalid_input(
                        "simulated rate must select a discount or forward curve",
                    ));
                }
                OperationSpec::CurveParallelBp {
                    curve_kind: *curve_kind,
                    curve_id: curve_id.clone(),
                    discount_curve_id: discount_curve_id.clone(),
                    bp: (next - previous) * 10_000.0,
                }
            }
            Self::Fx { base, quote, .. } => OperationSpec::MarketFxPct {
                base: *base,
                quote: *quote,
                // The date-selected source can differ from the previous path
                // quote even when the time roll preserves the FX matrix.
                pct: relative_move(
                    market
                        .fx_required()?
                        .rate(FxQuery::new(
                            *base,
                            *quote,
                            date,
                            FxConversionPolicy::CashflowDate,
                        ))?
                        .rate,
                )?,
            },
            Self::Equity { price_id, .. } => OperationSpec::EquityPricePct {
                ids: vec![price_id.to_string()],
                pct: relative_move(previous)?,
            },
        })
    }
}

/// Elected contractual cash VM and signed opening account balance.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExposureCashVm {
    /// Terms of the named bilateral agreement in the reporting currency.
    pub terms: finstack_quant_margin::types::CsaSpec,
    /// Settled cash at the valuation cut before the first modeled call.
    /// Positive means held and negative means posted.
    pub opening_settled: Money,
}

struct CashVmSchedule {
    calculator: finstack_quant_margin::calculators::VmCalculator,
    call_dates: BTreeSet<Date>,
    curve_id: CurveId,
    opening: Money,
}

impl CashVmSchedule {
    fn prepare(
        authored: &ExposureCashVm,
        scope: &NettingSetId,
        market: &MarketContext,
        currency: Currency,
        dates: &[Date],
    ) -> Result<Self> {
        let terms = &authored.terms;
        terms.validate()?;
        let matching_agreement =
            matches!(scope, NettingSetId::Bilateral { csa_id, .. } if *csa_id == terms.id);
        let cash_eligible = terms.eligible_collateral.eligible.iter().any(|row| {
            row.asset_class == finstack_quant_margin::types::CollateralAssetClass::Cash
                && row.haircut == 0.0
                && row.min_rating.is_none()
                && row.maturity_constraints.is_none()
                && row
                    .concentration_limit
                    .is_none_or(|limit| limit.total_cmp(&1.0).is_eq())
        });
        if !matching_agreement
            || terms.base_currency != currency
            || authored.opening_settled.currency() != currency
            || !cash_eligible
            || matches!(
                terms.vm_params.frequency,
                finstack_quant_margin::types::MarginTenor::OnDemand
            )
        {
            return Err(Error::invalid_input(
                "Exposure VM requires recurring same-currency cash terms under the admitted agreement",
            ));
        }
        let opening = dates[0];
        let interest_curve = market.get_discount(terms.collateral_curve_id.as_str())?;
        if interest_curve.base_date() > opening
            || interest_curve
                .rate_calibration()
                .is_some_and(|recipe| recipe.currency != currency)
        {
            return Err(Error::invalid_input(
                "Exposure VM collateral curve must cover the opening date in the agreement currency",
            ));
        }
        let calculator = finstack_quant_margin::calculators::VmCalculator::new(terms.clone());
        let horizon = dates[dates.len() - 1];
        let call_dates = calculator
            .margin_call_dates(opening, horizon)?
            .into_iter()
            .filter(|date| *date <= horizon)
            .collect::<BTreeSet<_>>();
        for &call in &call_dates {
            if dates.binary_search(&call).is_err() {
                return Err(Error::invalid_input(format!(
                    "Exposure dates omit scheduled VM call {call}"
                )));
            }
            let settlement = calculator
                .calculate(
                    Money::from((0_i64, currency)),
                    Money::from((0_i64, currency)),
                    call,
                )?
                .settlement_date;
            if settlement <= horizon && dates.binary_search(&settlement).is_err() {
                return Err(Error::invalid_input(format!(
                    "Exposure dates omit scheduled VM settlement {settlement}"
                )));
            }
        }
        Ok(Self {
            calculator,
            call_dates,
            curve_id: terms.collateral_curve_id.clone(),
            opening: authored.opening_settled,
        })
    }
}

struct PathCashVm<'a> {
    schedule: &'a CashVmSchedule,
    settled: Money,
    pending: std::collections::BTreeMap<Date, Money>,
}

impl<'a> PathCashVm<'a> {
    fn new(schedule: &'a CashVmSchedule) -> Self {
        Self {
            settled: schedule.opening,
            schedule,
            pending: std::collections::BTreeMap::new(),
        }
    }

    fn advance(&mut self, market: &MarketContext, previous: Date, next: Date) -> Result<()> {
        let curve = market.get_discount(self.schedule.curve_id.as_str())?;
        let factor = curve.df_on_date(next)? / curve.df_on_date(previous)?;
        if !factor.is_finite() || factor <= 0.0 {
            return Err(Error::invalid_input("Invalid collateral interest factor"));
        }
        self.settled = self.settled.checked_div_f64(factor)?;
        if let Some(cash) = self.pending.remove(&next) {
            self.settled = self.settled.checked_add(cash)?;
        }
        Ok(())
    }

    fn net_mark(&mut self, mark: Money, date: Date) -> Result<Money> {
        if self.schedule.call_dates.contains(&date) {
            let mut agreed = self.settled;
            for cash in self.pending.values() {
                agreed = agreed.checked_add(*cash)?;
            }
            let call = self.schedule.calculator.calculate(mark, agreed, date)?;
            let cash = call.net_margin().checked_neg();
            if call.settlement_date == date {
                self.settled = self.settled.checked_add(cash)?;
            } else if call.requires_call() {
                let due = self
                    .pending
                    .entry(call.settlement_date)
                    .or_insert(Money::from((0_i64, mark.currency())));
                *due = due.checked_add(cash)?;
            }
        }
        Ok(mark.checked_sub(self.settled)?)
    }
}
/// Explicit market risk exposures and dated paths for one admitted netting set.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExposureSimulationConfig {
    /// Structural counterparty and agreement scope for the supplied positions.
    pub netting_set: NettingSetId,
    /// Identifier of the authoritative record admitting the supplied book.
    pub netting_source_id: String,
    /// Exact position IDs in that netting record, qualified as one eligible set.
    pub position_ids: Vec<String>,
    /// Elected cash variation margin with scheduled calls, settlements and
    /// collateral interest. Omit for an uncollateralized calculation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash_vm: Option<ExposureCashVm>,
    /// Explicit selection and units of each market factor to simulate.
    pub factors: Vec<ExposureFactor>,
    /// Observation calendar dates, including the portfolio valuation date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub dates: Vec<Date>,
    /// Process initialized against the held market. Its explicit grid must equal
    /// the ACT/365 Fixed observation dates. Unmapped state channels remain
    /// internal to the process, such as variance driving the equity spot.
    pub simulation: PathSimulationSpec,
}

fn agreement(left: f64, right: f64) -> bool {
    (left - right).abs() <= 32.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}

fn monetary_spot(market: &MarketContext, id: &PriceId) -> Result<f64> {
    let price = market.get_price(id.as_str())?.get_price().ok_or_else(|| {
        Error::invalid_input(format!("Exposure spot '{id}' must be a monetary scalar"))
    })?;
    let level = price.amount();
    if !level.is_finite() || level <= 0.0 {
        return Err(Error::invalid_input(format!(
            "Exposure spot '{id}' must be positive and finite"
        )));
    }
    Ok(level)
}

fn roll_market(
    market: &mut MarketContext,
    old_date: Date,
    date: Date,
    mut shocks: Vec<OperationSpec>,
    engine: &ScenarioEngine,
) -> Result<()> {
    let period = format!("{}D", (date - old_date).whole_days());
    shocks.insert(
        0,
        OperationSpec::TimeRollForward {
            period,
            apply_shocks: true,
            roll_mode: TimeRollMode::CalendarDays,
        },
    );
    let shock = ScenarioSpec {
        id: "exposure-path-step".into(),
        name: None,
        description: None,
        operations: shocks,
        priority: 0,
        resolution_mode: Default::default(),
    };
    let mut ctx = ExecutionContext {
        market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of: old_date,
    };
    let result = engine
        .apply(&shock, &mut ctx)
        .map_err(|err| Error::scenario(err.kind(), err.to_string()))?;
    if ctx.as_of != date || !result.warnings.is_empty() {
        return Err(Error::invalid_input(format!(
            "Exposure step ending {date} did not complete: {:?}",
            result.warnings
        )));
    }
    Ok(())
}

/// Compute dated expected positive and negative exposure with the elected collateral basis.
///
/// Rolls one market per path date, applies the elected factor moves, and
/// prices the admitted book in its reporting currency before taking positive
/// and negative parts. A rate state supplies successive changes in annual
/// decimal rates on its selected curve, on top of its deterministic roll.
/// Spot states are absolute levels and must match the initial market.
///
/// # Arguments
/// * `portfolio` - Positions in one legally nettable set and its reporting currency.
/// * `market` - Immutable source market with the admitted risk factors, opening spot levels and pricing dependencies.
/// * `pricing` - Valuation and FX conversion basis shared by all path dates.
/// * `spec` - Calendar observation grid, netting record, exact market factor coordinates and process inputs.
///
/// # Errors
/// Returns an error for incompatible state/date grids, missing market data,
/// a failed roll, unavailable FX, or any incomplete instrument valuation.
pub fn portfolio_exposure_profile(
    portfolio: &Portfolio,
    market: &MarketContext,
    pricing: &FinstackConfig,
    spec: &ExposureSimulationConfig,
) -> Result<ExposureProfile> {
    let dates = &spec.dates;
    if dates.len() < 2
        || dates.first() != Some(&portfolio.as_of)
        || dates.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(Error::invalid_input(
            "Exposure dates must start at valuation and strictly increase",
        ));
    }
    let times: Vec<_> = dates
        .iter()
        .map(|date| (*date - portfolio.as_of).whole_days() as f64 / 365.0)
        .collect();
    let TimeGridSpec::Times { times: authored } = &spec.simulation.time_grid else {
        return Err(Error::invalid_input(
            "Exposure simulation requires explicit times on the dated grid",
        ));
    };
    if authored.len() != times.len()
        || authored
            .iter()
            .zip(&times)
            .any(|(&left, &right)| !agreement(left, right))
    {
        return Err(Error::invalid_input(
            "Exposure simulation times must match the ACT/365 Fixed observation grid",
        ));
    }
    let admitted: BTreeSet<_> = spec.position_ids.iter().map(String::as_str).collect();
    let actual: BTreeSet<_> = portfolio
        .positions()
        .iter()
        .map(|position| position.position_id.as_str())
        .collect();
    let named_scope = matches!(&spec.netting_set, NettingSetId::Bilateral { counterparty_id, csa_id }
        if !counterparty_id.trim().is_empty() && !csa_id.trim().is_empty());
    if !named_scope
        || spec.netting_source_id.trim().is_empty()
        || actual.is_empty()
        || admitted != actual
        || admitted.len() != spec.position_ids.len()
    {
        return Err(Error::invalid_input(
            "Exposure requires an explicit single netting scope for the exact position IDs",
        ));
    }
    let cash_vm = spec
        .cash_vm
        .as_ref()
        .map(|vm| {
            CashVmSchedule::prepare(
                vm,
                &spec.netting_set,
                market,
                portfolio.base_currency,
                dates,
            )
        })
        .transpose()?;
    let simulated = simulate_paths(&spec.simulation)?;
    let mut targets = BTreeSet::new();
    if spec.factors.is_empty() {
        return Err(Error::invalid_input(
            "Exposure simulation needs at least one factor mapping",
        ));
    }
    for factor in &spec.factors {
        if factor.state_index() >= simulated.dim || !targets.insert(factor.target()) {
            return Err(Error::invalid_input(
                "Exposure factor state index is invalid or its market target is duplicated",
            ));
        }
        factor.check_state(market, portfolio.as_of, &simulated.values[..simulated.dim])?;
    }

    let mut sums = vec![[NeumaierAccumulator::new(); 3]; dates.len()];
    let options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(Vec::new()),
    };
    let engine = ScenarioEngine::with_config(pricing.clone());
    let opening = value_portfolio_at(portfolio, market, pricing, &options, portfolio.as_of)?
        .total_base_currency;
    let stride = dates.len() * simulated.dim;
    for path in simulated.values.chunks_exact(stride) {
        let mut path_market = market.clone();
        let mut path_vm = cash_vm.as_ref().map(PathCashVm::new);
        for (index, ((state, &date), sum)) in path
            .chunks_exact(simulated.dim)
            .zip(dates)
            .zip(&mut sums)
            .enumerate()
        {
            let mark = if index == 0 {
                opening
            } else {
                if let Some(vm) = &mut path_vm {
                    vm.advance(&path_market, dates[index - 1], date)?;
                }
                let previous = &path[(index - 1) * simulated.dim..index * simulated.dim];
                let shocks = spec
                    .factors
                    .iter()
                    .map(|factor| factor.operation(&path_market, date, previous, state))
                    .collect::<Result<Vec<_>>>()?;
                roll_market(&mut path_market, dates[index - 1], date, shocks, &engine)?;
                for factor in &spec.factors {
                    factor.check_state(&path_market, date, state)?;
                }
                value_portfolio_at(portfolio, &path_market, pricing, &options, date)?
                    .total_base_currency
            };
            let net = match &mut path_vm {
                Some(vm) => vm.net_mark(mark, date)?,
                None => mark,
            }
            .amount();
            sum[0].add(mark.amount())?;
            sum[1].add(net.max(0.0))?;
            sum[2].add((-net).max(0.0))?;
        }
    }

    let source_id = content_hash(&(portfolio.to_spec(), market, pricing, spec))?;
    let path_count = simulated.num_simulated_paths;
    let basis = if cash_vm.is_some() {
        ExposureBasis::SimulatedCashVm {
            source_id,
            path_count,
        }
    } else {
        ExposureBasis::SimulatedUncollateralized {
            source_id,
            path_count,
        }
    };
    let mut profile = ExposureProfile {
        as_of: portfolio.as_of,
        currency: portfolio.base_currency,
        basis,
        dates: dates.clone(),
        times,
        mtm_values: Vec::with_capacity(dates.len()),
        epe: Vec::with_capacity(dates.len()),
        ene: Vec::with_capacity(dates.len()),
        diagnostics: ExposureDiagnostics {
            market_roll_failures: 0,
            valuation_failures: 0,
            total_time_points: dates.len(),
        },
    };
    let count = simulated.num_simulated_paths as f64;
    for [mtm, epe, ene] in sums {
        profile.mtm_values.push(mtm.get_total()? / count);
        profile.epe.push(epe.get_total()? / count);
        profile.ene.push(ene.get_total()? / count);
    }
    profile.validate()?;
    Ok(profile)
}

/// Simulate the dated exposure of one contractual instrument.
///
/// # Arguments
/// * `instrument` - Validated instrument to revalue at its contractual size and direction.
/// * `currency` - Reporting currency used by the exposure profile.
/// * `market` - Immutable market snapshot with the admitted factor dependencies.
/// * `pricing` - Shared valuation and reporting FX basis.
/// * `spec` - Dated path and netting evidence. Its one position ID equals the instrument ID.
///
/// # Errors
/// Rejects an empty grid or inconsistent identity, and propagates the same
/// path, data and revaluation checks as [`portfolio_exposure_profile`].
pub fn instrument_exposure_profile(
    instrument: &dyn finstack_quant_valuations::instruments::Instrument,
    currency: Currency,
    market: &MarketContext,
    pricing: &FinstackConfig,
    spec: &ExposureSimulationConfig,
) -> Result<ExposureProfile> {
    let date = spec.dates.first().copied().ok_or_else(|| {
        Error::invalid_input(
            "Instrument exposure requires the valuation date as its first observation",
        )
    })?;
    let position = crate::position::Position::new(
        instrument.id(),
        "instrument",
        instrument.id(),
        instrument.clone_box().into(),
        1.0,
        crate::position::PositionUnit::Units,
    )?;
    let portfolio = Portfolio::builder("instrument-exposure")
        .base_currency(currency)
        .as_of(date)
        .entity(crate::types::Entity::new("instrument"))
        .position(position)
        .build()?;
    portfolio_exposure_profile(&portfolio, market, pricing, spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::monte_carlo::process::gbm::GbmParams;
    use finstack_quant_models::monte_carlo::simulate::{ProcessSpec, SchemeSpec};
    use finstack_quant_valuations::instruments::{Instrument, equity::spot::Equity};
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn exposure_reprices_each_date_and_keeps_the_signed_position_basis() {
        let as_of = date!(2025 - 01 - 02);
        let dates = vec![as_of, date!(2025 - 04 - 02), date!(2025 - 07 - 02)];
        let times: Vec<_> = dates
            .iter()
            .map(|d| (*d - as_of).whole_days() as f64 / 365.0)
            .collect();
        let equity = Equity::new("equity", "STOCK", Currency::USD).with_spot_id("STOCK-SPOT");
        let position = Position::new(
            "short-stock",
            "entity",
            equity.id(),
            Arc::new(equity.clone()),
            -2.0,
            PositionUnit::Units,
        )
        .expect("signed position");
        let book = Portfolio::builder("test-paths")
            .entity(Entity::new("entity"))
            .base_currency(Currency::USD)
            .as_of(as_of)
            .position(position)
            .build()
            .expect("book");
        let market = MarketContext::new().insert_price(
            "STOCK-SPOT",
            MarketScalar::price(Money::new(100.0, Currency::USD).expect("fixed opening price")),
        );
        let mut spec = ExposureSimulationConfig {
            dates,
            position_ids: vec!["short-stock".into()],
            cash_vm: None,
            netting_set: NettingSetId::bilateral("counterparty", "scope"),
            netting_source_id: "approved-stochastic-fixture".into(),
            factors: vec![ExposureFactor::Equity {
                state: 0,
                price_id: "STOCK-SPOT".into(),
            }],
            simulation: PathSimulationSpec {
                process: ProcessSpec::Gbm(
                    GbmParams::new(0.05, 0.0, 0.0).expect("deterministic drift"),
                ),
                scheme: SchemeSpec::Default,
                initial_state: vec![100.0],
                time_grid: TimeGridSpec::Times {
                    times: times.clone(),
                },
                num_paths: 4,
                seed: 12,
                antithetic: true,
                fbm: None,
            },
        };
        let before = content_hash(&market).expect("opening market identity");
        let pricing = FinstackConfig::default();
        let profile =
            portfolio_exposure_profile(&book, &market, &pricing, &spec).expect("dated path prices");
        assert_eq!(profile.epe, vec![0.0; 3]);
        assert_eq!(profile.mtm_values[0], -200.0);
        let expected = 200.0 * (0.05 * times[2]).exp();
        assert!((profile.ene[2] - expected).abs() < 1.0e-9);
        assert_eq!(content_hash(&market).expect("immutable market"), before);
        spec.simulation.initial_state[0] = 90.0;
        assert!(portfolio_exposure_profile(&book, &market, &pricing, &spec).is_err());
        spec.simulation.initial_state[0] = 100.0;
        spec.position_ids = vec!["equity".into()];
        let single = instrument_exposure_profile(&equity, Currency::USD, &market, &pricing, &spec)
            .expect("standalone route uses the same factor paths");
        assert!((single.epe[2] - 0.5 * expected).abs() < 1.0e-9);
    }

    #[test]
    fn fx_state_replaces_the_dated_quote_before_reporting_conversion() {
        use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};

        let as_of = date!(2025 - 01 - 02);
        let final_date = date!(2026 - 01 - 02);
        let fx = FxMatrix::new(Arc::new(SimpleFxProvider::new()), None);
        for (date, rate) in [(as_of, 1.2), (final_date, 1.4)] {
            fx.set_quote_on(
                Currency::EUR,
                Currency::USD,
                date,
                FxConversionPolicy::CashflowDate,
                rate,
            )
            .expect("dated EUR/USD fixing");
        }
        let equity = Equity::new("eur-equity", "STOCK", Currency::EUR).with_spot_id("EURO-SPOT");
        let market = MarketContext::new()
            .insert_price(
                "EURO-SPOT",
                MarketScalar::price(Money::from((100_i64, Currency::EUR))),
            )
            .insert_fx(fx);
        let spec = ExposureSimulationConfig {
            dates: vec![as_of, final_date],
            position_ids: vec!["eur-equity".into()],
            cash_vm: None,
            netting_set: NettingSetId::bilateral("currency-dealer", "scope"),
            netting_source_id: "approved-fx-fixture".into(),
            factors: vec![ExposureFactor::Fx {
                state: 0,
                base: Currency::EUR,
                quote: Currency::USD,
            }],
            simulation: PathSimulationSpec {
                process: ProcessSpec::Gbm(
                    GbmParams::new(0.10, 0.0, 0.0).expect("deterministic drift"),
                ),
                scheme: SchemeSpec::Default,
                initial_state: vec![1.2],
                time_grid: TimeGridSpec::Times {
                    times: vec![0.0, 1.0],
                },
                num_paths: 2,
                seed: 42,
                antithetic: false,
                fbm: None,
            },
        };
        let profile = instrument_exposure_profile(
            &equity,
            Currency::USD,
            &market,
            &FinstackConfig::default(),
            &spec,
        )
        .expect("dated FX should match the simulated level at every mark");
        assert!((profile.epe[0] - 120.0).abs() < 1.0e-9);
        assert!((profile.epe[1] - 120.0 * 0.1_f64.exp()).abs() < 1.0e-9);
    }

    #[test]
    fn rate_path_applies_decimal_changes_to_the_rolled_discount_curve() {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_core::math::Compounding;
        use finstack_quant_valuations::instruments::fixed_income::bond::Bond;

        let as_of = date!(2025 - 01 - 02);
        let final_date = date!(2026 - 01 - 02);
        let maturity = date!(2027 - 01 - 04);
        let bond = Bond::zero_coupon(
            "bond",
            Money::from((1_000_i64, Currency::USD)),
            as_of,
            maturity,
            Compounding::Continuous,
            "discount",
        )
        .expect("fixed redemption");
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::flat("discount", as_of, 0.04, DayCount::Act365F)
                    .expect("flat discount curve"),
            )
            .expect("opening discount");
        let spec = ExposureSimulationConfig {
            dates: vec![as_of, final_date],
            position_ids: vec!["bond".into()],
            cash_vm: None,
            netting_set: NettingSetId::bilateral("cash-dealer", "scope"),
            netting_source_id: "admitted-rate-fixture".into(),
            factors: vec![ExposureFactor::Rate {
                state: 0,
                curve_kind: CurveKind::Discount,
                curve_id: "discount".into(),
                discount_curve_id: None,
            }],
            simulation: PathSimulationSpec {
                process: ProcessSpec::Gbm(
                    GbmParams::new(1.25_f64.ln(), 0.0, 0.0)
                        .expect("deterministic annual increase from 4% to 5%"),
                ),
                scheme: SchemeSpec::Default,
                initial_state: vec![0.04],
                time_grid: TimeGridSpec::Times {
                    times: vec![0.0, 1.0],
                },
                num_paths: 2,
                seed: 42,
                antithetic: false,
                fbm: None,
            },
        };
        let profile = instrument_exposure_profile(
            &bond,
            Currency::USD,
            &market,
            &FinstackConfig::default(),
            &spec,
        )
        .expect("zero coupon reprices after the roll and 100 bp move");
        let expected = |date: Date, rate: f64| {
            1_000.0 * (-rate * (maturity - date).whole_days() as f64 / 365.0_f64).exp()
        };
        assert!((profile.epe[0] - expected(as_of, 0.04)).abs() < 1.0e-8);
        assert!((profile.epe[1] - expected(final_date, 0.05)).abs() < 1.0e-8);
    }

    #[test]
    fn cash_vm_tracks_pending_calls_and_deducts_only_settled_cash() {
        use finstack_quant_core::dates::DayCount;
        use finstack_quant_core::market_data::term_structures::DiscountCurve;
        use finstack_quant_margin::types::CsaSpec;

        let dates = [
            date!(2025 - 01 - 02),
            date!(2025 - 01 - 03),
            date!(2025 - 01 - 06),
        ];
        let mut terms =
            CsaSpec::regulatory_for_currency(Currency::USD, "test-cash-csa", "collateral")
                .expect("USD CSA");
        terms.vm_params.threshold = Money::from((50_i64, Currency::USD));
        terms.vm_params.mta = Money::from((0_i64, Currency::USD));
        terms.vm_params.rounding = Money::from((0_i64, Currency::USD));
        terms.vm_params.independent_amount = Money::from((0_i64, Currency::USD));
        terms.vm_params.settlement_lag = 1;
        let market = MarketContext::new()
            .try_insert(
                DiscountCurve::flat("collateral", dates[0], 0.0, DayCount::Act365F)
                    .expect("flat collateral discount"),
            )
            .expect("opening collateral market");
        let scope = NettingSetId::bilateral("counterparty", "test-cash-csa");
        let authored = ExposureCashVm {
            terms,
            opening_settled: Money::from((0_i64, Currency::USD)),
        };
        let schedule = CashVmSchedule::prepare(&authored, &scope, &market, Currency::USD, &dates)
            .expect("covered call and settlement grid");
        let mut account = PathCashVm::new(&schedule);
        let mark = Money::from((-200_i64, Currency::USD));
        assert_eq!(
            account.net_mark(mark, dates[0]).expect("pending call"),
            mark
        );
        account
            .advance(&market, dates[0], dates[1])
            .expect("dated settlement");
        assert_eq!(
            account
                .net_mark(mark, dates[1])
                .expect("settled margin")
                .amount(),
            -50.0
        );
        assert!(
            CashVmSchedule::prepare(
                &authored,
                &scope,
                &market,
                Currency::USD,
                &[dates[0], dates[2]]
            )
            .is_err()
        );
    }
}
