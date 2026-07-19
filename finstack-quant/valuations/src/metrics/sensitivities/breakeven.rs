//! Breakeven parameter shift calculator.
//!
//! Computes how much a valuation parameter (spread, yield, vol, correlation)
//! can move before carry + roll-down is wiped out over the configured horizon.

use crate::metrics::sensitivities::theta::calculate_theta_date;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::Result;
use std::borrow::Cow;
use std::cell::RefCell;

/// Minimum absolute sensitivity value below which breakeven is undefined.
const SENSITIVITY_FLOOR: f64 = 1e-12;

/// Which valuation parameter to solve the breakeven for.
///
/// # Result units
///
/// The breakeven metric is a bare `f64` whose unit depends on the target. Read
/// the per-variant docs before interpreting a value:
///
/// | Target             | Sensitivity     | Result unit          |
/// |--------------------|-----------------|----------------------|
/// | `ZSpread`          | CS01            | basis points         |
/// | `Ytm`              | DV01            | basis points         |
/// | `Oas`              | CS01            | basis points         |
/// | `ImpliedVol`       | Vega            | vol points (1 = 1%)  |
/// | `BaseCorrelation`  | Correlation01   | correlation points   |
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BreakevenTarget {
    /// Z-spread breakeven, in **basis points** (sensitivity: CS01).
    ZSpread,
    /// Yield-to-maturity breakeven, in **basis points** (sensitivity: DV01).
    Ytm,
    /// Implied volatility breakeven, in **vol points** where 1.0 = 1% absolute
    /// vol (sensitivity: Vega).
    ImpliedVol,
    /// Base correlation breakeven, in **correlation points** (sensitivity:
    /// Correlation01).
    ///
    /// Only [`BreakevenMode::Linear`] is supported. Base-correlation skew is
    /// strongly non-linear, so a first-order breakeven here is a coarser
    /// approximation than for spread or yield targets.
    BaseCorrelation,
    /// OAS breakeven, in **basis points** (sensitivity: CS01).
    ///
    /// Note that under [`BreakevenMode::Iterative`] the solve applies a
    /// parallel discount-curve shift. For an instrument with embedded
    /// optionality that is a duration-space answer, not a true OAS shift,
    /// because OAS is defined relative to the option model.
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
///
/// # Why the two modes disagree
///
/// The gap between them is usually **not** dominated by convexity. `Linear`
/// divides by the sensitivity measured at `as_of`, whereas `Iterative`
/// reprices at the horizon date, where the instrument has less remaining time
/// and therefore a different sensitivity. On a 5Y bond over a 6M horizon the
/// two differ by several percent even though convexity over the ~9bp solved
/// shift contributes only a fraction of that. The gap grows with horizon
/// length, not just with curvature.
///
/// `Iterative` is the more accurate answer where it is supported; `Linear` is
/// the fast approximation and the default.
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
    /// `-(carry_total) / sensitivity`, using the sensitivity at `as_of`.
    ///
    /// Fast, first-order: ignores both convexity and the change in sensitivity
    /// over the horizon.
    #[default]
    Linear,
    /// Brent root-find with a full reprice at the horizon date.
    ///
    /// Captures convexity *and* the horizon change in sensitivity. Not
    /// supported for [`BreakevenTarget::BaseCorrelation`], nor for
    /// credit-curve instruments under `ZSpread`/`Oas` — see
    /// [`BreakevenTarget`] and the calculator docs for why.
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
            return Err(finstack_quant_core::Error::Validation(format!(
                "Breakeven for {:?} is undefined: sensitivity metric {} is {} (below the \
                 {SENSITIVITY_FLOOR:e} floor), so no finite parameter shift offsets carry. \
                 This typically means the instrument has no exposure to the chosen target \
                 (e.g. a ZSpread breakeven on a credit-risk-free instrument).",
                config.target, sensitivity_id, sensitivity,
            )));
        }

        match config.mode {
            BreakevenMode::Linear => Ok(-carry_total / sensitivity),
            BreakevenMode::Iterative => {
                if config.target == BreakevenTarget::BaseCorrelation {
                    return Err(finstack_quant_core::Error::Validation(
                        "BreakevenMode::Iterative is unsupported for BaseCorrelation because no scalar base-correlation bump API exists".to_string(),
                    ));
                }
                ensure_iterative_bump_matches_sensitivity(context, config.target)?;
                iterative_breakeven(context, carry_total, sensitivity, &config)
            }
        }
    }

    fn dependencies(&self) -> &[MetricId] {
        static DEPS: &[MetricId] = &[MetricId::CarryTotal];
        DEPS
    }

    /// Declares the configured target's sensitivity metric alongside
    /// [`MetricId::CarryTotal`].
    ///
    /// The sensitivity is read from `context.computed` in
    /// [`calculate`](MetricCalculator::calculate) but is only knowable at
    /// runtime, from [`BreakevenConfig::target`]. Declaring it statically is
    /// impossible (the target varies) and declaring the union of all four
    /// candidates would force e.g. `Vega` computation on instruments with no
    /// vol surface, which is a hard error in the registry. Hence the dynamic
    /// hook.
    ///
    /// Without this, whether `Breakeven` succeeded depended on the *order* the
    /// caller listed metrics in: `[Cs01, Breakeven]` worked while
    /// `[Breakeven, Cs01]` and a bare `[Breakeven]` both failed.
    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Some(config) = context
            .get_metric_overrides()
            .and_then(|o| o.breakeven_config)
        else {
            // No config: `calculate` will raise the descriptive error. Fall
            // back to the static set so ordering stays well-defined.
            return Cow::Borrowed(self.dependencies());
        };
        Cow::Owned(vec![
            MetricId::CarryTotal,
            config.target.sensitivity_metric(),
        ])
    }
}

