// finstack/scenarios/src/horizon.rs

//! Horizon total return analysis.
//!
//! Composes [`ScenarioSpec`] application with P&L attribution to answer:
//! "If I hold this instrument under these market assumptions, what is my
//! decomposed total return?"
//!
//! The caller supplies a [`ScenarioSpec`] that may include a
//! [`OperationSpec::TimeRollForward`] (holding period) alongside any market
//! shocks (spread widening, rate shifts, vol changes, etc.).  The engine
//! applies the spec to construct a T₁ market state, then delegates to the
//! existing attribution framework to decompose the P&L.
//!
//! # Quick Start
//!
//! ```ignore
//! use finstack_scenarios::horizon::HorizonAnalysis;
//! use finstack_scenarios::{OperationSpec, ScenarioSpec, TimeRollMode};
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_valuations::instruments::Instrument;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let instrument: Arc<dyn Instrument> = todo!("your instrument");
//! let market = MarketContext::new();
//! let as_of = date!(2025-01-15);
//!
//! // Hold for 3 months, spreads widen 25bp
//! let scenario = ScenarioSpec {
//!     id: "hold_3m_spread_25".into(),
//!     name: None,
//!     description: None,
//!     operations: vec![
//!         OperationSpec::TimeRollForward {
//!             period: "3M".into(),
//!             apply_shocks: true,
//!             roll_mode: TimeRollMode::BusinessDays,
//!         },
//!         OperationSpec::CurveParallelBp {
//!             curve_kind: finstack_scenarios::CurveKind::ParCDS,
//!             curve_id: "AAPL-CDS".into(),
//!             discount_curve_id: None,
//!             bp: 25.0,
//!         },
//!     ],
//!     priority: 0,
//!     resolution_mode: Default::default(),
//! };
//!
//! let analyzer = HorizonAnalysis::default();
//! let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;
//!
//! println!("Total return: {:.2}%", result.total_return_pct() * 100.0);
//! println!("Carry: {}", result.attribution.carry);
//! println!("Credit P&L: {}", result.attribution.credit_curves_pnl);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use finstack_attribution::{
    attribute_pnl_metrics_based, attribute_pnl_parallel, attribute_pnl_taylor,
    attribute_pnl_waterfall, default_attribution_metrics, AttributionFactor, AttributionMethod,
    PnlAttribution,
};
use finstack_core::config::FinstackConfig;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::PricingOptions;

use crate::engine::ApplicationReport;
use crate::{ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec};

fn horizon_unsupported_instrument_operation(scenario: &ScenarioSpec) -> Option<&'static str> {
    scenario.operations.iter().find_map(|op| match op {
        OperationSpec::InstrumentPricePctByType { .. } => Some("InstrumentPricePctByType"),
        OperationSpec::InstrumentPricePctByAttr { .. } => Some("InstrumentPricePctByAttr"),
        OperationSpec::InstrumentSpreadBpByType { .. } => Some("InstrumentSpreadBpByType"),
        OperationSpec::InstrumentSpreadBpByAttr { .. } => Some("InstrumentSpreadBpByAttr"),
        OperationSpec::AssetCorrelationPts { .. } => Some("AssetCorrelationPts"),
        OperationSpec::PrepayDefaultCorrelationPts { .. } => Some("PrepayDefaultCorrelationPts"),
        _ => None,
    })
}

/// Horizon total return analyzer.
///
/// Composes scenario application with P&L attribution.  Construct with an
/// [`AttributionMethod`] and [`FinstackConfig`], then call [`compute`] to
/// project an instrument forward under a [`ScenarioSpec`] and decompose the
/// resulting P&L.
///
/// [`compute`]: HorizonAnalysis::compute
#[derive(Debug, Clone)]
pub struct HorizonAnalysis {
    /// Attribution methodology for decomposing the horizon P&L.
    pub attribution_method: AttributionMethod,
    /// Finstack configuration (rounding, tolerances).
    pub config: FinstackConfig,
    /// Scenario engine instance.
    pub engine: ScenarioEngine,
}

impl Default for HorizonAnalysis {
    fn default() -> Self {
        Self {
            attribution_method: AttributionMethod::Parallel,
            config: FinstackConfig::default(),
            engine: ScenarioEngine::new(),
        }
    }
}

