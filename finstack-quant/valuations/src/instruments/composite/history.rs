//! Dated-market engine for composite value, P&L, returns, and rebalancing.
//!
//! [`CompositeHistoryEngine`] values a resolved composite on each strictly
//! increasing observation. Warmup snapshots feed dynamic weighting only. A
//! scheduled rebalance is close-effective: the emitted row still uses the
//! holdings that entered the close, and the next interval opens at the
//! post-trade financed value.
//!
//! Period return is `pnl / capital` for one composite unit. The chained
//! `return_index` starts at `100` on the first output row.
use super::types::{cashflows_between, validate_history};
use super::{
    CompositeExposureReport, CompositeInstrument, CompositeMarketObservation, CompositeSpec,
    CompositeTrade,
};
use crate::instruments::Instrument;
use crate::metrics::MetricId;
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// One dated output row from the focused composite history engine.
///
/// The first output row reports `cashflows = 0`, `pnl = 0`,
/// `period_return = 0`, and `return_index = 100`. Later rows use
/// `period_return = pnl / capital` and chain
/// `return_index *= 1 + period_return`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeHistoryRow {
    /// Market observation date of this close.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Composite value before any close-of-period rebalance, in reporting currency.
    pub value: Money,
    /// Signed primitive cashflows on `(previous_date, date]`; zero on the first row.
    pub cashflows: Money,
    /// `Δvalue + cashflows` versus the prior interval's financed opening value;
    /// zero on the first row.
    pub pnl: Money,
    /// `pnl / capital` for one composite unit; zero on the first row.
    pub period_return: f64,
    /// Chained total-return index, initialized to `100` on the first row.
    pub return_index: f64,
    /// Effective date of quantities held into this close.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub held_state_effective_date: Date,
    /// New state date made effective for the next interval, when rebalanced.
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub next_state_effective_date: Option<Date>,
    /// Primitive path, net, and gross exposures under the held (pre-rebalance) state.
    pub exposures: CompositeExposureReport,
    /// Primitive quantity deltas emitted by a close-of-period rebalance.
    pub rebalance_trades: Vec<CompositeTrade>,
}

/// Dated-market engine for composite value, P&L, returns, and rebalancing.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompositeHistoryEngine;

impl CompositeHistoryEngine {
    /// Initialize a specification and run it over dated market snapshots.
    ///
    /// Warmup observations are visible to dynamic weighting but are not emitted
    /// as history rows. The initial state is resolved on the first output
    /// observation using only warmup data and that observation.
    ///
    /// # Arguments
    ///
    /// * `spec` - Unresolved composite specification initialized from warmup
    ///   plus the first output observation.
    /// * `warmup` - Strictly increasing complete snapshots whose last date
    ///   precedes the first output observation; used only for weighting
    ///   inputs.
    /// * `observations` - Non-empty strictly increasing complete snapshots
    ///   that become output rows.
    /// * `metrics` - Additive primitive risk metrics included in each
    ///   exposure report; an empty slice reports value only.
    ///
    /// # Errors
    ///
    /// Returns an error for empty or unordered observations, overlapping warmup
    /// dates, initialization failures, missing market data, or history failures.
    pub fn run_from_spec(
        spec: &CompositeSpec,
        warmup: &[CompositeMarketObservation],
        observations: &[CompositeMarketObservation],
        metrics: &[MetricId],
    ) -> Result<Vec<CompositeHistoryRow>> {
        validate_output_history(warmup, observations)?;
        let first = observations.first().ok_or_else(|| {
            Error::Validation("composite history requires at least one observation".to_string())
        })?;
        let first_market = first.restore()?;
        let mut initial_history = warmup.to_vec();
        initial_history.push(first.clone());
        let initial = spec
            .initialize(&first_market, first.date, &initial_history)?
            .instrument;
        Self::run_with_warmup(&initial, warmup, observations, metrics)
    }

