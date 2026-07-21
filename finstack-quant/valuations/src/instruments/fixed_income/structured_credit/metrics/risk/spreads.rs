//! Spread calculators for structured credit (Z-spread, CS01, Spread Duration).

use crate::cashflow::traits::DatedFlows;
use crate::constants::ONE_BASIS_POINT;
use crate::instruments::fixed_income::structured_credit::types::constants::{
    Z_SPREAD_INITIAL_BRACKET, Z_SPREAD_SOLVER_TOLERANCE,
};
use crate::instruments::fixed_income::structured_credit::{StructuredCredit, TrancheCoupon};
use crate::instruments::Instrument;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

// Z-spread bounds in decimal (not basis points)
// -500 bps to allow for premium bonds at tight spreads
const Z_SPREAD_MIN: f64 = -0.05;
// 5000 bps (50%) for distressed credits
const Z_SPREAD_MAX: f64 = 0.50;

/// Calculates Z-spread for structured credit.
///
/// Z-spread (zero-volatility spread) is the constant spread added to the
/// discount curve that equates the present value of cashflows to the market price.
///
/// # Market Standard Definition
///
/// Z-spread is the constant additive spread `z` such that:
/// ```text
/// Σ CF_i × DF(t_i) × exp(-z × t_i) = Market Price
/// ```
///
/// # Returns
///
/// Z-spread in decimal units (e.g., 0.0175 = 175 basis points)
///
/// # Market Conventions
///
/// - **CLO (fixed)**: 150-300 bps typical for AAA
/// - **ABS (fixed)**: 50-150 bps typical for AAA
/// - **RMBS (fixed)**: 100-250 bps typical
/// - **CMBS (fixed)**: 75-200 bps typical
///
pub struct ZSpreadCalculator;

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Z-spread requires an external quote. Using model `DirtyPrice`
        // (`base_value / notional * 100`) would make the objective
        // `PV(curve + z) == PV(curve)` and force a circular zero spread.
        let price_pct = context
            .get_metric_overrides()
            .and_then(|overrides| overrides.quoted_price_pct)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "structured-credit ZSpread requires \
                     MetricPricingOverrides.quoted_price_pct (price as a percentage of original \
                     balance); a model DirtyPrice cannot be used as its own spread target"
                        .to_string(),
                )
            })?;

        // Convert price points back to currency using original notional.
        let notional = crate::instruments::fixed_income::structured_credit::metrics::pricing::prices::get_original_notional(context)?;
        let target_value = notional * (price_pct / 100.0);

        // Get cashflows
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;
        let as_of = context.as_of;
        let day_count =
            crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

        // Pre-compute (t, df, amount) for deterministic, fallible date handling.
        // Discount from the valuation date `as_of` (settlement convention) so the
        // PV matches the as-of base value the target price is derived from; this
        // keeps the metric-registry z-spread consistent with the standalone
        // `calculate_tranche_z_spread` even when `as_of != curve.base_date()`.
        let cached_flows: Vec<(f64, f64, f64)> = flows
            .iter()
            .filter(|(date, _)| *date > as_of)
            .map(
                |(date, amount)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                    let t = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
                    let df = disc.df_between_dates(as_of, *date)?;
                    Ok((t, df, amount.amount()))
                },
            )
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        // Objective function: PV(z) - target = 0
        let objective = |z: f64| -> f64 {
            let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
            for (t, df, amt) in &cached_flows {
                let df_z = df * (-z * t).exp();
                pv.add(amt * df_z);
            }
            pv.total() - target_value
        };

        // Solve for z-spread using Brent's method with adaptive bracketing
        //
        // Credit spread characteristics:
        // - Investment grade: 50-300 bps (0.005-0.03)
        // - High yield: 300-1000 bps (0.03-0.10)
        // - Distressed: 1000+ bps (0.10+)
        // - Premium bonds may have negative Z-spread
        //
        // We start with a moderate bracket and allow expansion for edge cases.
        // Tolerance: 1e-6 = 0.01 bps precision (market standard)
        let solver = BrentSolver::new()
            .tolerance(Z_SPREAD_SOLVER_TOLERANCE)
            .initial_bracket_size(Some(Z_SPREAD_INITIAL_BRACKET));

        let valid_range = Z_SPREAD_MIN..=Z_SPREAD_MAX;

        // Try solving with standard initial guess
        match solver.solve(objective, 0.01) {
            Ok(z) if valid_range.contains(&z) => Ok(z),
            _ => {
                // Adaptive retry: try with a different initial guess
                // For distressed credits, start higher
                let z_high_guess = solver.solve(objective, 0.10);
                if let Ok(z) = z_high_guess {
                    if valid_range.contains(&z) {
                        return Ok(z);
                    }
                }

                // For premium bonds, try negative initial guess
                let z_low_guess = solver.solve(objective, -0.01);
                if let Ok(z) = z_low_guess {
                    if valid_range.contains(&z) {
                        return Ok(z);
                    }
                }

                // Final fallback: wider bracket with explicit bounds
                let wide_solver = BrentSolver::new()
                    .tolerance(Z_SPREAD_SOLVER_TOLERANCE)
                    .initial_bracket_size(Some(0.20)); // ±2000 bps

                // Wide-bracket fallback must still respect `valid_range`.
                let z = wide_solver.solve(objective, 0.05)?;
                if valid_range.contains(&z) {
                    Ok(z)
                } else {
                    Err(finstack_quant_core::Error::Validation(format!(
                        "z-spread solved to {z} which is outside the plausible \
                         range [{}, {}]; the objective is likely ill-posed for \
                         this target price",
                        Z_SPREAD_MIN, Z_SPREAD_MAX
                    )))
                }
            }
        }
    }

    fn dependencies(&self) -> &[MetricId] {
        &[]
    }
}

