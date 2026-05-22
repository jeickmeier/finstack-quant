//! Commodity spread option pricer using Kirk's approximation.
//!
//! Kirk's approximation (1995) prices spread options on two correlated
//! commodities by reducing the problem to a single-asset Black-76 formula
//! with adjusted volatility.
//!
//! # Algorithm
//!
//! Given forward prices F1, F2, strike K, vols sigma1, sigma2, and
//! correlation rho:
//!
//! 1. Adjusted strike: K_adj = F2 + K
//! 2. Weight: w = F2 / (F2 + K)
//! 3. Kirk's vol: sigma_kirk = sqrt(sigma1^2 - 2*rho*sigma1*sigma2*w + (sigma2*w)^2)
//! 4. Call price = Black76(F1, K_adj, sigma_kirk, T, DF)
//! 5. Put price (direct Black-76 put): P = DF * (K_adj * N(-d2) - F1 * N(-d1))
//!
//! # Guard Conditions
//!
//! - Kirk's approximation breaks down when F2 + K ~ 0 (division by near-zero).
//!   A guard returns intrinsic value when |F2 + K| < epsilon.
//! - Correlation must be in [-1, 1].
//!
//! # References
//!
//! - Kirk, E. (1995). "Correlation in the Energy Markets."

use crate::instruments::commodity::commodity_spread_option::CommoditySpreadOption;
use crate::instruments::OptionType;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::norm_cdf;
use finstack_core::money::Money;

/// Minimum denominator for Kirk's approximation (F2 + K).
/// When `k_adj = F2 + K <= KIRK_DENOM_EPSILON` (zero or negative), the
/// Black-76 mapping `ln(F1/k_adj)` is undefined and Kirk's formula breaks
/// down entirely. In that region we fall back to the discounted intrinsic
/// value `df*(F1-F2-K)` for the call; the put is worthless by put-call
/// parity. This guard covers both the near-zero and strictly negative cases.
const KIRK_DENOM_EPSILON: f64 = 1e-10;

