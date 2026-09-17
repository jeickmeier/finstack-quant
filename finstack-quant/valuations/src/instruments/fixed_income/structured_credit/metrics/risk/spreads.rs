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
// -500 bp to allow for premium bonds at tight spreads
const Z_SPREAD_MIN: f64 = -0.05;
// 5000 bp (50%) for distressed credits
const Z_SPREAD_MAX: f64 = 0.50;
const RELATIVE_BASE_PV_EPSILON: f64 = 1e-12;

fn quoted_target_value(context: &mut MetricContext) -> Result<(f64, f64)> {
    let deal = context.instrument_as::<StructuredCredit>()?;
    let quotes = &deal.instrument_pricing_overrides.market_quotes;
    if quotes.quoted_clean_price.is_none() && quotes.quoted_dirty_price_currency.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "structured-credit spread metrics require quoted_clean_price or quoted_dirty_price_currency".into(),
        ));
    }
    let quote = super::super::quote::SettlementQuote::from_context(context)?;
    let target = quote
        .external_target(context.instrument_as::<StructuredCredit>()?)?
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation("missing structured-credit quote target".into())
        })?;
    Ok((target, quote.notional))
}

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
/// - **CLO (fixed)**: 150-300 bp typical for AAA
/// - **ABS (fixed)**: 50-150 bp typical for AAA
/// - **RMBS (fixed)**: 100-250 bp typical
/// - **CMBS (fixed)**: 75-200 bp typical
///
pub struct ZSpreadCalculator;

impl MetricCalculator for ZSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let (target, _) = quoted_target_value(context)?;
        let deal = context.instrument_as::<StructuredCredit>()?;
        let settlement = super::super::quote::settlement_date(deal, context.as_of)?;
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "structured-credit Z-spread requires projected cashflows".into(),
            )
        })?;
        let curve = context
            .curves
            .get_discount(deal.discount_curve_id.as_str())?;
        let spread = calculate_tranche_z_spread(
            flows,
            &curve,
            Money::new(target, deal.pool.get_base_currency())?,
            settlement,
        )? * 1e-4;
        if !(Z_SPREAD_MIN..=Z_SPREAD_MAX).contains(&spread) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit z-spread {spread} is outside [{Z_SPREAD_MIN}, {Z_SPREAD_MAX}]"
            )));
        }
        Ok(spread)
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

        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;

        let disc = context.curves.get_discount(disc_curve_id.as_str())?;
        let as_of = super::super::quote::settlement_date(
            context.instrument_as::<StructuredCredit>()?,
            context.as_of,
        )?;
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

/// Calculates structured-credit spread duration from Z-spread CS01.
///
/// Spread duration measures the percentage change in price for a 1 % change
/// in spread, expressed in years; it converts CS01 into a duration-like
/// metric.
///
/// The normalization basis is the external quoted-price target used to solve
/// [`MetricId::ZSpread`], not the model PV in `MetricContext::base_value`.
/// This keeps the CS01 numerator and price denominator on the same
/// quote-reproducing basis.
///
/// # Formula
///
/// ```text
/// Spread Duration = -Z-spread CS01 / (Quoted target PV × 0.0001)
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
        let cs01 = context
            .computed
            .get(&MetricId::Cs01)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Cs01".to_string(),
                })
            })?;
        if !cs01.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit Z-spread CS01 must be finite; got {cs01}"
            )));
        }

        let (base_npv, notional) = quoted_target_value(context)?;
        let base_pv_floor = RELATIVE_BASE_PV_EPSILON * notional.abs().max(1.0);
        if base_npv.abs() <= base_pv_floor {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit spread duration is undefined for zero or near-zero \
                 quote-reproducing base PV ({base_npv}); minimum magnitude is {base_pv_floor}"
            )));
        }

        let spread_duration = -cs01 / (base_npv * ONE_BASIS_POINT);
        if !spread_duration.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "structured-credit spread duration result must be finite; got {spread_duration}"
            )));
        }

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
    if !target_pv.amount().is_finite() || target_pv.amount() <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Z-spread requires a finite positive dirty settlement target".into(),
        ));
    }
    if cashflows
        .iter()
        .any(|(date, amount)| *date > as_of && amount.currency() != target_pv.currency())
    {
        return Err(finstack_quant_core::Error::Validation(
            "Z-spread target and cashflows must share one currency".into(),
        ));
    }
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
        pv.total() / target_pv.amount() - 1.0
    };

    // Solve the relative-price objective with a scale-independent tolerance.
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