impl HorizonAnalysis {
    /// Create a new analyzer with the given attribution method and config.
    pub fn new(attribution_method: AttributionMethod, config: FinstackConfig) -> Self {
        Self {
            attribution_method,
            config,
            engine: ScenarioEngine::new(),
        }
    }
}

/// Result of a horizon total return computation.
///
/// Wraps a [`PnlAttribution`] with scenario context and convenience
/// accessors for total return percentage and annualized return.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HorizonResult {
    /// Full factor-decomposed P&L from the attribution framework.
    pub attribution: PnlAttribution,
    /// Initial instrument value at (market_t0, as_of_t0).
    pub initial_value: Money,
    /// Final instrument value at (market_t1, as_of_t1).
    pub terminal_value: Money,
    /// Number of calendar days in the horizon (`None` if no time-roll in spec).
    pub horizon_days: Option<i64>,
    /// Report from scenario engine application.
    pub scenario_report: ApplicationReport,
}

impl HorizonAnalysis {
    /// Compute horizon total return under a scenario.
    ///
    /// Applies the [`ScenarioSpec`] to the provided market context (cloned
    /// internally) and runs P&L attribution between the original and
    /// scenario-modified states.
    ///
    /// The spec may include a [`crate::spec::OperationSpec::TimeRollForward`]
    /// to define the holding period.  If no time-roll is present, the
    /// analysis is a pure mark-to-scenario (carry will be zero, `horizon_days`
    /// will be `None`).
    ///
    /// Horizon analysis supports market-state scenarios. It rejects operations
    /// that mutate the instrument collection itself, including instrument
    /// price/spread shocks and structured-credit correlation shocks, because
    /// attribution prices the same instrument instance at both `t0` and `t1`.
    /// Apply those instrument-scoped changes before calling this method, or use
    /// a market-only horizon scenario.
    ///
    /// # Errors
    ///
    /// Returns an error if scenario application or attribution fails (e.g.
    /// missing market data for a curve referenced in the spec), or if the
    /// scenario contains an instrument-scoped operation unsupported by horizon
    /// attribution.
    pub fn compute(
        &self,
        instrument: &Arc<dyn Instrument>,
        market_t0: &MarketContext,
        as_of_t0: Date,
        scenario: &ScenarioSpec,
    ) -> crate::Result<HorizonResult> {
        if let Some(op_name) = horizon_unsupported_instrument_operation(scenario) {
            return Err(crate::Error::Validation(format!(
                "{op_name} is not supported by HorizonAnalysis because attribution uses one \
                 instrument instance for both t0 and t1 pricing. Apply instrument-scoped \
                 shocks before calling horizon analysis or use a market-only horizon scenario."
            )));
        }

        // 1. Price at t0
        let initial_value = instrument.value(market_t0, as_of_t0)?;

        // 2. Clone market and build a market-only execution context
        let mut market_t1 = market_t0.clone();
        let mut ctx = ExecutionContext {
            market: &mut market_t1,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: as_of_t0,
        };

        // 3. Apply scenario
        let scenario_report = self.engine.apply(scenario, &mut ctx)?;
        let as_of_t1 = ctx.as_of;

        // 4. Derive horizon
        let diff_days = (as_of_t1 - as_of_t0).whole_days();
        let horizon_days = if diff_days > 0 { Some(diff_days) } else { None };

        // 5. Run attribution
        let attribution =
            self.run_attribution(instrument, market_t0, &market_t1, as_of_t0, as_of_t1)?;

        // 6. Price at t1
        let terminal_value = instrument.value(&market_t1, as_of_t1)?;

        Ok(HorizonResult {
            attribution,
            initial_value,
            terminal_value,
            horizon_days,
            scenario_report,
        })
    }