/// Calculates CS01 (credit spread DV01) for structured credit.
///
/// CS01 measures the dollar change in tranche value for a 1 basis point
/// parallel widening of the credit spread; for structured credit this is
/// **the primary risk metric**.
///
/// # Methodology
///
/// Structured-credit tranches are not priced off a par CDS curve, so this
/// calculator deviates from the workspace's canonical CS01 convention
/// (parallel 1 bp shock to par CDS curve, central difference — see
/// `metrics::sensitivities::cs01`). Instead it shocks the tranche's
/// **z-spread** by 1 bp and uses a forward finite difference:
///
/// ```text
/// CS01 = PV(z + 1bp) - PV(z)
///       = Σ CF_i · DF_i · (exp(-(z + 1bp) · t_i) − exp(-z · t_i))
/// ```
///
/// The forward form is preserved for deterministic golden parity; it agrees
/// with the canonical central form to `O(bump²) ≈ 10⁻⁸` of CS01 magnitude
/// for a 1 bp shock.
///
/// # Sign Convention
///
/// Identical to the workspace canonical reference:
/// - Long tranche → CS01 negative (wider spreads reduce PV).
/// - Short tranche → CS01 positive.
///
/// # Market Conventions (magnitudes for orientation)
///
/// - **CLO AAA**: $0.30-$0.50 per $100 face (30-50 DV01)
/// - **ABS AAA**: $2-$6 per $100 face
/// - **RMBS AAA**: $3-$8 per $100 face
/// - **CMBS AAA**: $4-$8 per $100 face
///
/// For **floating-rate CLO**, `|CS01| >> |DV01|` (spread risk dominates IR risk).
pub struct Cs01Calculator;

impl MetricCalculator for Cs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Get Z-spread (base spread)
        let base_spread = context
            .computed
            .get(&MetricId::ZSpread)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:ZSpread".to_string(),
                })
            })?;

        // Bump spread by 1bp
        let bumped_spread = base_spread + ONE_BASIS_POINT;

        // Get cashflows
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        // Get discount curve
        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;
        let as_of = context.as_of;
        let day_count =
            crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

        // CS01 must be marginal: PV(z) - PV(z + 1bp), not PV(0) - PV(z + 1bp).
        // Compute both base PV (at Z-spread) and bumped PV (at Z-spread + 1bp).
        // Discount from `as_of` to stay consistent with the z-spread that fed it.
        let mut base_npv_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        let mut bumped_npv_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();

        for (date, amount) in flows {
            if *date <= as_of {
                continue;
            }

            let t = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
            let df = disc.df_between_dates(as_of, *date)?;
            let amt = amount.amount();

            let df_base = df * (-base_spread * t).exp();
            base_npv_acc.add(amt * df_base);

            let df_bumped = df * (-bumped_spread * t).exp();
            bumped_npv_acc.add(amt * df_bumped);
        }

        let cs01 = bumped_npv_acc.total() - base_npv_acc.total();

        Ok(cs01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }
}

