//! Breakeven parameter shift calculator.
//!
//! Computes how much a valuation parameter (spread, yield, vol, correlation)
//! can move before carry + roll-down is wiped out over the configured horizon.

use crate::metrics::sensitivities::theta::calculate_theta_date;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;

/// Minimum absolute sensitivity value below which breakeven is undefined.
const SENSITIVITY_FLOOR: f64 = 1e-12;

/// Which valuation parameter to solve the breakeven for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BreakevenTarget {
    /// Z-spread breakeven (sensitivity: CS01).
    ZSpread,
    /// Yield-to-maturity breakeven (sensitivity: DV01).
    Ytm,
    /// Implied volatility breakeven (sensitivity: Vega).
    ImpliedVol,
    /// Base correlation breakeven (sensitivity: Correlation01).
    BaseCorrelation,
    /// OAS breakeven (sensitivity: CS01).
    Oas,
}

impl BreakevenTarget {
    /// Returns the sensitivity [`MetricId`] used to compute the linear breakeven.
    pub fn sensitivity_metric(&self) -> MetricId {
        match self {
            Self::ZSpread | Self::Oas => MetricId::Cs01,
            Self::Ytm => MetricId::Dv01,
            Self::ImpliedVol => MetricId::Vega,
            Self::BaseCorrelation => MetricId::Correlation01,
        }
    }
}

/// Linear (first-order) or iterative (full-reprice root-find) solve mode.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BreakevenMode {
    /// `-(carry_total) / sensitivity`. Fast, ignores convexity.
    #[default]
    Linear,
    /// Brent root-find with full reprice at horizon. Accounts for convexity.
    Iterative,
}

/// Configuration for the breakeven calculator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct BreakevenConfig {
    /// Which valuation parameter to solve for.
    pub target: BreakevenTarget,
    /// Solve mode (default: linear).
    #[serde(default)]
    pub mode: BreakevenMode,
}

/// Computes breakeven parameter shift from carry and sensitivity.
pub(crate) struct BreakevenCalculator;

impl MetricCalculator for BreakevenCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let config = context
            .get_metric_overrides()
            .and_then(|o| o.breakeven_config)
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: "breakeven_config: set BreakevenConfig on MetricPricingOverrides".into(),
            })?;

        let carry_total = context
            .computed
            .get(&MetricId::CarryTotal)
            .copied()
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: "metric:carry_total".into(),
            })?;

        let sensitivity_id = config.target.sensitivity_metric();
        let sensitivity = context
            .computed
            .get(&sensitivity_id)
            .copied()
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: format!(
                    "metric:{}: compute {} alongside Breakeven",
                    sensitivity_id, sensitivity_id,
                ),
            })?;

        if sensitivity.abs() < SENSITIVITY_FLOOR {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        match config.mode {
            BreakevenMode::Linear => Ok(-carry_total / sensitivity),
            BreakevenMode::Iterative => {
                if config.target == BreakevenTarget::BaseCorrelation {
                    return Err(finstack_quant_core::Error::Validation(
                        "BreakevenMode::Iterative is unsupported for BaseCorrelation because no scalar base-correlation bump API exists".to_string(),
                    ));
                }
                iterative_breakeven(context, carry_total, sensitivity, &config)
            }
        }
    }

    fn dependencies(&self) -> &[MetricId] {
        static DEPS: &[MetricId] = &[MetricId::CarryTotal];
        DEPS
    }
}

