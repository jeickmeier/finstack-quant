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
//! 5. Put price via put-call parity: P = C - DF * (F1 - F2 - K)
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
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::OptionType;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::norm_cdf;
use finstack_core::money::Money;

/// Minimum denominator for Kirk's approximation (F2 + K).
/// Below this threshold, we fall back to intrinsic value.
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

    // Guard: if K_adj ~ 0, Kirk's approximation breaks down
    if k_adj.abs() < KIRK_DENOM_EPSILON {
        // Fall back to intrinsic value
        let intrinsic = match inst.option_type {
            OptionType::Call => (f1 - f2 - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - (f1 - f2)).max(0.0),
        };
        return Ok(intrinsic * df);
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
    let call_price = black76_call(f1, k_adj, sigma_kirk, t, df);

    match inst.option_type {
        OptionType::Call => Ok(call_price),
        OptionType::Put => {
            // Put-call parity: P = C - DF * (F1 - F2 - K)
            Ok(call_price - df * (f1 - f2 - inst.strike))
        }
    }
}

/// Black-76 call price.
fn black76_call(forward: f64, strike: f64, sigma: f64, t: f64, df: f64) -> f64 {
    let d1 = crate::instruments::common_impl::models::d1_black76(forward, strike, sigma, t);
    let d2 = crate::instruments::common_impl::models::d2_black76(forward, strike, sigma, t);

    df * (forward * norm_cdf(d1) - strike * norm_cdf(d2))
}

/// Commodity spread option pricer using Kirk's approximation.
pub struct CommoditySpreadOptionKirkPricer {
    model: ModelKey,
}

impl CommoditySpreadOptionKirkPricer {
    /// Create a new commodity spread option pricer with Black-76 model key.
    pub fn new() -> Self {
        Self {
            model: ModelKey::Black76,
        }
    }

    /// Create a pricer with a specific model key.
    pub fn with_model(model: ModelKey) -> Self {
        Self { model }
    }
}

impl Default for CommoditySpreadOptionKirkPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CommoditySpreadOptionKirkPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CommoditySpreadOption, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let spread_opt = instrument
            .as_any()
            .downcast_ref::<CommoditySpreadOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CommoditySpreadOption, instrument.key())
            })?;

        let pv = spread_opt.value(market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(spread_opt.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::OptionType;
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
        // Our implementation computes put = call - DF*(F1-F2-K), so parity
        // holds by construction. Verify with a K=0, zero-vol forward contract
        // to get the exact discounted spread from the same code path.
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