/// Calculates spread duration for structured credit.
///
/// Spread duration measures the percentage change in price for a 1 % change
/// in spread, expressed in years; it converts CS01 into a duration-like
/// metric.
///
/// # Formula
///
/// ```text
/// Spread Duration = -CS01 / (Price × 0.0001)
/// ```
///
/// Per the workspace canonical CS01 sign convention (see
/// `metrics::sensitivities::cs01`), CS01 is negative for a long tranche /
/// sell protection position; the leading minus sign therefore keeps spread
/// duration positive (in line with modified duration).
///
/// # Interpretation
///
/// - **CLO AAA (floating)**: 0.3-0.5 years (low spread duration)
/// - **ABS (fixed)**: 2-4 years
/// - **RMBS (fixed)**: 3-7 years (varies with prepayments)
/// - **CMBS (fixed)**: 4-8 years (close to modified duration)
///
/// # Key Insight
///
/// For fixed-rate structures, spread duration ≈ modified duration.
/// For floating-rate (CLO), spread duration >> IR duration.
///
pub struct SpreadDurationCalculator;

impl MetricCalculator for SpreadDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // Get CS01
        let cs01 = context
            .computed
            .get(&MetricId::Cs01)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Cs01".to_string(),
                })
            })?;

        // Normalize the dollar CS01 by the context's base NPV.
        let base_npv = context.base_value.amount();

        if base_npv == 0.0 {
            return Ok(0.0);
        }

        let spread_duration = -cs01 / (base_npv * ONE_BASIS_POINT);

        Ok(spread_duration)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Cs01]
    }
}

/// Calculate tranche-specific Z-spread in basis points.
///
/// Z-spread (zero-volatility spread) is the constant spread added to the
/// discount curve that equates the present value of cashflows to the market price.
///
/// # Arguments
///
/// * `cashflows` - The dated cashflows for the tranche
/// * `discount_curve` - The discount curve for PV calculation
/// * `target_pv` - The target present value to solve for
/// * `as_of` - The valuation date
///
/// # Returns
///
/// Z-spread in basis points
pub fn calculate_tranche_z_spread(
    cashflows: &DatedFlows,
    discount_curve: &DiscountCurve,
    target_pv: Money,
    as_of: Date,
) -> Result<f64> {
    let day_count = crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;
    let cached_flows: Vec<(f64, f64, f64)> = cashflows
        .iter()
        .filter(|(date, _)| *date > as_of)
        .map(|(date, amount)| -> Result<(f64, f64, f64)> {
            let t_from_as_of = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
            let df = discount_curve.df_between_dates(as_of, *date)?;
            Ok((t_from_as_of, df, amount.amount()))
        })
        .collect::<Result<Vec<_>>>()?;

    let objective = |z: f64| -> f64 {
        let mut pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for (t_from_as_of, df, amount) in &cached_flows {
            let df_z = *df * (-z * *t_from_as_of).exp();

            pv.add(*amount * df_z);
        }
        pv.total() - target_pv.amount()
    };

    // Tolerance: 1e-6 = 0.01 bps precision (market standard)
    let solver = BrentSolver::new()
        .tolerance(Z_SPREAD_SOLVER_TOLERANCE)
        .initial_bracket_size(Some(Z_SPREAD_INITIAL_BRACKET));

    let z_spread = solver.solve(objective, 0.0)?;

    // Convert to basis points
    Ok(z_spread * 10_000.0)
}