    /// Dispatch to the appropriate attribution function based on `self.attribution_method`.
    fn run_attribution(
        &self,
        instrument: &Arc<dyn Instrument>,
        market_t0: &MarketContext,
        market_t1: &MarketContext,
        as_of_t0: Date,
        as_of_t1: Date,
    ) -> crate::Result<PnlAttribution> {
        let result = match &self.attribution_method {
            AttributionMethod::Parallel => attribute_pnl_parallel(
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                &self.config,
                None,
            ),
            AttributionMethod::Waterfall(order) => attribute_pnl_waterfall(
                instrument,
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
                &self.config,
                order.clone(),
                false,
                None,
            ),
            AttributionMethod::MetricsBased => {
                let metrics = default_attribution_metrics();
                let val_t0 = instrument.price_with_metrics(
                    market_t0,
                    as_of_t0,
                    &metrics,
                    PricingOptions::default(),
                )?;
                let val_t1 = instrument.price_with_metrics(
                    market_t1,
                    as_of_t1,
                    &metrics,
                    PricingOptions::default(),
                )?;
                attribute_pnl_metrics_based(
                    instrument, market_t0, market_t1, &val_t0, &val_t1, as_of_t0, as_of_t1,
                )
            }
            AttributionMethod::Taylor(config) => {
                attribute_pnl_taylor(instrument, market_t0, market_t1, as_of_t0, as_of_t1, config)
            }
        };
        Ok(result?)
    }
}

impl HorizonResult {
    /// Total return as a decimal fraction (e.g. `0.05` = 5%).
    ///
    /// Computed as `total_pnl / initial_value`. Returns:
    /// - `0.0` when `initial_value` is zero (to avoid division by zero), and
    /// - [`f64::NAN`] when `initial_value` and `total_pnl` are denominated in
    ///   different currencies. Multi-currency results require an explicit
    ///   base-currency conversion policy that this helper does not apply.
    pub fn total_return_pct(&self) -> f64 {
        if self.initial_value.currency() != self.attribution.total_pnl.currency() {
            return f64::NAN;
        }
        let iv = self.initial_value.amount();
        if iv == 0.0 {
            return 0.0;
        }
        self.attribution.total_pnl.amount() / iv
    }

    /// Annualized total return.
    ///
    /// Uses `(1 + total_return_pct)^(365 / horizon_days) - 1`.
    ///
    /// Returns `None` when there is no time-roll in the scenario, when total
    /// return is not finite (e.g. multi-currency result), or when the
    /// compounded result would not be finite. Total returns at or below
    /// `-100%` short-circuit to `Some(-1.0)` (a total loss stays a total loss
    /// under any positive compounding exponent).
    pub fn annualized_return(&self) -> Option<f64> {
        let days = self.horizon_days? as f64;
        if days <= 0.0 {
            return None;
        }
        let tr = self.total_return_pct();
        if !tr.is_finite() {
            return None;
        }
        if tr <= -1.0 {
            return Some(-1.0);
        }
        let out = (1.0 + tr).powf(365.0 / days) - 1.0;
        out.is_finite().then_some(out)
    }

    /// A single factor's P&L as a fraction of initial value.
    ///
    /// Returns `0.0` if initial value is zero and [`f64::NAN`] if the factor
    /// P&L currency does not match the initial value currency.
    pub fn factor_contribution(&self, factor: &AttributionFactor) -> f64 {
        let factor_money = match factor {
            AttributionFactor::Carry => &self.attribution.carry,
            AttributionFactor::RatesCurves => &self.attribution.rates_curves_pnl,
            AttributionFactor::CreditCurves => &self.attribution.credit_curves_pnl,
            AttributionFactor::InflationCurves => &self.attribution.inflation_curves_pnl,
            AttributionFactor::Correlations => &self.attribution.correlations_pnl,
            AttributionFactor::Fx => &self.attribution.fx_pnl,
            AttributionFactor::Volatility => &self.attribution.vol_pnl,
            AttributionFactor::ModelParameters => &self.attribution.model_params_pnl,
            AttributionFactor::MarketScalars => &self.attribution.market_scalars_pnl,
        };
        if self.initial_value.currency() != factor_money.currency() {
            return f64::NAN;
        }
        let iv = self.initial_value.amount();
        if iv == 0.0 {
            return 0.0;
        }
        factor_money.amount() / iv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use finstack_valuations::instruments::fixed_income::bond::CashflowSpec;
    use finstack_valuations::instruments::pricing_overrides::PricingOverrides;
    use finstack_valuations::instruments::{Attributes, Bond};
    use time::macros::date;

    /// Build a simple 2-year fixed-rate bond for testing.
    fn test_bond(base_date: Date) -> crate::Result<Arc<dyn Instrument>> {
        let bond = Bond::builder()
            .id("TEST-BOND".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(base_date)
            .maturity(base_date + time::Duration::days(730))
            .cashflow_spec(
                CashflowSpec::fixed(
                    0.05,
                    finstack_core::dates::Tenor::annual(),
                    DayCount::Thirty360,
                )
                .expect("finite test coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(None)
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()?;
        Ok(Arc::new(bond))
    }

    /// Build a market with a flat discount curve.
    fn test_market(base_date: Date) -> crate::Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.90)])
            .build()?;
        Ok(MarketContext::new().insert(curve))
    }