/// Calculate the discount margin (curve DM) for a floating-rate tranche.
///
/// Follows the workspace's canonical FRN discount-margin convention (see the
/// bond `DiscountMarginCalculator`, Fabozzi / Bloomberg YAS): the tranche's
/// contractual cashflows are projected once — floating coupons off the index
/// forward curve at the contractual quoted margin, unchanged by the DM — and
/// the solver then applies a constant additive spread to the DEAL DISCOUNT
/// CURVE until those cashflows reproduce `target_pv`.
///
/// This is a **curve DM**: when the deal's discount curve differs from the
/// tranche's projection index curve, the solved DM includes that basis, so it
/// is not the pure spread-over-index quote a dealer run shows. On a
/// consistent curve setup where the discount curve equals the index curve, a
/// tranche priced at par solves to (approximately) its contractual quoted
/// margin. Discounting uses the continuous-compounding spread kernel shared
/// with the tranche Z-spread solver.
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
/// * `target_pv` - Dirty settlement value in the tranche's currency, including
///   accrued interest once. The buyer owns only flows after the deal's
///   `quote_settlement_date` (valuation date when absent). The sign of
///   the result is negative above model PV and positive below model PV.
///
/// # Returns
///
/// Curve discount margin in decimal units (`0.0125` = 125 bp).
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

    let quote = super::super::quote::SettlementQuote::for_tranche(
        deal,
        as_of,
        tranche.current_balance.amount(),
        &cashflows,
    )?;
    if target_pv.currency() != tranche.original_balance.currency() {
        return Err(finstack_quant_core::Error::Validation(
            "discount-margin target currency must match the tranche".into(),
        ));
    }
    quote.dirty_target(target_pv.amount())?;
    let disc_curve_id = deal.discount_curve_id.as_str();
    let discount_curve = context.get_discount(disc_curve_id)?;

    let z_spread_bp = calculate_tranche_z_spread(
        &cashflows.cashflows,
        discount_curve.as_ref(),
        target_pv,
        quote.settlement,
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
        let as_of = super::super::quote::settlement_date(
            context.instrument_as::<StructuredCredit>()?,
            context.as_of,
        )?;

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
            MetricId::composite(&MetricId::BucketedCs01, &[disc_curve_id.as_str()]),
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
    use crate::instruments::MarketQuoteOverrides;
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
            Money::from((95_i64, Currency::USD)),
            MetricContext::default_config(),
        );
        context.cashflows = Some(vec![(payment_date, Money::from((100_i64, Currency::USD)))]);
        context.discount_curve_id = Some(discount_curve_id);
        context.notional = Some(Money::from((100_i64, Currency::USD)));
        context.computed.insert(MetricId::DirtyPrice, 95.0);
        context.structured_credit_accruals = Some(Vec::new());
        context
    }

    fn context_with_quote(quoted_price_pct: f64) -> MetricContext {
        let mut context = context_without_quote();
        let mut deal = context
            .instrument_as::<StructuredCredit>()
            .expect("deal")
            .clone();
        deal.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(quoted_price_pct);
        context.set_instrument(Arc::new(deal));
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
                message.contains("quoted_clean_price"),
                "{requested} must identify the missing quote; got: {message}"
            );
        }
    }

    #[test]
    fn spread_duration_depends_only_on_cs01() {
        assert_eq!(SpreadDurationCalculator.dependencies(), &[MetricId::Cs01]);
    }

    #[test]
    fn spread_duration_normalizes_by_quote_reproducing_target() {
        let mut context = context_with_quote(90.0);
        standard_registry()
            .compute(&[MetricId::SpreadDuration], &mut context)
            .expect("quoted spread duration");

        let cs01 = context.computed[&MetricId::Cs01];
        let spread_duration = context.computed[&MetricId::SpreadDuration];
        let expected = -cs01 / (90.0 * ONE_BASIS_POINT);
        let old_model_pv_normalization = -cs01 / (95.0 * ONE_BASIS_POINT);

        assert!(
            (spread_duration - expected).abs() < 1e-12,
            "spread duration must normalize by the quoted target: \
             metric={spread_duration}, expected={expected}"
        );
        assert!(
            (spread_duration - old_model_pv_normalization).abs() > 1e-3,
            "fixture must distinguish quoted-target normalization from context.base_value"
        );
    }

    #[test]
    fn spread_duration_rejects_zero_near_zero_and_nonfinite_quote_targets() {
        for quoted_price_pct in [0.0, 1e-13, f64::NAN, f64::INFINITY] {
            let mut context = context_with_quote(quoted_price_pct);
            context.computed.insert(MetricId::Cs01, -0.01);

            let err = SpreadDurationCalculator
                .calculate(&mut context)
                .expect_err("invalid quote target must fail closed");
            let message = err.to_string();
            assert!(
                message.contains("quote") || message.contains("base PV"),
                "error must identify the invalid normalization basis; got: {message}"
            );
        }
    }

    #[test]
    fn spread_duration_rejects_nonfinite_cs01_and_result() {
        for cs01 in [f64::NAN, f64::INFINITY] {
            let mut context = context_with_quote(95.0);
            context.computed.insert(MetricId::Cs01, cs01);

            let err = SpreadDurationCalculator
                .calculate(&mut context)
                .expect_err("non-finite CS01 must fail closed");
            assert!(
                err.to_string().contains("CS01"),
                "error must identify non-finite CS01; got: {err}"
            );
        }
    }

    /// An external quote breaks the model-price spread circularity.
    #[test]
    fn quoted_price_override_has_one_market_quote_owner() {
        let mut overrides = MarketQuoteOverrides::default();
        assert!(
            overrides.quoted_clean_price.is_none(),
            "no quote by default, so existing behaviour is unchanged"
        );

        overrides.quoted_clean_price = Some(98.5);
        assert_eq!(
            overrides.quoted_clean_price,
            Some(98.5),
            "a quoted price must survive on MarketQuoteOverrides so ZSpread \
             has a target that did not come from the model"
        );
    }

    /// The external quote round-trips through JSON binding inputs.
    #[test]
    fn quoted_price_override_round_trips_through_json() {
        let overrides = MarketQuoteOverrides {
            quoted_clean_price: Some(102.25),
            ..Default::default()
        };

        let json = serde_json::to_string(&overrides).expect("serialize");
        assert!(
            json.contains("quoted_clean_price"),
            "a set quote must serialize; got {json}"
        );

        let parsed: MarketQuoteOverrides = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.quoted_clean_price, Some(102.25));

        // And an absent quote must not bloat the wire format.
        let empty = MarketQuoteOverrides::default();
        let empty_json = serde_json::to_string(&empty).expect("serialize");
        assert!(
            !empty_json.contains("quoted_clean_price"),
            "an unset quote must be skipped on the wire; got {empty_json}"
        );
    }
}