/// Calculate tranche-specific CS01 (credit spread sensitivity).
///
/// CS01 measures the dollar change in tranche value for a 1 basis point
/// parallel widening of the credit spread.
///
/// # Methodology
///
/// Structured-credit tranches are not priced off a par CDS curve, so this
/// helper deviates from the workspace's canonical CS01 convention
/// (parallel 1 bp shock to par CDS curve, central difference — see
/// `metrics::sensitivities::cs01`). It shocks the supplied `z_spread` by
/// 1 bp and uses a forward finite difference
/// `CS01 = PV(z + 1bp) − PV(z)`. The forward form is preserved for
/// deterministic golden parity; it agrees with the canonical central form to
/// `O(bump²) ≈ 10⁻⁸` of CS01 magnitude for a 1 bp shock.
///
/// # Arguments
///
/// * `cashflows` - The dated cashflows for the tranche
/// * `discount_curve` - The discount curve for PV calculation
/// * `z_spread` - The Z-spread in decimal (not basis points)
/// * `as_of` - The valuation date
///
/// # Returns
///
/// CS01 in currency units (dollar value change per 1 bp spread increase).
/// Sign convention follows the workspace canonical reference: long tranche /
/// sell protection → negative; short tranche / buy protection → positive.
pub fn calculate_tranche_cs01(
    cashflows: &DatedFlows,
    discount_curve: &DiscountCurve,
    z_spread: f64,
    as_of: Date,
) -> Result<f64> {
    let day_count = crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;

    // Calculate base PV
    let mut base_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let mut bumped_pv = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    let bumped_spread = z_spread + ONE_BASIS_POINT;

    for (date, amount) in cashflows {
        if *date <= as_of {
            continue;
        }

        let t_from_as_of = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
        let df = discount_curve.df_between_dates(as_of, *date)?;

        // Base PV
        let df_base = df * (-z_spread * t_from_as_of).exp();
        base_pv.add(amount.amount() * df_base);

        // Bumped PV
        let df_bumped = df * (-bumped_spread * t_from_as_of).exp();
        bumped_pv.add(amount.amount() * df_bumped);
    }

    Ok(bumped_pv.total() - base_pv.total())
}

/// Calculate a z-spread-equivalent discount margin for a floating-rate tranche.
///
/// The tranche's contractual cashflows are projected once without changing its
/// coupon projection. The solver then applies a constant additive spread to the
/// discount curve until those cashflows reproduce `target_pv`. The result is
/// therefore a z-spread-equivalent discount margin, not the tranche's
/// contractual quoted margin.
///
/// The returned decimal is zero when `target_pv` equals the model PV. A richer
/// (higher) target PV produces a negative margin; a cheaper (lower) target PV
/// produces a positive margin.
///
/// # Arguments
///
/// * `deal` - Structured-credit deal owning the tranche and defining its
///   contractual cashflows and discount-curve identifier.
/// * `tranche_id` - Identifier of the floating-rate tranche whose projected
///   cashflows are spread-discounted.
/// * `context` - Market context supplying the discount curve and any forward
///   curves or historical fixings required to project contractual cashflows.
/// * `as_of` - Valuation date used for cashflow projection and discounting.
/// * `target_pv` - Target present value in the tranche's currency. The sign of
///   the result is negative above model PV and positive below model PV.
///
/// # Returns
///
/// Z-spread-equivalent discount margin in decimal units (`0.0125` = 125 bp).
///
/// # Errors
///
/// Returns an error if the deal fails validation, the tranche is missing or is
/// fixed-rate, required discount/projection market data is unavailable, or the
/// spread solve fails or exceeds the ±5000 bp bound.
pub fn calculate_tranche_discount_margin(
    deal: &StructuredCredit,
    tranche_id: &str,
    context: &MarketContext,
    as_of: Date,
    target_pv: Money,
) -> Result<f64> {
    deal.validate_for_pricing()?;
    let tranche = deal
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: format!("tranche:{tranche_id}"),
            })
        })?;

    let TrancheCoupon::Floating(_) = &tranche.coupon else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "DiscountMargin is only defined for floating-rate tranches; '{tranche_id}' is fixed-rate"
        )));
    };

    // Project contractual cashflows once, then solve an additive discount
    // spread through the shared z-spread kernel.
    let cashflows =
        crate::instruments::fixed_income::structured_credit::pricing::generate_tranche_cashflows(
            deal, tranche_id, context, as_of,
        )?;

    let disc_curve_id = deal.discount_curve_id.as_str();
    let discount_curve = context.get_discount(disc_curve_id)?;

    let z_spread_bp = calculate_tranche_z_spread(
        &cashflows.cashflows,
        discount_curve.as_ref(),
        target_pv,
        as_of,
    )?;

    if !z_spread_bp.is_finite() || z_spread_bp.abs() > 5000.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Discount margin {z_spread_bp} bp exceeds reasonable bounds (±5000 bp)"
        )));
    }

    Ok(z_spread_bp * 1e-4)
}

