//! Breakeven parameter shift calculator.
//!
//! Computes how much a valuation parameter (spread, yield, vol, correlation)
//! can move before carry + roll-down is wiped out over the configured horizon.

use crate::instruments::{BreakevenConfig, BreakevenMode, BreakevenTarget, Instrument};
use crate::metrics::sensitivities::theta::calculate_theta_date;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::borrow::Cow;

/// Minimum absolute sensitivity value below which breakeven is undefined.
const SENSITIVITY_FLOOR: f64 = 1e-12;

/// Computes breakeven parameter shift from carry and sensitivity.
pub struct BreakevenCalculator;

impl MetricCalculator for BreakevenCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let config = context
            .get_metric_pricing_overrides()
            .and_then(|o| o.breakeven_config)
            .ok_or_else(|| finstack_quant_core::InputError::NotFound {
                id: "breakeven_config: set BreakevenConfig on MetricPricingOverrides".into(),
            })?;

        if config.mode == BreakevenMode::Iterative {
            if let Some(message) = iterative_mode_error(context, config.target) {
                return Err(finstack_quant_core::Error::Validation(message));
            }
        }

        let carry_total = context.computed_metric(&MetricId::CarryTotal)?;

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
            .get_metric_pricing_overrides()
            .and_then(|o| o.breakeven_config)
        else {
            // No config: `calculate` will raise the descriptive error. Fall
            // back to the static set so ordering stays well-defined.
            return Cow::Borrowed(self.dependencies());
        };
        if config.mode == BreakevenMode::Iterative
            && iterative_mode_error(context, config.target).is_some()
        {
            return Cow::Borrowed(self.dependencies());
        }
        Cow::Owned(vec![
            MetricId::CarryTotal,
            config.target.sensitivity_metric(),
        ])
    }
}

/// Describe why [`BreakevenMode::Iterative`] is unsupported for this context.
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
fn iterative_mode_error(context: &MetricContext, target: BreakevenTarget) -> Option<String> {
    if target == BreakevenTarget::BaseCorrelation {
        return Some(
            "BreakevenMode::Iterative is unsupported for BaseCorrelation because no scalar \
             base-correlation bump API exists"
                .to_string(),
        );
    }
    if !matches!(target, BreakevenTarget::ZSpread | BreakevenTarget::Oas) {
        return None;
    }
    let has_credit_curve = context
        .instrument
        .market_dependencies()
        .map(|d| !d.curves.credit_curves.is_empty())
        .unwrap_or(false);
    if has_credit_curve {
        return Some(format!(
            "BreakevenMode::Iterative is unsupported for {target:?} on an instrument with a \
             credit curve: CS01 is measured as a hazard-curve (par spread) bump while the \
             iterative solve applies a parallel discount-curve shift, so the two modes would \
             report different risk factors under one metric name. Use BreakevenMode::Linear."
        ));
    }
    None
}

/// Discount and forward curves a yield breakeven shifts, discount curves
/// first, without duplicates.
fn ytm_curve_ids(instrument: &dyn Instrument) -> Result<Vec<CurveId>> {
    let curves = instrument.market_dependencies()?.curves;
    let mut ids: Vec<CurveId> = curves.discount_curves.iter().cloned().collect();
    for fwd in &curves.forward_curves {
        if !ids.contains(fwd) {
            ids.push(fwd.clone());
        }
    }
    Ok(ids)
}

/// Bump `base` by `delta` for the given breakeven target.
///
/// Curve and surface ids are resolved from `context`; the bump is applied to
/// `base`, so any fixings `base` holds are carried into the bumped market.
///
/// Returns the bumped [`MarketContext`] or an error if the required
/// curve / surface cannot be determined.
fn bump_market_for_target(
    context: &MetricContext,
    base: &MarketContext,
    delta: f64,
    target: BreakevenTarget,
) -> Result<MarketContext> {
    match target {
        BreakevenTarget::ZSpread | BreakevenTarget::Oas => {
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
            crate::metrics::bump_discount_curve_parallel(base, &curve_id, delta)
        }
        BreakevenTarget::Ytm => {
            // A yield move shifts every rate the instrument discounts or
            // projects with, so all its discount and forward curves move
            // together in parallel.
            let mut curve_ids = ytm_curve_ids(context.instrument.as_ref())?;
            if curve_ids.is_empty() {
                curve_ids.push(context.discount_curve_id.clone().ok_or_else(|| {
                    finstack_quant_core::InputError::NotFound {
                        id: "iterative_breakeven: no rate curve found for instrument".into(),
                    }
                })?);
            }
            base.bump(curve_ids.into_iter().map(|id| MarketBump::Curve {
                id,
                spec: BumpSpec::parallel_bp(delta),
            }))
        }
        BreakevenTarget::ImpliedVol => {
            let vol_surface_ids = context
                .instrument
                .market_dependencies()
                .ok()
                .map(|dependencies| dependencies.present_vol_surface_ids(&context.curves))
                .filter(|vol_surface_ids| !vol_surface_ids.is_empty())
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
            crate::metrics::finite_difference::bump_surfaces_vol_absolute(
                base,
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
///
/// # Market-held fixings
///
/// Both horizon reprices run on the unrolled market with every coupon fixing
/// the as-of schedule projected in `[as_of, rolled_date]` added as an
/// exact-date `FIXING:` observation at its as-of projection (as theta and
/// carry decomposition do), so instruments whose horizon crosses a fixing
/// reprice instead of failing on the unpublished fixing.
///
/// The bump is applied on top of that fixings-held market: crossed fixings
/// stay at their unbumped as-of projections in the base and in every bumped
/// reprice. The objective therefore isolates the parameter shift's effect on
/// the remaining flows, on the same footing as `carry_total`.
fn iterative_breakeven(
    context: &MetricContext,
    carry_total: f64,
    sensitivity: f64,
    config: &BreakevenConfig,
) -> Result<f64> {
    // Determine the horizon date (same convention as carry decomposition).
    let period = crate::metrics::sensitivities::theta::theta_period(context);
    let expiry_date = context
        .instrument
        .last_payment_date(&context.curves, context.as_of)?;
    let rolled_date = calculate_theta_date(context.as_of, period, expiry_date)?;

    let held_market = context.fixings_held_market(rolled_date)?;

    // Base PV at the horizon with current (un-bumped) curves.
    let base_pv_at_horizon = context.reprice_money(&held_market, rolled_date)?.amount();

    // Linear estimate as initial guess.
    let initial_guess = -carry_total / sensitivity;

    let target = config.target;

    // A bump or repricing failure (e.g. "no vol surface found") aborts the
    // solve and is surfaced as the real cause.
    let objective = |delta: f64| -> Result<f64> {
        let bumped_market = bump_market_for_target(context, &held_market, delta, target)?;
        let pv = context.reprice_money(&bumped_market, rolled_date)?;
        Ok(carry_total + pv.amount() - base_pv_at_horizon)
    };

    // No `BracketHint` is set deliberately. Core's hints are sized in absolute
    // rate units (`Spread` = 0.005, `Ytm` = 0.02) whereas `delta` here is in
    // basis points / vol points, so a hint would size the initial bracket ~100x
    // too small. The adaptive default (1% of the initial guess, floored at
    // 0.01) is correctly scaled for this objective.
    BrentSolver::new()
        .with_tolerance(1e-8)
        .with_max_iterations(50)
        .solve_fallible(objective, initial_guess)
}