    #[test]
    fn no_op_scenario_returns_zero_pnl() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "no_op".into(),
            name: None,
            description: None,
            operations: vec![],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(
            result.attribution.total_pnl.amount().abs() < 1e-10,
            "no-op scenario should produce zero P&L, got {}",
            result.attribution.total_pnl.amount()
        );
        assert!(result.horizon_days.is_none());
        assert!(result.annualized_return().is_none());
        assert!((result.total_return_pct()).abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn time_roll_only_has_horizon_days_and_carry() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_1m".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::TimeRollForward {
                period: "1M".into(),
                apply_shocks: false,
                roll_mode: crate::TimeRollMode::BusinessDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(matches!(result.horizon_days, Some(days) if days > 0));
        assert!(result.annualized_return().is_some());

        // Carry should be non-zero since the bond accrues over the holding period.
        let carry = result.attribution.carry.amount();
        assert!(
            carry.abs() > 1e-6,
            "time-roll only: carry should be non-zero, got {carry}"
        );

        // Factor decomposition should be coherent
        let a = &result.attribution;
        let sum_of_factors = a.carry.amount()
            + a.rates_curves_pnl.amount()
            + a.credit_curves_pnl.amount()
            + a.inflation_curves_pnl.amount()
            + a.correlations_pnl.amount()
            + a.fx_pnl.amount()
            + a.vol_pnl.amount()
            + a.cross_factor_pnl.amount()
            + a.model_params_pnl.amount()
            + a.market_scalars_pnl.amount()
            + a.residual.amount();
        assert!(
            (a.total_pnl.amount() - sum_of_factors).abs() < 1e-8,
            "factors + residual ({sum_of_factors}) should equal total ({})",
            a.total_pnl.amount()
        );
        Ok(())
    }

    #[test]
    fn combined_time_roll_and_shock() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "roll_and_shock".into(),
            name: None,
            description: None,
            operations: vec![
                crate::OperationSpec::TimeRollForward {
                    period: "1M".into(),
                    apply_shocks: true,
                    roll_mode: crate::TimeRollMode::BusinessDays,
                },
                crate::OperationSpec::CurveParallelBp {
                    curve_kind: crate::CurveKind::Discount,
                    curve_id: "USD-OIS".into(),
                    discount_curve_id: None,
                    bp: 50.0,
                },
            ],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        // Horizon present
        assert!(result.horizon_days.is_some());

        // Both carry and rates should be non-zero
        assert!(
            result.attribution.carry.amount().abs() > 1e-6,
            "combined: carry should be non-zero"
        );
        assert!(
            result.attribution.rates_curves_pnl.amount().abs() > 1e-6,
            "combined: rates P&L should be non-zero"
        );