/// Locate the tenor bucket(s) for year fraction `t`, with a triangular weight.
///
/// Returns `(lo, hi, w_hi)`: `t`'s sensitivity is split `(1 - w_hi)` to
/// `buckets[lo]` and `w_hi` to `buckets[hi]`. At or beyond the grid ends,
/// `lo == hi` and the whole weight lands in one bucket. The two weights always
/// sum to 1, so a per-cashflow split reconciles exactly to the parallel total.
fn locate_bucket(t: f64, buckets: &[f64]) -> (usize, usize, f64) {
    let last = buckets.len() - 1;
    if t <= buckets[0] {
        return (0, 0, 0.0);
    }
    if t >= buckets[last] {
        return (last, last, 0.0);
    }
    for i in 0..last {
        if t < buckets[i + 1] {
            let w = (t - buckets[i]) / (buckets[i + 1] - buckets[i]);
            return (i, i + 1, w);
        }
    }
    (last, last, 0.0)
}

/// Key-rate (bucketed) CS01 calculator for structured credit.
///
/// Mirrors [`Cs01Calculator`] — a 1 bp z-spread shock — but attributes each
/// cashflow's spread sensitivity to standard tenor buckets via triangular
/// (linear) allocation by the cashflow's year fraction. Because each
/// cashflow's two triangular weights sum to 1, the per-bucket CS01s sum
/// **exactly** to the parallel z-spread CS01.
///
/// There is no credit *curve* here — the z-spread is a scalar — so "key-rate"
/// means *where in time* the spread sensitivity sits, not a per-tenor curve
/// bump. The per-tenor series is stored under
/// `bucketed_cs01::{discount_curve_id}`.
pub struct BucketedCs01Calculator;

impl MetricCalculator for BucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        use crate::metrics::sensitivities::config::{
            format_bucket_label_cow, STANDARD_BUCKETS_YEARS,
        };

        let base_spread = context
            .computed
            .get(&MetricId::ZSpread)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:ZSpread".to_string(),
                })
            })?;
        let bumped_spread = base_spread + ONE_BASIS_POINT;
        let as_of = context.as_of;

        let disc_curve_id = context.discount_curve_id.clone().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        // Collect (year_fraction, discount_factor, amount) for surviving flows
        // into owned data, so no borrow of `context` outlives the curve/cashflow
        // reads — `store_bucketed_series` below needs `&mut context`.
        let cached: Vec<(f64, f64, f64)> = {
            let flows = context.cashflows.as_ref().ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "context.cashflows".to_string(),
                })
            })?;
            let disc = context.curves.get_discount(disc_curve_id.as_str())?;
            let day_count =
                crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;
            // Discount from `as_of` (settlement) so bucketed CS01 reconciles to the
            // parallel z-spread CS01, which now uses the same convention.
            flows
                .iter()
                .filter(|(date, _)| *date > as_of)
                .map(|(date, amount)| -> Result<(f64, f64, f64)> {
                    let t = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
                    let df = disc.df_between_dates(as_of, *date)?;
                    Ok((t, df, amount.amount()))
                })
                .collect::<Result<Vec<_>>>()?
        };

        // Each cashflow's z-spread CS01 contribution, triangular-allocated to
        // the surrounding standard tenor buckets.
        let buckets = STANDARD_BUCKETS_YEARS;
        let mut bucket_pnl = vec![0.0_f64; buckets.len()];
        for (t, df, amt) in &cached {
            let sens = amt * df * ((-bumped_spread * t).exp() - (-base_spread * t).exp());
            let (lo, hi, w_hi) = locate_bucket(*t, &buckets);
            bucket_pnl[lo] += sens * (1.0 - w_hi);
            if hi != lo {
                bucket_pnl[hi] += sens * w_hi;
            }
        }

        let series: Vec<(std::borrow::Cow<'static, str>, f64)> = buckets
            .iter()
            .zip(bucket_pnl.iter())
            .map(|(&t, &pnl)| (format_bucket_label_cow(t), pnl))
            .collect();
        let total: f64 = bucket_pnl.iter().sum();

        context.store_bucketed_series(
            MetricId::custom(format!("bucketed_cs01::{}", disc_curve_id.as_str())),
            series,
        );
        Ok(total)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }
}

