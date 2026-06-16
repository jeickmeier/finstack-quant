//! Spread calculators for structured credit (Z-spread, CS01, Spread Duration).

use crate::cashflow::traits::DatedFlows;
use crate::constants::ONE_BASIS_POINT;
use crate::instruments::fixed_income::structured_credit::types::constants::{
    Z_SPREAD_INITIAL_BRACKET, Z_SPREAD_SOLVER_TOLERANCE,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
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
        // Get dirty price (target value)
        let dirty_price = context
            .computed
            .get(&MetricId::DirtyPrice)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:DirtyPrice".to_string(),
                })
            })?;

        // Get notional to convert price to currency
        let base_npv = context.base_value.amount();
        let target_value = base_npv * (dirty_price / 100.0);

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
        let base_date = disc.base_date();
        let day_count = finstack_quant_core::dates::DayCount::Act365F;

        // Pre-compute (t, df, amount) for deterministic, fallible date handling.
        let cached_flows: Vec<(f64, f64, f64)> = flows
            .iter()
            .filter(|(date, _)| *date > context.as_of)
            .map(
                |(date, amount)| -> finstack_quant_core::Result<(f64, f64, f64)> {
                    let t =
                        day_count.year_fraction(base_date, *date, DayCountContext::default())?;
                    let df = disc.df_on_date_curve(*date)?;
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

                wide_solver.solve(objective, 0.05)
            }
        }
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::DirtyPrice]
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
        let base_date = disc.base_date();
        let day_count = finstack_quant_core::dates::DayCount::Act365F;

        // CS01 must be marginal: PV(z) - PV(z + 1bp), not PV(0) - PV(z + 1bp).
        // Compute both base PV (at Z-spread) and bumped PV (at Z-spread + 1bp).
        let mut base_npv_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        let mut bumped_npv_acc = finstack_quant_core::math::summation::NeumaierAccumulator::new();

        for (date, amount) in flows {
            if *date <= context.as_of {
                continue;
            }

            let t = day_count.year_fraction(base_date, *date, DayCountContext::default())?;
            let df = disc.df_on_date_curve(*date)?;
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

        // Note: We use base_npv directly instead of dirty_price for spread duration
        // since we're measuring dollar value change, not percentage change

        // Get base NPV
        let base_npv = context.base_value.amount();

        if base_npv == 0.0 {
            return Ok(0.0);
        }

        let spread_duration = -cs01 / (base_npv * ONE_BASIS_POINT);

        Ok(spread_duration)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Cs01, MetricId::DirtyPrice]
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
    let day_count = DayCount::Act365F;
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
    let day_count = DayCount::Act365F;

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
            let base_date = disc.base_date();
            let day_count = DayCount::Act365F;
            flows
                .iter()
                .filter(|(date, _)| *date > as_of)
                .map(|(date, amount)| -> Result<(f64, f64, f64)> {
                    let t =
                        day_count.year_fraction(base_date, *date, DayCountContext::default())?;
                    let df = disc.df_on_date_curve(*date)?;
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