/// Compute the present value of a commodity spread option using Kirk's approximation.
pub(crate) fn compute_pv(
    inst: &CommoditySpreadOption,
    market: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<Money> {
    inst.validate()?;

    // Post-expiry: option is fully settled
    if as_of > inst.expiry {
        return Ok(Money::new(0.0, inst.currency));
    }

    let t = inst.time_to_expiry(as_of)?;

    let f1 = inst.leg1_forward(market)?;
    let f2 = inst.leg2_forward(market)?;

    let disc = market.get_discount(inst.discount_curve_id.as_str())?;
    let df = disc.df_between_dates(as_of, inst.expiry)?;

    // At expiry or zero time: return intrinsic value
    if t <= 0.0 {
        let intrinsic = match inst.option_type {
            OptionType::Call => (f1 - f2 - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - (f1 - f2)).max(0.0),
        };
        return Ok(Money::new(intrinsic * inst.notional * df, inst.currency));
    }

    let unit_price = kirk_price(inst, market, as_of, f1, f2, t, df)?;

    Ok(Money::new(unit_price * inst.notional, inst.currency))
}

/// Kirk's approximation for spread option pricing.
///
/// Returns the per-unit option price (already discounted).
fn kirk_price(
    inst: &CommoditySpreadOption,
    market: &MarketContext,
    as_of: Date,
    f1: f64,
    f2: f64,
    t: f64,
    df: f64,
) -> finstack_core::Result<f64> {
    let disc = market.get_discount(inst.discount_curve_id.as_str())?;
    let curve_dc = disc.day_count();
    let t_rate = curve_dc
        .year_fraction(
            as_of,
            inst.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?
        .max(0.0);
    let _r = disc.zero(t_rate);

    // Get vols from surfaces
    let surface1 = market.get_surface(inst.leg1_vol_surface_id.as_str())?;
    let sigma1 = surface1.value_clamped(t, f1);

    let surface2 = market.get_surface(inst.leg2_vol_surface_id.as_str())?;
    let sigma2 = surface2.value_clamped(t, f2);

    let rho = inst.correlation;

    // Kirk's adjusted strike
    let k_adj = f2 + inst.strike;

    // Guard: if K_adj <= 0 (or near-zero), Kirk's approximation breaks down.
    // A non-positive k_adj = F2 + K makes ln(F1/k_adj) undefined (NaN for
    // negative, -inf for zero). Under Kirk's framework the best available
    // approximation is the discounted forward spread df*(F1-F2-K) for the
    // call; the put is then worthless by put-call parity. Note: the spread
    // payoff (F1_T - F2_T - K)^+ can still expire worthless in Monte-Carlo
    // paths even when k_adj <= 0 — this is an approximation, not a certainty.
    if k_adj <= KIRK_DENOM_EPSILON {
        let price = match inst.option_type {
            OptionType::Call => (f1 - f2 - inst.strike).max(0.0) * df,
            OptionType::Put => 0.0,
        };
        return Ok(price);
    }

    // Kirk's vol: sigma_kirk = sqrt(sigma1^2 - 2*rho*sigma1*sigma2*w + (sigma2*w)^2)
    // where w = F2 / (F2 + K)
    let w = f2 / k_adj;
    let sigma_kirk_sq = sigma1 * sigma1 - 2.0 * rho * sigma1 * sigma2 * w + (sigma2 * w).powi(2);

    // Guard against numerical issues (negative variance from extreme inputs)
    let sigma_kirk = if sigma_kirk_sq <= 0.0 {
        0.0
    } else {
        sigma_kirk_sq.sqrt()
    };

    // Zero vol case: return intrinsic
    if sigma_kirk <= 0.0 {
        let intrinsic = match inst.option_type {
            OptionType::Call => (f1 - k_adj).max(0.0),
            OptionType::Put => (k_adj - f1).max(0.0),
        };
        return Ok(intrinsic * df);
    }

    // Black-76 on F1 vs K_adj with sigma_kirk
    match inst.option_type {
        OptionType::Call => Ok(black76_call(f1, k_adj, sigma_kirk, t, df)),
        OptionType::Put => {
            // Price the put DIRECTLY with the Black-76 put formula.
            // P = df * (K_adj * N(-d2) - F1 * N(-d1))
            // This avoids injecting Kirk approximation error via put-call parity.
            Ok(black76_put(f1, k_adj, sigma_kirk, t, df))
        }
    }
}

/// Black-76 call price.
fn black76_call(forward: f64, strike: f64, sigma: f64, t: f64, df: f64) -> f64 {
    let (d1, d2) =
        crate::instruments::common_impl::models::d1_d2_black76(forward, strike, sigma, t);

    df * (forward * norm_cdf(d1) - strike * norm_cdf(d2))
}

/// Black-76 put price (direct formula, not derived via put-call parity).
///
/// P = df * (strike * N(-d2) - forward * N(-d1))
fn black76_put(forward: f64, strike: f64, sigma: f64, t: f64, df: f64) -> f64 {
    let (d1, d2) =
        crate::instruments::common_impl::models::d1_d2_black76(forward, strike, sigma, t);

    df * (strike * norm_cdf(-d2) - forward * norm_cdf(-d1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use finstack_core::types::{CurveId, InstrumentId};

    /// Helper to build a flat vol surface at a given level.
    fn flat_vol_surface(id: &str, vol: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[0.25, 1.0, 2.0, 5.0])
            .strikes(&[50.0, 100.0, 150.0])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .build()
            .expect("flat vol surface")
    }

    /// Helper to build a flat price curve at a given level.
    fn flat_price_curve(id: &str, price: f64, as_of: time::Date) -> PriceCurve {
        PriceCurve::builder(id)
            .base_date(as_of)
            .spot_price(price)
            .knots([(0.0, price), (1.0, price), (2.0, price)])
            .build()
            .expect("flat price curve")
    }

    /// Helper to build a flat discount curve.
    fn flat_discount_curve(id: &str, rate: f64, as_of: time::Date) -> DiscountCurve {
        // Build a discount curve from discount factors: DF(t) = exp(-r*t)
        let df_1y = (-rate * 1.0_f64).exp();
        let df_2y = (-rate * 2.0_f64).exp();
        let df_5y = (-rate * 5.0_f64).exp();
        DiscountCurve::builder(id)
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, df_1y), (2.0, df_2y), (5.0, df_5y)])
            .build()
            .expect("flat discount curve")
    }

    fn make_market(
        as_of: time::Date,
        f1: f64,
        f2: f64,
        vol1: f64,
        vol2: f64,
        rate: f64,
    ) -> MarketContext {
        let leg1_fwd = flat_price_curve("LEG1-FWD", f1, as_of);
        let leg2_fwd = flat_price_curve("LEG2-FWD", f2, as_of);
        let leg1_vol = flat_vol_surface("LEG1-VOL", vol1);
        let leg2_vol = flat_vol_surface("LEG2-VOL", vol2);
        let disc = flat_discount_curve("USD-OIS", rate, as_of);

        MarketContext::new()
            .insert(leg1_fwd)
            .insert(leg2_fwd)
            .insert_surface(leg1_vol)
            .insert_surface(leg2_vol)
            .insert(disc)
    }

    fn make_spread_option(
        option_type: OptionType,
        strike: f64,
        correlation: f64,
        expiry: time::Date,
    ) -> CommoditySpreadOption {
        try_make_spread_option(option_type, strike, correlation, expiry)
            .expect("build spread option")
    }

    fn try_make_spread_option(
        option_type: OptionType,
        strike: f64,
        correlation: f64,
        expiry: time::Date,
    ) -> finstack_core::Result<CommoditySpreadOption> {
        CommoditySpreadOption::builder()
            .id(InstrumentId::new("TEST-SPREAD"))
            .currency(Currency::USD)
            .option_type(option_type)
            .expiry(expiry)
            .strike(strike)
            .notional(1.0)
            .leg1_forward_curve_id(CurveId::new("LEG1-FWD"))
            .leg2_forward_curve_id(CurveId::new("LEG2-FWD"))
            .leg1_vol_surface_id(CurveId::new("LEG1-VOL"))
            .leg2_vol_surface_id(CurveId::new("LEG2-VOL"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .correlation(correlation)
            .day_count(DayCount::Act365F)
            .build()
    }

    #[test]
    fn identical_assets_zero_strike_perfect_correlation_near_zero_price() {
        // Same forward for both legs, K=0, rho=1 => spread is always 0,
        // call on max(0, 0) = 0.
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let market = make_market(as_of, 100.0, 100.0, 0.30, 0.30, 0.05);
        let opt = make_spread_option(OptionType::Call, 0.0, 1.0, expiry);

        let pv = opt.value(&market, as_of).expect("price spread option");
        // With identical forwards, zero strike, and perfect correlation,
        // Kirk's vol = sqrt(sigma1^2 - 2*1*sigma1*sigma2*(F2/(F2+0)) + (sigma2*(F2/(F2+0)))^2)
        //            = sqrt(sigma1^2 - 2*sigma1*sigma2 + sigma2^2)
        //            = |sigma1 - sigma2| = 0 when sigma1 == sigma2
        // So the option should be worth ~0 (intrinsic only)
        assert!(
            pv.amount().abs() < 0.01,
            "Expected near-zero price for identical assets with K=0 and rho=1, got {}",
            pv.amount()
        );
    }

    #[test]
    fn perfect_correlation_equal_vols_reduces_effective_vol() {
        // With rho=1 and sigma1 == sigma2 == sigma:
        // Kirk's vol = sqrt(sigma^2 - 2*sigma^2*w + sigma^2*w^2) = sigma*sqrt((1-w)^2) = sigma*(1-w)
        // where w = F2/(F2+K)
        // This is strictly less than sigma1 when w > 0
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let f1 = 100.0;
        let f2 = 80.0;
        let sigma = 0.30;
        let k = 10.0;

        let market = make_market(as_of, f1, f2, sigma, sigma, 0.05);

        // Price with perfect correlation
        let opt_corr = make_spread_option(OptionType::Call, k, 1.0, expiry);
        let pv_corr = opt_corr.value(&market, as_of).expect("price corr=1");

        // Price with zero correlation (higher vol -> higher price for ATM-ish option)
        let opt_zero = make_spread_option(OptionType::Call, k, 0.0, expiry);
        let pv_zero = opt_zero.value(&market, as_of).expect("price corr=0");

        assert!(
            pv_corr.amount() < pv_zero.amount(),
            "Perfect correlation should give lower price ({}) than zero correlation ({})",
            pv_corr.amount(),
            pv_zero.amount()
        );
    }

    #[test]
    fn put_call_parity() {
        // C - P = DF * (F1 - F2 - K)
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let f1 = 100.0;
        let f2 = 80.0;
        let k = 10.0;
        let rate = 0.05;
        let rho = 0.6;

        let market = make_market(as_of, f1, f2, 0.25, 0.30, rate);

        let call = make_spread_option(OptionType::Call, k, rho, expiry);
        let put = make_spread_option(OptionType::Put, k, rho, expiry);

        let call_pv = call.value(&market, as_of).expect("call price").amount();
        let put_pv = put.value(&market, as_of).expect("put price").amount();

        // Put-call parity for spread options: C - P = DF * (F1 - F2 - K)
        // Both call and put are priced directly with Black-76 formulas using
        // the same sigma_kirk, d1, d2. Black-76 call and put satisfy parity
        // exactly by construction, so this test verifies the implementation.
        // Verify with a K=0, zero-vol forward contract to get the exact
        // discounted spread from the same code path.
        let fwd_contract = make_spread_option(OptionType::Call, 0.0, rho, expiry);
        let zero_vol_mkt = make_market(as_of, f1, f2, 0.0, 0.0, rate);
        let fwd_spread_pv = fwd_contract
            .value(&zero_vol_mkt, as_of)
            .expect("fwd spread")
            .amount();
        // fwd_spread_pv = DF * (F1 - F2) from the zero-vol call with K=0

        // The parity relation: C - P should be proportional to (F1-F2-K)/(F1-F2) * fwd_spread_pv
        // More practically: verify C - P and DF*(F1-F2-K) agree to 0.1% relative
        let actual_f1 = call.leg1_forward(&market).expect("leg1 fwd");
        let actual_f2 = call.leg2_forward(&market).expect("leg2 fwd");
        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc
            .df_between_dates(as_of, expiry)
            .expect("discount factor");
        let parity_rhs = df * (actual_f1 - actual_f2 - k);
        let _ = fwd_spread_pv;

        let diff = (call_pv - put_pv) - parity_rhs;
        let rel_err = diff.abs() / parity_rhs.abs();
        assert!(
            rel_err < 1e-3,
            "Put-call parity violated: C-P={}, DF*(F1-F2-K)={}, relative error={}",
            call_pv - put_pv,
            parity_rhs,
            rel_err
        );
    }

    #[test]
    fn negative_correlation_increases_spread_vol() {
        // Negative correlation should increase the effective Kirk vol,
        // resulting in a higher option price compared to positive correlation.
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let market = make_market(as_of, 100.0, 80.0, 0.25, 0.30, 0.05);

        let opt_pos = make_spread_option(OptionType::Call, 10.0, 0.5, expiry);
        let opt_neg = make_spread_option(OptionType::Call, 10.0, -0.5, expiry);

        let pv_pos = opt_pos
            .value(&market, as_of)
            .expect("positive corr")
            .amount();
        let pv_neg = opt_neg
            .value(&market, as_of)
            .expect("negative corr")
            .amount();

        assert!(
            pv_neg > pv_pos,
            "Negative correlation ({}) should give higher price than positive correlation ({})",
            pv_neg,
            pv_pos
        );
    }

    #[test]
    fn zero_vol_returns_intrinsic() {
        // With zero vol, option value should be max(F1 - F2 - K, 0) * DF
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let f1 = 100.0;
        let f2 = 80.0;
        let k = 10.0;
        let rate = 0.05;

        let market = make_market(as_of, f1, f2, 0.0, 0.0, rate);
        let opt = make_spread_option(OptionType::Call, k, 0.7, expiry);

        let pv = opt.value(&market, as_of).expect("zero vol price").amount();

        // With zero vol, the option value should closely approximate the
        // discounted intrinsic. Small deviations can arise from curve
        // interpolation between knot points.
        let actual_f1 = opt.leg1_forward(&market).expect("leg1 fwd");
        let actual_f2 = opt.leg2_forward(&market).expect("leg2 fwd");
        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc
            .df_between_dates(as_of, expiry)
            .expect("discount factor");
        let expected = (actual_f1 - actual_f2 - k).max(0.0) * df;

        let rel_err = (pv - expected).abs() / expected.abs();
        assert!(
            rel_err < 1e-3,
            "Zero vol price ({}) should approximate discounted intrinsic ({}), rel_err={}",
            pv,
            expected,
            rel_err
        );
    }

    #[test]
    fn zero_vol_otm_returns_zero() {
        // Zero vol, OTM option: F1 - F2 - K < 0 => max(., 0) = 0
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let f1 = 100.0;
        let f2 = 80.0;
        let k = 30.0; // OTM: spread is 20, strike is 30

        let market = make_market(as_of, f1, f2, 0.0, 0.0, 0.05);
        let opt = make_spread_option(OptionType::Call, k, 0.7, expiry);

        let pv = opt
            .value(&market, as_of)
            .expect("zero vol OTM price")
            .amount();
        assert!(
            pv.abs() < 1e-12,
            "OTM call with zero vol should be zero, got {}",
            pv
        );
    }

    #[test]
    fn correlation_validation() {
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let market = make_market(as_of, 100.0, 80.0, 0.25, 0.30, 0.05);

        // Out-of-range correlations fail at construction (builder validation).
        assert!(try_make_spread_option(OptionType::Call, 10.0, 1.5, expiry).is_err());
        assert!(try_make_spread_option(OptionType::Call, 10.0, -1.5, expiry).is_err());

        // Boundary values should price.
        let opt = make_spread_option(OptionType::Call, 10.0, 1.0, expiry);
        assert!(opt.value(&market, as_of).is_ok());

        let opt = make_spread_option(OptionType::Call, 10.0, -1.0, expiry);
        assert!(opt.value(&market, as_of).is_ok());
    }

    #[test]
    fn spread_option_is_positive() {
        // Any option with positive vol should have positive price
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let market = make_market(as_of, 100.0, 80.0, 0.25, 0.30, 0.05);

        let call = make_spread_option(OptionType::Call, 15.0, 0.6, expiry);
        let pv_call = call.value(&market, as_of).expect("call price").amount();
        assert!(
            pv_call > 0.0,
            "Call price should be positive, got {}",
            pv_call
        );

        let put = make_spread_option(OptionType::Put, 15.0, 0.6, expiry);
        let pv_put = put.value(&market, as_of).expect("put price").amount();
        assert!(pv_put > 0.0, "Put price should be positive, got {}", pv_put);
    }

    /// Verify that the direct Black-76 put formula agrees with a 2-D Monte Carlo
    /// reference for a near-ATM spread put.
    ///
    /// Both the direct `P = df*(K_adj*N(-d2) - F1*N(-d1))` and a parity-derived
    /// `P = C - df*(F1-F2-K)` are numerically identical (Black-76 satisfies
    /// put-call parity exactly). This test simply confirms the implementation
    /// produces a put price consistent with an independent MC simulation within
    /// Kirk's approximation tolerance (~3%).
    #[test]
    fn put_priced_directly_matches_monte_carlo() {
        use std::f64::consts::PI;

        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2026, time::Month::January, 1).expect("valid date");

        // Near-ATM parameters: spread = F1-F2 = 10, strike K=10
        let f1 = 110.0_f64;
        let f2 = 100.0_f64;
        let k = 10.0_f64; // near-ATM: F1 - F2 = 10 = K
        let vol1 = 0.20_f64;
        let vol2 = 0.20_f64;
        let rho = 0.5_f64;
        let rate = 0.05_f64;
        let t = 1.0_f64;

        let market = make_market(as_of, f1, f2, vol1, vol2, rate);
        let put = make_spread_option(OptionType::Put, k, rho, expiry);
        let kirk_put = put.value(&market, as_of).expect("kirk put").amount();

        // 2-D MC reference: simulate correlated lognormals
        // F1_T = F1 * exp(-0.5*vol1^2*T + vol1*sqrt(T)*Z1)
        // F2_T = F2 * exp(-0.5*vol2^2*T + vol2*sqrt(T)*Z2)
        // where Z2 = rho*Z1 + sqrt(1-rho^2)*Z_indep
        let n_paths = 500_000_usize;
        let df = (-rate * t).exp();

        // Use a deterministic Box-Muller sequence (LCG seed) for reproducibility
        let mut payoff_sum = 0.0_f64;
        let mut seed: u64 = 12345678901234567;
        let lcg_a: u64 = 6364136223846793005;
        let lcg_c: u64 = 1442695040888963407;

        for _ in 0..n_paths {
            // Generate two independent standard normals via Box-Muller
            seed = seed.wrapping_mul(lcg_a).wrapping_add(lcg_c);
            let u1 = (seed >> 11) as f64 / (1u64 << 53) as f64;
            seed = seed.wrapping_mul(lcg_a).wrapping_add(lcg_c);
            let u2 = (seed >> 11) as f64 / (1u64 << 53) as f64;

            let u1 = u1.max(1e-300);
            let u2 = u2.max(1e-300);
            let z1 = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
            let z_ind = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).sin();
            let z2 = rho * z1 + (1.0 - rho * rho).sqrt() * z_ind;

            let f1_t = f1 * (-0.5 * vol1 * vol1 * t + vol1 * t.sqrt() * z1).exp();
            let f2_t = f2 * (-0.5 * vol2 * vol2 * t + vol2 * t.sqrt() * z2).exp();

            let spread_t = f1_t - f2_t;
            payoff_sum += (k - spread_t).max(0.0);
        }

        let mc_put = df * payoff_sum / n_paths as f64;

        // Kirk is an approximation; MC tolerance is generous at 3%
        let tol = 0.03 * mc_put;
        assert!(
            (kirk_put - mc_put).abs() < tol,
            "Kirk put ({:.6}) deviates too much from MC reference ({:.6}), diff={:.6}, tol={:.6}",
            kirk_put,
            mc_put,
            (kirk_put - mc_put).abs(),
            tol
        );
    }

    /// Test that a spread CALL with K < -F2 (i.e. k_adj = F2+K < 0) returns
    /// the discounted intrinsic value (df*(F1-F2-K)) and is NOT NaN.
    ///
    /// The old code called Black76(F1, k_adj<0, ...) which computed ln(F1/k_adj)
    /// of a negative number, producing NaN.
    #[test]
    fn negative_adjusted_strike_call_returns_intrinsic_not_nan() {
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        // K = -150, F2 = 80 => k_adj = 80 + (-150) = -70 < 0
        // The call is deeply in-the-money: F1 - F2 - K = 100 - 80 - (-150) = 170
        let f1 = 100.0_f64;
        let f2 = 80.0_f64;
        let k = -150.0_f64;
        let rate = 0.05_f64;

        let market = make_market(as_of, f1, f2, 0.25, 0.30, rate);
        let call = make_spread_option(OptionType::Call, k, 0.5, expiry);

        let pv = call
            .value(&market, as_of)
            .expect("negative k_adj call")
            .amount();

        assert!(
            pv.is_finite(),
            "Call with k_adj<0 must be finite, got {}",
            pv
        );
        assert!(!pv.is_nan(), "Call with k_adj<0 must not be NaN, got {}", pv);

        // Expected: discounted intrinsic = df * (F1 - F2 - K) = df * 170
        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc
            .df_between_dates(as_of, expiry)
            .expect("discount factor");
        let expected = df * (f1 - f2 - k); // 170 * df, always > 0

        assert!(
            (pv - expected).abs() < 1e-8,
            "Call with k_adj<0 should equal discounted intrinsic ({:.6}), got {:.6}",
            expected,
            pv
        );
    }

    /// Test that a spread PUT with K < -F2 (k_adj < 0) returns 0.
    ///
    /// When k_adj < 0, the call is worth its full intrinsic (always exercised),
    /// and put-call parity implies the put is worthless.
    #[test]
    fn negative_adjusted_strike_put_returns_zero() {
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        // K = -150, F2 = 80 => k_adj = -70 < 0
        let f1 = 100.0_f64;
        let f2 = 80.0_f64;
        let k = -150.0_f64;

        let market = make_market(as_of, f1, f2, 0.25, 0.30, 0.05);
        let put = make_spread_option(OptionType::Put, k, 0.5, expiry);

        let pv = put
            .value(&market, as_of)
            .expect("negative k_adj put")
            .amount();

        assert!(
            pv.is_finite(),
            "Put with k_adj<0 must be finite, got {}",
            pv
        );
        assert!(
            pv.abs() < 1e-12,
            "Put with k_adj<0 should be 0 (worthless), got {}",
            pv
        );
    }

    #[test]
    fn post_expiry_returns_zero() {
        let as_of = time::Date::from_calendar_date(2025, time::Month::July, 2).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let market = make_market(as_of, 100.0, 80.0, 0.25, 0.30, 0.05);
        let opt = make_spread_option(OptionType::Call, 10.0, 0.6, expiry);

        let pv = opt.value(&market, as_of).expect("post-expiry").amount();
        assert!(
            pv.abs() < 1e-12,
            "Post-expiry option should be zero, got {}",
            pv
        );
    }

    #[test]
    fn large_positive_spread_deep_itm_call() {
        // Deep ITM call: F1 - F2 - K >> 0, should approach DF * (F1 - F2 - K)
        let as_of =
            time::Date::from_calendar_date(2025, time::Month::January, 1).expect("valid date");
        let expiry =
            time::Date::from_calendar_date(2025, time::Month::July, 1).expect("valid date");

        let f1 = 200.0;
        let f2 = 80.0;
        let k = 10.0; // spread = 120, very deep ITM
        let rate = 0.05;

        let market = make_market(as_of, f1, f2, 0.20, 0.20, rate);
        let opt = make_spread_option(OptionType::Call, k, 0.8, expiry);

        let pv = opt.value(&market, as_of).expect("deep ITM call").amount();

        let disc = market.get_discount("USD-OIS").expect("discount curve");
        let df = disc
            .df_between_dates(as_of, expiry)
            .expect("discount factor");
        let intrinsic = (f1 - f2 - k) * df;

        // Deep ITM call should be close to but slightly above intrinsic
        assert!(
            pv >= intrinsic - 1e-6,
            "Deep ITM call ({}) should be >= discounted intrinsic ({})",
            pv,
            intrinsic
        );
    }
}