    /// Run an already-resolved composite over dated market snapshots.
    ///
    /// The initial state's `effective_date` must be on or before the first
    /// observation. Scheduled rebalances after that date are applied
    /// close-effectively.
    ///
    /// # Arguments
    ///
    /// * `initial` - Resolved quantities held from the first observation
    ///   until a later scheduled or explicit rebalance.
    /// * `observations` - Non-empty strictly increasing complete market
    ///   snapshots that become output rows.
    /// * `metrics` - Additive primitive risk metrics included in each
    ///   exposure report; an empty slice reports value only.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid state, empty or unordered observations,
    /// missing market data, valuation failures, or rebalance failures.
    pub fn run(
        initial: &CompositeInstrument,
        observations: &[CompositeMarketObservation],
        metrics: &[MetricId],
    ) -> Result<Vec<CompositeHistoryRow>> {
        Self::run_with_warmup(initial, &[], observations, metrics)
    }

    fn run_with_warmup(
        initial: &CompositeInstrument,
        warmup: &[CompositeMarketObservation],
        observations: &[CompositeMarketObservation],
        metrics: &[MetricId],
    ) -> Result<Vec<CompositeHistoryRow>> {
        validate_output_history(warmup, observations)?;
        initial.validate_for_pricing()?;
        let first = observations.first().ok_or_else(|| {
            Error::Validation("composite history requires at least one observation".to_string())
        })?;
        if initial.state.effective_date > first.date {
            return Err(Error::Validation(
                "initial composite state is effective after the first history observation"
                    .to_string(),
            ));
        }

        let mut state = initial.clone();
        let mut rows = Vec::with_capacity(observations.len());
        let mut available_history = warmup.to_vec();
        let mut previous_value = None::<f64>;
        let mut previous_date = None::<Date>;
        let mut return_index = 100.0;

        for observation in observations {
            available_history.push(observation.clone());
            let market = observation.restore()?;
            let held_state_effective_date = state.state.effective_date;
            let value = state.value(&market, observation.date)?;
            let cashflows = match previous_date {
                Some(start) => {
                    composite_cashflows_between(&state, &market, start, observation.date)?
                }
                None => 0.0,
            };
            let pnl = previous_value.map_or(0.0, |previous| value.amount() - previous + cashflows);
            let period_return = if previous_value.is_some() {
                pnl / state.spec.capital.amount()
            } else {
                0.0
            };
            if !period_return.is_finite() {
                return Err(Error::Validation(
                    "composite history produced a non-finite period return".to_string(),
                ));
            }
            if previous_value.is_some() {
                return_index *= 1.0 + period_return;
            }
            let exposures = state.primitive_exposure_report(&market, observation.date, metrics)?;

            let mut rebalance_trades = Vec::new();
            let mut next_state_effective_date = None;
            let mut financed_close_value = value.amount();
            if rebalance_due(&state, observation.date)? {
                let result = state.rebalance(&market, observation.date, &available_history)?;
                rebalance_trades = result.trades;
                next_state_effective_date = Some(observation.date);
                state = result.instrument;
                // Reset the next interval's opening value to the post-trade
                // portfolio at this same close. The difference from the
                // pre-trade value is external financing, not investment P&L.
                financed_close_value = state.value(&market, observation.date)?.amount();
            }

            rows.push(CompositeHistoryRow {
                date: observation.date,
                value,
                cashflows: Money::new(cashflows, initial.spec.reporting_currency)?,
                pnl: Money::new(pnl, initial.spec.reporting_currency)?,
                period_return,
                return_index,
                held_state_effective_date,
                next_state_effective_date,
                exposures,
                rebalance_trades,
            });
            previous_value = Some(financed_close_value);
            previous_date = Some(observation.date);
        }
        Ok(rows)
    }
}

fn validate_output_history(
    warmup: &[CompositeMarketObservation],
    observations: &[CompositeMarketObservation],
) -> Result<()> {
    validate_history(warmup, None)?;
    validate_history(observations, None)?;
    if let (Some(warmup_last), Some(first)) = (warmup.last(), observations.first()) {
        if warmup_last.date >= first.date {
            return Err(Error::Validation(
                "composite warmup observations must precede output observations".to_string(),
            ));
        }
    }
    Ok(())
}

fn rebalance_due(instrument: &CompositeInstrument, date: Date) -> Result<bool> {
    Ok(instrument
        .spec
        .rebalance_rule
        .dates_through(date)?
        .into_iter()
        .any(|scheduled| scheduled > instrument.state.effective_date && scheduled <= date))
}