/// Bump a market context by `delta` for the given breakeven target.
///
/// Returns the bumped [`MarketContext`] or an error if the required
/// curve / surface cannot be determined.
fn bump_market_for_target(
    context: &MetricContext,
    delta: f64,
    target: BreakevenTarget,
) -> Result<MarketContext> {
    match target {
        BreakevenTarget::ZSpread | BreakevenTarget::Oas | BreakevenTarget::Ytm => {
            // Use the instrument's first discount curve, falling back to
            // the cached `discount_curve_id` on the context.
            let curve_id = context
                .instrument
                .market_dependencies()
                .ok()
                .and_then(|d| d.curves.discount_curves.first().cloned())
                .or_else(|| context.discount_curve_id.clone())
                .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                    id: "iterative_breakeven: no discount curve found for instrument".into(),
                })?;
            crate::metrics::bump_discount_curve_parallel(context.curves.as_ref(), &curve_id, delta)
        }
        BreakevenTarget::ImpliedVol => {
            let vol_surface_id = context
                .instrument
                .market_dependencies()
                .ok()
                .and_then(|d| d.unique_vol_surface_ids().into_iter().next())
                .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                    id: "iterative_breakeven: no vol surface found for instrument".into(),
                })?;
            // `delta` is the breakeven expressed in **vol points** (consistent
            // with `initial_guess = -carry / Vega_per_vol_point` and the Linear
            // mode output). One vol point = 0.01 absolute vol, so convert vol
            // points -> absolute vol with * 0.01. (Using * 0.0001 applied only
            // 1/100th of a vol point per unit, making the iterative implied-vol
            // breakeven ~100x the Linear value.)
            let bump_abs = delta * 0.01;
            crate::metrics::bump_surface_vol_absolute(
                context.curves.as_ref(),
                vol_surface_id.as_str(),
                bump_abs,
            )
        }
        BreakevenTarget::BaseCorrelation => Err(finstack_quant_core::InputError::NotFound {
            id: "iterative_breakeven: BaseCorrelation has no scalar bump API".into(),
        }
        .into()),
    }
}