        // Factor decomposition should be coherent: sum of factors + residual = total
        let a = &result.attribution;
        let sum_of_factors = a.carry.amount()
            + a.rates_curves_pnl.amount()
            + a.credit_curves_pnl.amount()
            + a.inflation_curves_pnl.amount()
            + a.correlations_pnl.amount()
            + a.fx_pnl.amount()
            + a.vol_pnl.amount()
            + a.cross_factor_pnl.amount()
            + a.model_params_pnl.amount()
            + a.market_scalars_pnl.amount()
            + a.residual.amount();
        assert!(
            (a.total_pnl.amount() - sum_of_factors).abs() < 1e-8,
            "factors + residual ({sum_of_factors}) should equal total ({})",
            a.total_pnl.amount()
        );
        Ok(())
    }

    #[test]
    fn shock_only_has_no_horizon_and_zero_carry() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "rate_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::CurveParallelBp {
                curve_kind: crate::CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 50.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        assert!(result.horizon_days.is_none());
        assert!(result.annualized_return().is_none());

        assert!(
            result.attribution.carry.amount().abs() < 1e-10,
            "shock-only: carry should be zero, got {}",
            result.attribution.carry.amount()
        );

        assert!(
            result.attribution.rates_curves_pnl.amount().abs() > 1e-6,
            "shock-only: rates P&L should be non-zero"
        );
        Ok(())
    }

    #[test]
    fn instrument_scoped_shocks_are_rejected_in_horizon_analysis() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "instrument_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::InstrumentPricePctByType {
                instrument_types: vec![finstack_valuations::pricer::InstrumentType::Bond],
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let err = analyzer
            .compute(&instrument, &market, as_of, &scenario)
            .expect_err("instrument-scoped horizon shocks should fail loudly");

        assert!(
            err.to_string().contains("InstrumentPricePctByType")
                && err.to_string().contains("not supported by HorizonAnalysis"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn total_return_pct_matches_pnl_over_initial() -> crate::Result<()> {
        let as_of = date!(2025 - 01 - 15);
        let instrument = test_bond(as_of)?;
        let market = test_market(as_of)?;

        let scenario = ScenarioSpec {
            id: "rate_shock".into(),
            name: None,
            description: None,
            operations: vec![crate::OperationSpec::CurveParallelBp {
                curve_kind: crate::CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 50.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let analyzer = HorizonAnalysis::default();
        let result = analyzer.compute(&instrument, &market, as_of, &scenario)?;

        let expected_pct = result.attribution.total_pnl.amount() / result.initial_value.amount();
        assert!(
            (result.total_return_pct() - expected_pct).abs() < 1e-12,
            "total_return_pct() should match manual calculation"
        );
        Ok(())
    }

    fn empty_report() -> ApplicationReport {
        ApplicationReport {
            operations_applied: 0,
            user_operations: 0,
            expanded_operations: 0,
            warnings: vec![],
            rounding_context: None,
        }
    }

    fn synthetic_result(
        initial_ccy: Currency,
        initial_amount: f64,
        pnl_ccy: Currency,
        pnl_amount: f64,
        days: i64,
    ) -> HorizonResult {
        let attribution = PnlAttribution::new(
            Money::new(pnl_amount, pnl_ccy),
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 15) + time::Duration::days(days),
            AttributionMethod::Parallel,
        );
        HorizonResult {
            attribution,
            initial_value: Money::new(initial_amount, initial_ccy),
            terminal_value: Money::new(initial_amount + pnl_amount, initial_ccy),
            horizon_days: Some(days),
            scenario_report: empty_report(),
        }
    }

    /// Currency mismatch between initial value and total P&L must surface as
    /// NaN rather than a silently wrong ratio.
    #[test]
    fn total_return_pct_currency_mismatch_returns_nan() {
        let result = synthetic_result(Currency::USD, 100.0, Currency::EUR, 10.0, 30);

        assert!(result.total_return_pct().is_nan());
        assert!(result.annualized_return().is_none());
        assert!(result
            .factor_contribution(&AttributionFactor::Carry)
            .is_nan());
    }

    /// A total loss (or worse) must collapse to `-100%` annualized rather
    /// than returning `NaN` from `powf` on a non-positive base.
    #[test]
    fn annualized_return_total_loss_returns_minus_one() {
        let total_loss = synthetic_result(Currency::USD, 100.0, Currency::USD, -100.0, 30);
        assert_eq!(total_loss.total_return_pct(), -1.0);
        assert_eq!(total_loss.annualized_return(), Some(-1.0));

        // Worse-than-total-loss (e.g. short book reported as -120%) must also
        // resolve to -100%, not NaN.
        let blown_up = synthetic_result(Currency::USD, 100.0, Currency::USD, -120.0, 30);
        assert_eq!(blown_up.annualized_return(), Some(-1.0));
    }
}