#[cfg(test)]
mod zspread_quote_tests {
    use super::*;
    use crate::instruments::MetricPricingOverrides;
    use crate::metrics::standard_registry;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use std::sync::Arc;
    use time::Month;

    fn context_without_quote() -> MetricContext {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let payment_date = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let deal = StructuredCredit::example();
        let discount_curve_id = deal.discount_curve_id.clone();
        let market = MarketContext::new().insert(
            DiscountCurve::builder(discount_curve_id.as_str())
                .base_date(as_of)
                .knots([(0.0, 1.0), (1.0, 0.95)])
                .build()
                .expect("valid curve"),
        );
        let mut context = MetricContext::new(
            Arc::new(deal),
            Arc::new(market),
            as_of,
            Money::new(95.0, Currency::USD),
            MetricContext::default_config(),
        );
        context.cashflows = Some(vec![(payment_date, Money::new(100.0, Currency::USD))]);
        context.discount_curve_id = Some(discount_curve_id);
        context.notional = Some(Money::new(100.0, Currency::USD));
        context.computed.insert(MetricId::DirtyPrice, 95.0);
        context
    }

    #[test]
    fn registry_quote_dependent_spread_metrics_reject_a_missing_quote() {
        for requested in [
            MetricId::ZSpread,
            MetricId::Cs01,
            MetricId::BucketedCs01,
            MetricId::SpreadDuration,
        ] {
            let mut context = context_without_quote();
            let err = standard_registry()
                .compute(std::slice::from_ref(&requested), &mut context)
                .expect_err("quote-dependent spread metrics must require an external quote");
            let message = err.to_string();
            assert!(
                message.contains("quoted_price_pct"),
                "{requested} must identify the missing quote; got: {message}"
            );
        }
    }

    #[test]
    fn spread_duration_depends_only_on_cs01() {
        assert_eq!(SpreadDurationCalculator.dependencies(), &[MetricId::Cs01]);
    }

    /// An external quote breaks the model-price spread circularity.
    #[test]
    fn quoted_price_override_is_carried_on_metric_overrides() {
        let mut overrides = MetricPricingOverrides::default();
        assert!(
            overrides.quoted_price_pct.is_none(),
            "no quote by default, so existing behaviour is unchanged"
        );

        overrides.quoted_price_pct = Some(98.5);
        assert_eq!(
            overrides.quoted_price_pct,
            Some(98.5),
            "a quoted price must survive on MetricPricingOverrides so ZSpread \
             has a target that did not come from the model"
        );
    }

    /// The external quote round-trips through JSON binding inputs.
    #[test]
    fn quoted_price_override_round_trips_through_json() {
        let overrides = MetricPricingOverrides {
            quoted_price_pct: Some(102.25),
            ..Default::default()
        };

        let json = serde_json::to_string(&overrides).expect("serialize");
        assert!(
            json.contains("quoted_price_pct"),
            "a set quote must serialize; got {json}"
        );

        let parsed: MetricPricingOverrides = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.quoted_price_pct, Some(102.25));

        // And an absent quote must not bloat the wire format.
        let empty = MetricPricingOverrides::default();
        let empty_json = serde_json::to_string(&empty).expect("serialize");
        assert!(
            !empty_json.contains("quoted_price_pct"),
            "an unset quote must be skipped on the wire; got {empty_json}"
        );
    }
}