fn composite_cashflows_between(
    instrument: &CompositeInstrument,
    market: &finstack_quant_core::market_data::context::MarketContext,
    start: Date,
    end: Date,
) -> Result<f64> {
    let primitives = instrument.flatten_primitives()?;
    let amounts = primitives
        .iter()
        .map(|primitive| {
            let instrument_json = primitive.instrument.as_ref().ok_or_else(|| {
                Error::Internal("primitive exposure lost its runtime instrument".to_string())
            })?;
            cashflows_between(
                instrument_json,
                primitive.quantity,
                instrument.spec.reporting_currency,
                market,
                start,
                end,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(neumaier_sum(amounts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::composite::{CompositeLegSpec, RebalanceRule, WeightingMethod};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::expr::{BinOp, Expr, UnaryOp};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;
    use time::macros::date;

    #[test]
    fn fixed_history_chains_returns_without_rebalancing() -> Result<()> {
        let initial = CompositeInstrument::example()?;
        let observations = vec![
            CompositeMarketObservation::new(date!(2025 - 01 - 01), &MarketContext::new()),
            CompositeMarketObservation::new(date!(2025 - 01 - 02), &MarketContext::new()),
        ];
        let rows = CompositeHistoryEngine::run(&initial, &observations, &[])?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].return_index, 100.0);
        assert_eq!(rows[1].pnl.amount(), 0.0);
        assert!(rows[1].rebalance_trades.is_empty());
        Ok(())
    }

    #[test]
    fn scheduled_rebalance_is_close_effective_and_principal_is_not_pnl() -> Result<()> {
        let legs = vec![
            CompositeLegSpec::new(
                "A",
                crate::instruments::InstrumentJson::Equity(crate::instruments::Equity::new(
                    "A",
                    "A",
                    Currency::USD,
                )),
                1.0,
            ),
            CompositeLegSpec::new(
                "B",
                crate::instruments::InstrumentJson::Equity(crate::instruments::Equity::new(
                    "B",
                    "B",
                    Currency::USD,
                )),
                -1.0,
            ),
        ];
        let expressions = IndexMap::from([
            (
                "A".to_string(),
                Expr::bin_op(
                    BinOp::Div,
                    Expr::literal(100.0),
                    Expr::column("leg.A.value"),
                ),
            ),
            (
                "B".to_string(),
                Expr::unary_op(
                    UnaryOp::Neg,
                    Expr::bin_op(
                        BinOp::Div,
                        Expr::literal(100.0),
                        Expr::column("leg.B.value"),
                    ),
                ),
            ),
        ]);
        let spec = CompositeSpec::new(
            "HISTORY",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            legs,
            WeightingMethod::UserDefined {
                required_metrics: Vec::new(),
                quantity_expressions: expressions,
            },
            RebalanceRule::Dates {
                dates: vec![date!(2025 - 01 - 02)],
            },
        );
        let observations = [
            (date!(2025 - 01 - 01), 100.0, 100.0),
            (date!(2025 - 01 - 02), 200.0, 100.0),
            (date!(2025 - 01 - 03), 220.0, 110.0),
        ]
        .into_iter()
        .map(|(date, a, b)| {
            let market = MarketContext::new()
                .insert_price("A", MarketScalar::Unitless(a))
                .insert_price("B", MarketScalar::Unitless(b));
            CompositeMarketObservation::new(date, &market)
        })
        .collect::<Vec<_>>();

        let rows = CompositeHistoryEngine::run_from_spec(&spec, &[], &observations, &[])?;
        assert_eq!(rows[1].held_state_effective_date, date!(2025 - 01 - 01));
        assert_eq!(
            rows[1].next_state_effective_date,
            Some(date!(2025 - 01 - 02))
        );
        assert_eq!(rows[1].rebalance_trades.len(), 1);
        assert!((rows[1].rebalance_trades[0].quantity_delta + 0.5).abs() < 1.0e-12);
        assert_eq!(rows[2].held_state_effective_date, date!(2025 - 01 - 02));
        assert!(rows[2].pnl.amount().abs() < 1.0e-12);
        Ok(())
    }
}