/// Iterative breakeven using Brent root-finding.
///
/// Finds the parameter shift `delta` such that:
///   carry_total + PV(bumped market, rolled_date) - base_pv_at_horizon = 0
fn iterative_breakeven(
    context: &MetricContext,
    carry_total: f64,
    sensitivity: f64,
    config: &BreakevenConfig,
) -> Result<f64> {
    // Determine the horizon date (same convention as carry decomposition).
    let period_str = context
        .get_metric_overrides()
        .and_then(|o| o.theta_period.as_deref())
        .unwrap_or("1D");
    let expiry_date = context.instrument.expiry();
    let rolled_date = calculate_theta_date(context.as_of, period_str, expiry_date)?;

    // Base PV at the horizon with current (un-bumped) curves.
    let base_pv_at_horizon = context
        .instrument_value_with_scenario(context.curves.as_ref(), rolled_date)?
        .amount();

    // Linear estimate as initial guess.
    let initial_guess = -carry_total / sensitivity;

    let target = config.target;
    let objective = |delta: f64| -> f64 {
        let Ok(bumped_market) = bump_market_for_target(context, delta, target) else {
            return f64::NAN;
        };
        let Ok(pv) = context.instrument_value_with_scenario(&bumped_market, rolled_date) else {
            return f64::NAN;
        };
        carry_total + pv.amount() - base_pv_at_horizon
    };

    BrentSolver::new()
        .tolerance(1e-8)
        .max_iterations(50)
        .solve(objective, initial_guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Bond;
    use crate::instruments::{BreakevenConfig, BreakevenMode, BreakevenTarget};
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::macros::date;

    fn flat_discount_curve(
        id: &str,
        rate: f64,
        base_date: finstack_quant_core::dates::Date,
    ) -> DiscountCurve {
        let knots: Vec<(f64, f64)> = (0..=20)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, (-rate * t).exp())
            })
            .collect();
        DiscountCurve::builder(id)
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(knots)
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("flat discount curve")
    }

    fn context_with_carry_and_sensitivity(
        carry_total: f64,
        sensitivity: f64,
        target: BreakevenTarget,
        mode: BreakevenMode,
    ) -> MetricContext {
        let as_of = date!(2025 - 01 - 15);
        let bond = Bond::fixed(
            "TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("pv");

        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        ctx.computed.insert(MetricId::CarryTotal, carry_total);
        ctx.computed
            .insert(target.sensitivity_metric(), sensitivity);

        let overrides = crate::instruments::MetricPricingOverrides::default()
            .with_breakeven_config(BreakevenConfig { target, mode });
        ctx.set_metric_overrides(Some(overrides));
        ctx
    }

    #[test]
    fn test_linear_breakeven_positive_carry() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.50,
            -0.04,
            BreakevenTarget::ZSpread,
            BreakevenMode::Linear,
        );
        let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
        assert!((result - 12.5).abs() < 1e-10, "got {result}");
    }

    #[test]
    fn test_linear_breakeven_negative_carry() {
        let mut ctx = context_with_carry_and_sensitivity(
            -0.30,
            -0.04,
            BreakevenTarget::ZSpread,
            BreakevenMode::Linear,
        );
        let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
        assert!((result - (-7.5)).abs() < 1e-10, "got {result}");
    }

    #[test]
    fn test_linear_breakeven_zero_sensitivity_returns_error() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.50,
            0.0,
            BreakevenTarget::ZSpread,
            BreakevenMode::Linear,
        );
        let result = BreakevenCalculator.calculate(&mut ctx);
        assert!(result.is_err(), "zero sensitivity should error");
    }

    #[test]
    fn test_missing_sensitivity_returns_error() {
        let as_of = date!(2025 - 01 - 15);
        let bond = Bond::fixed(
            "TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("pv");

        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        ctx.computed.insert(MetricId::CarryTotal, 0.50);
        let overrides = crate::instruments::MetricPricingOverrides::default()
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Linear,
            });
        ctx.set_metric_overrides(Some(overrides));

        let result = BreakevenCalculator.calculate(&mut ctx);
        assert!(result.is_err(), "missing sensitivity should error");
    }

    #[test]
    fn test_missing_config_returns_error() {
        let as_of = date!(2025 - 01 - 15);
        let bond = Bond::fixed(
            "TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("pv");

        let mut ctx = MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        );
        ctx.computed.insert(MetricId::CarryTotal, 0.50);
        ctx.computed.insert(MetricId::Cs01, -0.04);

        let result = BreakevenCalculator.calculate(&mut ctx);
        assert!(result.is_err(), "missing breakeven config should error");
    }

    #[test]
    fn test_linear_breakeven_ytm_target() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.25,
            -0.05,
            BreakevenTarget::Ytm,
            BreakevenMode::Linear,
        );
        let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");
        assert!((result - 5.0).abs() < 1e-10, "got {result}");
    }

    #[test]
    fn test_linear_breakeven_base_correlation_target() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.25,
            -0.02,
            BreakevenTarget::BaseCorrelation,
            BreakevenMode::Linear,
        );

        let result = BreakevenCalculator.calculate(&mut ctx).expect("breakeven");

        assert!((result - 12.5).abs() < 1e-10, "got {result}");
    }

    #[test]
    fn test_iterative_breakeven_base_correlation_is_explicitly_unsupported() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.25,
            -0.02,
            BreakevenTarget::BaseCorrelation,
            BreakevenMode::Iterative,
        );

        let err = BreakevenCalculator
            .calculate(&mut ctx)
            .expect_err("iterative base-correlation breakeven should be unsupported");

        assert!(
            err.to_string().contains("BaseCorrelation")
                && err.to_string().contains("Iterative")
                && err.to_string().contains("unsupported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_breakeven_via_standard_registry() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::MetricPricingOverrides;
        use crate::instruments::PricingOptions;

        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "CARRY-TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");

        bond.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("6M")
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Linear,
            });

        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("price_with_metrics should succeed");

        let carry = result
            .measures
            .get(MetricId::CarryTotal.as_str())
            .copied()
            .expect("carry_total");
        let cs01 = result
            .measures
            .get(MetricId::Cs01.as_str())
            .copied()
            .expect("cs01");
        let breakeven = result
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("breakeven");

        // Verify: breakeven = -carry / cs01
        let expected = -carry / cs01;
        assert!(
            (breakeven - expected).abs() < 1e-8,
            "breakeven={breakeven}, expected={expected}, carry={carry}, cs01={cs01}"
        );
    }

    #[test]
    fn test_breakeven_horizon_matches_carry_horizon() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::MetricPricingOverrides;
        use crate::instruments::PricingOptions;

        let as_of = date!(2025 - 01 - 15);

        // Compute with 1M horizon
        let mut bond_1m = Bond::fixed(
            "HORIZON-TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");

        bond_1m.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("1M")
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Linear,
            });

        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

        let result_1m = bond_1m
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("1m result");

        // Compute with 6M horizon
        let mut bond_6m = Bond::fixed(
            "HORIZON-TEST",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");

        bond_6m.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("6M")
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Linear,
            });

        let result_6m = bond_6m
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::CarryTotal, MetricId::Cs01, MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("6m result");

        let be_1m = result_1m
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("be_1m");
        let be_6m = result_6m
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("be_6m");

        // 6M carry > 1M carry, so 6M breakeven should be larger (more room to widen)
        assert!(
            be_6m.abs() > be_1m.abs(),
            "6M breakeven ({be_6m}) should have larger magnitude than 1M ({be_1m})"
        );
    }
}