/// Reject [`BreakevenMode::Iterative`] when the solver's bump would move a
/// different risk factor than the sensitivity metric measures.
///
/// `iterative_breakeven` shifts the **discount curve** for `ZSpread`/`Oas`.
/// That matches `CS01` only when `CS01` itself resolved to a z-spread bump,
/// which is the no-credit-curve fallback. When the instrument carries a credit
/// curve, `CS01` is a **hazard-curve** bump (par spreads re-bootstrapped), so a
/// discount-curve solve would answer "bp of discount-rate rise" while the
/// linear mode answers "bp of credit-spread widening" — under one metric name,
/// with the credit-space linear estimate used to seed the rates-space solve.
///
/// Rather than silently return a mislabeled number we reject, mirroring the
/// existing [`BreakevenTarget::BaseCorrelation`] rejection. Use
/// [`BreakevenMode::Linear`] for credit-curve instruments.
fn ensure_iterative_bump_matches_sensitivity(
    context: &MetricContext,
    target: BreakevenTarget,
) -> Result<()> {
    if !matches!(target, BreakevenTarget::ZSpread | BreakevenTarget::Oas) {
        return Ok(());
    }
    let has_credit_curve = context
        .instrument
        .market_dependencies()
        .map(|d| !d.curves.credit_curves.is_empty())
        .unwrap_or(false);
    if has_credit_curve {
        return Err(finstack_quant_core::Error::Validation(format!(
            "BreakevenMode::Iterative is unsupported for {target:?} on an instrument with a \
             credit curve: CS01 is measured as a hazard-curve (par spread) bump while the \
             iterative solve applies a parallel discount-curve shift, so the two modes would \
             report different risk factors under one metric name. Use BreakevenMode::Linear."
        )));
    }
    Ok(())
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
            let vol_surface_ids = context
                .instrument
                .market_dependencies()
                .ok()
                .map(|dependencies| {
                    dependencies
                        .unique_vol_surface_ids()
                        .into_iter()
                        .filter(|surface_id| {
                            context.curves.get_surface(surface_id.as_str()).is_ok()
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|surface_ids| !surface_ids.is_empty())
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
            crate::metrics::core::finite_difference::bump_surfaces_vol_absolute(
                context.curves.as_ref(),
                &vol_surface_ids,
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

    // Brent rejects non-finite evaluations, so a NaN here surfaces as a generic
    // "non-finite value" convergence error that hides the real cause (e.g. "no
    // vol surface found"). Capture the first underlying error and re-raise it.
    let first_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);
    let record = |err: finstack_quant_core::Error| {
        let mut slot = first_error.borrow_mut();
        if slot.is_none() {
            *slot = Some(err);
        }
        f64::NAN
    };

    let objective = |delta: f64| -> f64 {
        let bumped_market = match bump_market_for_target(context, delta, target) {
            Ok(market) => market,
            Err(err) => return record(err),
        };
        match context.instrument_value_with_scenario(&bumped_market, rolled_date) {
            Ok(pv) => carry_total + pv.amount() - base_pv_at_horizon,
            Err(err) => record(err),
        }
    };

    // No `BracketHint` is set deliberately. Core's hints are sized in absolute
    // rate units (`Spread` = 0.005, `Ytm` = 0.02) whereas `delta` here is in
    // basis points / vol points, so a hint would size the initial bracket ~100x
    // too small. The adaptive default (1% of the initial guess, floored at
    // 0.01) is correctly scaled for this objective.
    let solved = BrentSolver::new()
        .tolerance(1e-8)
        .max_iterations(50)
        .solve(objective, initial_guess);

    match first_error.into_inner() {
        // Prefer the underlying cause over the solver's non-finite report.
        Some(err) if solved.is_err() => Err(err),
        _ => solved,
    }
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

    /// Build a bond configured for breakeven, priced off a flat 4% curve.
    fn breakeven_bond(id: &str, target: BreakevenTarget, mode: BreakevenMode) -> Bond {
        use crate::instruments::MetricPricingOverrides;
        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            id,
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        bond.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("6M")
            .with_breakeven_config(BreakevenConfig { target, mode });
        bond
    }

    /// The sensitivity metric is declared via `dynamic_dependencies`, so the
    /// result must not depend on the order the caller lists metrics in.
    ///
    /// Regression: `dependencies()` declared only `CarryTotal`, so
    /// `[Cs01, Breakeven]` returned a value while `[Breakeven, Cs01]` and a
    /// bare `[Breakeven]` both failed with "compute cs01 alongside Breakeven".
    #[test]
    fn test_breakeven_is_independent_of_request_order() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::PricingOptions;

        let as_of = date!(2025 - 01 - 15);
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
        let bond = breakeven_bond("ORDER", BreakevenTarget::ZSpread, BreakevenMode::Linear);

        let breakeven_for = |ids: &[MetricId]| -> f64 {
            bond.price_with_metrics(&market, as_of, ids, PricingOptions::default())
                .unwrap_or_else(|e| panic!("price_with_metrics failed for {ids:?}: {e}"))
                .measures
                .get(MetricId::Breakeven.as_str())
                .copied()
                .unwrap_or_else(|| panic!("breakeven missing for {ids:?}"))
        };

        let sensitivity_first = breakeven_for(&[MetricId::Cs01, MetricId::Breakeven]);
        let breakeven_first = breakeven_for(&[MetricId::Breakeven, MetricId::Cs01]);
        let breakeven_alone = breakeven_for(&[MetricId::Breakeven]);

        assert!(
            sensitivity_first.is_finite() && sensitivity_first.abs() > 1e-9,
            "expected a meaningful breakeven, got {sensitivity_first}"
        );
        assert_eq!(sensitivity_first, breakeven_first);
        assert_eq!(sensitivity_first, breakeven_alone);
    }

    /// A `Ytm` breakeven depends on `Dv01`, not `Cs01`; the dynamic dependency
    /// must follow the configured target.
    #[test]
    fn test_breakeven_ytm_target_resolves_dv01_dependency_alone() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::PricingOptions;

        let as_of = date!(2025 - 01 - 15);
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));
        let bond = breakeven_bond("YTM-DEP", BreakevenTarget::Ytm, BreakevenMode::Linear);

        // Requesting Breakeven alone must succeed: the dynamic dependency has to
        // resolve Dv01 (not Cs01) for a Ytm target. The registry returns only
        // the metrics the caller asked for, so compare against an explicit run.
        let alone = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("Ytm breakeven should resolve Dv01 without being asked")
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("breakeven");

        let explicit = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Dv01, MetricId::CarryTotal, MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("explicit run");
        let breakeven = explicit
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("breakeven");
        let dv01 = explicit
            .measures
            .get(MetricId::Dv01.as_str())
            .copied()
            .expect("dv01");
        let carry = explicit
            .measures
            .get(MetricId::CarryTotal.as_str())
            .copied()
            .expect("carry_total");

        assert_eq!(
            alone, breakeven,
            "Breakeven must not depend on request list"
        );
        assert!((breakeven - (-carry / dv01)).abs() < 1e-8);
    }

    /// The iterative path had no passing-case coverage — the mode with the
    /// root-finder and the unit conversions. Pin it against the linear
    /// estimate and verify it actually zeroes the P&L objective.
    #[test]
    fn test_iterative_breakeven_zeroes_the_horizon_pnl() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::PricingOptions;

        let as_of = date!(2025 - 01 - 15);
        let market = MarketContext::new().insert(flat_discount_curve("USD-OIS", 0.04, as_of));

        let ids = &[MetricId::Cs01, MetricId::CarryTotal, MetricId::Breakeven];
        let linear = breakeven_bond("BE-LIN", BreakevenTarget::ZSpread, BreakevenMode::Linear)
            .price_with_metrics(&market, as_of, ids, PricingOptions::default())
            .expect("linear");
        let iterative = breakeven_bond(
            "BE-ITER",
            BreakevenTarget::ZSpread,
            BreakevenMode::Iterative,
        )
        .price_with_metrics(&market, as_of, ids, PricingOptions::default())
        .expect("iterative");

        let be_lin = linear
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("linear breakeven");
        let be_iter = iterative
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("iterative breakeven");
        let carry = iterative
            .measures
            .get(MetricId::CarryTotal.as_str())
            .copied()
            .expect("carry_total");

        assert!(be_iter.is_finite(), "iterative breakeven must be finite");
        // Same sign and same order of magnitude as the linear estimate: this is
        // the guard that would have caught the historical 100x vol-units bug.
        assert_eq!(
            be_iter.signum(),
            be_lin.signum(),
            "iterative ({be_iter}) and linear ({be_lin}) must agree in sign"
        );
        let ratio = be_iter / be_lin;
        assert!(
            (0.5..2.0).contains(&ratio),
            "iterative ({be_iter}) vs linear ({be_lin}) ratio {ratio} is out of plausible range; \
             a units error would show up here as ~100x"
        );

        // The solved shift must actually zero the objective:
        //   carry + PV(bumped, horizon) - PV(base, horizon) ~= 0
        let bond = breakeven_bond("BE-CHK", BreakevenTarget::ZSpread, BreakevenMode::Iterative);
        let rolled = calculate_theta_date(as_of, "6M", bond.expiry()).expect("horizon date");
        let base_pv = bond.value(&market, rolled).expect("base pv").amount();
        let bumped = crate::metrics::bump_discount_curve_parallel(
            &market,
            &finstack_quant_core::types::CurveId::new("USD-OIS"),
            be_iter,
        )
        .expect("bump");
        let bumped_pv = bond.value(&bumped, rolled).expect("bumped pv").amount();

        let residual = carry + bumped_pv - base_pv;
        assert!(
            residual.abs() < 1e-6,
            "solved shift {be_iter} should zero the objective, residual = {residual}"
        );
    }

    /// On a credit-curve instrument, CS01 is a hazard-curve (par spread) bump
    /// while the iterative solve shifts the discount curve. Reporting that as a
    /// "ZSpread breakeven" would mislabel the risk factor, so it must be
    /// rejected rather than silently returned.
    #[test]
    fn test_iterative_zspread_rejected_for_credit_curve_instrument() {
        use crate::instruments::common_impl::traits::Instrument;
        use crate::instruments::MetricPricingOverrides;
        use crate::instruments::PricingOptions;
        use finstack_quant_core::market_data::term_structures::HazardCurve;
        use finstack_quant_core::types::CurveId;

        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "CREDIT-BE",
            Money::new(100.0, Currency::USD),
            0.05,
            as_of,
            date!(2030 - 01 - 15),
            "USD-OIS",
        )
        .expect("bond");
        bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
        bond.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("6M")
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Iterative,
            });

        let hazard = HazardCurve::builder("USD-CREDIT")
            .base_date(as_of)
            .recovery_rate(0.4)
            .knots([(0.0, 0.02), (5.0, 0.02)])
            .build()
            .expect("hazard curve");
        let market = MarketContext::new()
            .insert(flat_discount_curve("USD-OIS", 0.04, as_of))
            .insert(hazard);

        let err = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect_err("iterative ZSpread on a credit bond should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("hazard-curve") && msg.contains("BreakevenMode::Linear"),
            "error should explain the risk-factor mismatch and the remedy: {msg}"
        );

        // Linear mode remains available for the same instrument.
        let mut linear_bond = bond;
        linear_bond.metric_pricing_overrides = MetricPricingOverrides::default()
            .with_theta_period("6M")
            .with_breakeven_config(BreakevenConfig {
                target: BreakevenTarget::ZSpread,
                mode: BreakevenMode::Linear,
            });
        let value = linear_bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Breakeven],
                PricingOptions::default(),
            )
            .expect("linear mode should still work for a credit bond")
            .measures
            .get(MetricId::Breakeven.as_str())
            .copied()
            .expect("breakeven");
        assert!(value.is_finite());
    }

    /// A near-zero sensitivity must explain itself rather than emitting the
    /// context-free "Invalid input data".
    #[test]
    fn test_zero_sensitivity_error_names_the_metric_and_value() {
        let mut ctx = context_with_carry_and_sensitivity(
            0.50,
            0.0,
            BreakevenTarget::ZSpread,
            BreakevenMode::Linear,
        );
        let err = BreakevenCalculator
            .calculate(&mut ctx)
            .expect_err("zero sensitivity should error");
        let msg = err.to_string();
        assert!(
            msg.contains("ZSpread") && msg.contains("cs01") && msg.contains("undefined"),
            "error should name target, sensitivity metric and cause: {msg}"
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

        // Pin the relationship rather than only its direction: both are
        // `-carry / cs01` against the same CS01, so the breakeven ratio must
        // track the carry ratio. A bare inequality passes for a wide range of
        // wrong implementations.
        let carry_1m = result_1m
            .measures
            .get(MetricId::CarryTotal.as_str())
            .copied()
            .expect("carry_1m");
        let carry_6m = result_6m
            .measures
            .get(MetricId::CarryTotal.as_str())
            .copied()
            .expect("carry_6m");
        let cs01_1m = result_1m
            .measures
            .get(MetricId::Cs01.as_str())
            .copied()
            .expect("cs01_1m");
        let cs01_6m = result_6m
            .measures
            .get(MetricId::Cs01.as_str())
            .copied()
            .expect("cs01_6m");

        assert!((be_1m - (-carry_1m / cs01_1m)).abs() < 1e-8, "1M: {be_1m}");
        assert!((be_6m - (-carry_6m / cs01_6m)).abs() < 1e-8, "6M: {be_6m}");

        // CS01 is horizon-independent here (both measured at as_of), so the
        // breakeven ratio must equal the carry ratio.
        assert!(
            (cs01_1m - cs01_6m).abs() < 1e-12,
            "CS01 should not depend on the theta period: {cs01_1m} vs {cs01_6m}"
        );
        let carry_ratio = carry_6m / carry_1m;
        let breakeven_ratio = be_6m / be_1m;
        assert!(
            (carry_ratio - breakeven_ratio).abs() < 1e-8,
            "breakeven ratio ({breakeven_ratio}) should equal carry ratio ({carry_ratio})"
        );
    }
}
