//! Commodity Asian option analytical pricers.
//!
//! Provides analytical pricing for commodity Asian options using forward prices
//! from a price curve. Key difference from equity Asian: forward prices are read
//! from the curve for each fixing date, not derived from spot × exp((r-q)t).
//!
//! # Pricing Approach
//!
//! 1. For each future fixing date, read F(t_i) from the forward price curve
//! 2. Compute average forward: `F_avg = (Σ realized + Σ F(t_i)) / n`
//! 3. For geometric: use Kemna-Vorst with adjusted moments from forwards
//! 4. For arithmetic: use Turnbull-Wakeman moment-matching with forward prices
//!
//! # References
//!
//! - Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
//!   Based on Average Asset Values."
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing
//!   European Average Options."

use crate::instruments::commodity::commodity_asian_option::types::CommodityAsianOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::asian_option::AveragingMethod;
use crate::instruments::OptionType;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

/// Present value for a commodity Asian option.
pub(crate) fn compute_pv(
    inst: &CommodityAsianOption,
    market: &MarketContext,
    as_of: Date,
) -> finstack_core::Result<Money> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

    let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);
    let total_fixings = inst.fixing_dates.len();

    if total_fixings == 0 {
        return Err(finstack_core::Error::Validation(
            "CommodityAsianOption requires at least one fixing date".to_string(),
        ));
    }

    // Handle expired / fully observed options
    if t <= 0.0 {
        let average = if hist_count > 0 {
            match inst.averaging_method {
                AveragingMethod::Arithmetic => hist_sum / hist_count as f64,
                AveragingMethod::Geometric => (hist_prod_log / hist_count as f64).exp(),
            }
        } else {
            // Fallback: use spot from forward curve
            let price_curve = market.get_price_curve(inst.forward_curve_id.as_str())?;
            price_curve.spot_price()
        };

        let intrinsic = match inst.option_type {
            OptionType::Call => (average - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - average).max(0.0),
        };
        return Ok(Money::new(
            intrinsic * inst.quantity,
            inst.underlying.currency,
        ));
    }

    // Get discount curve
    let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;

    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &inst.pricing_overrides.market_quotes,
        market,
        inst.vol_surface_id.as_str(),
        t,
        inst.strike,
    )?;

    // Get forward prices for all future fixing dates
    let price_curve = market.get_price_curve(inst.forward_curve_id.as_str())?;
    let mut future_forwards: Vec<(f64, f64)> = Vec::new(); // (time_to_fixing, forward_price)

    for &fixing_date in &inst.fixing_dates {
        if fixing_date > as_of {
            let t_i =
                inst.day_count
                    .year_fraction(as_of, fixing_date, DayCountContext::default())?;
            if t_i > 0.0 {
                let fwd = price_curve.price_on_date(fixing_date)?;
                future_forwards.push((t_i, fwd));
            }
        }
    }

    let future_count = future_forwards.len();

    // All fixings already observed but not yet settled
    if future_count == 0 {
        let average = match inst.averaging_method {
            AveragingMethod::Arithmetic => hist_sum / total_fixings as f64,
            AveragingMethod::Geometric => (hist_prod_log / total_fixings as f64).exp(),
        };
        let payoff = match inst.option_type {
            OptionType::Call => (average - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - average).max(0.0),
        };
        return Ok(Money::new(
            payoff * df * inst.quantity,
            inst.underlying.currency,
        ));
    }

    // Compute price based on averaging method
    let price = match inst.averaging_method {
        AveragingMethod::Geometric => {
            if hist_count > 0 {
                // Seasoned geometric: use adjusted strike method.
                // K_adj = (K^n / exp(hist_prod_log))^(1/m) where m = future fixings
                price_seasoned_geometric_commodity(
                    &future_forwards,
                    inst.strike,
                    sigma,
                    df,
                    inst.option_type,
                    hist_prod_log,
                    hist_count,
                    total_fixings,
                )
            } else {
                price_geometric_kv_commodity(
                    &future_forwards,
                    inst.strike,
                    sigma,
                    df,
                    inst.option_type,
                )
            }
        }
        AveragingMethod::Arithmetic => price_arithmetic_tw_commodity(
            &future_forwards,
            inst.strike,
            sigma,
            df,
            inst.option_type,
            hist_sum,
            total_fixings,
        ),
    };

    Ok(Money::new(price * inst.quantity, inst.underlying.currency))
}

/// Geometric Asian pricing with commodity forwards (Kemna-Vorst adapted).
///
/// For commodity forwards, the geometric average of forwards has a lognormal
/// distribution. We compute the adjusted forward and volatility from the
/// forward prices directly.
///
/// # Variance Calculation
///
/// Uses the exact variance formula for non-equally-spaced observation times:
/// ```text
/// sigma_G^2 = (1/n^2) * sum_i sum_j sigma^2 * min(t_i, t_j)
/// ```
/// This correctly handles irregular fixing schedules (different month lengths,
/// business day adjustments) unlike the simplified equally-spaced formula.
fn price_geometric_kv_commodity(
    future_forwards: &[(f64, f64)], // (time, forward_price)
    strike: f64,
    sigma: f64,
    df: f64,
    option_type: OptionType,
) -> f64 {
    let n = future_forwards.len() as f64;
    if n == 0.0 {
        return 0.0;
    }

    // Geometric mean of forwards: G = exp((1/n) Σ ln(F_i))
    let log_sum: f64 = future_forwards.iter().map(|(_, f)| f.ln()).sum();
    let geo_mean_fwd = (log_sum / n).exp();

    // Adjusted volatility using exact variance for non-equally-spaced observations:
    // sigma_G^2 = (1/n^2) * sum_i sum_j sigma^2 * min(t_i, t_j)
    let mut var_sum = 0.0;
    for (t_i, _) in future_forwards.iter() {
        for (t_j, _) in future_forwards.iter() {
            var_sum += sigma * sigma * t_i.min(*t_j);
        }
    }
    let vol_adj_sq = var_sum / (n * n);
    let vol_adj = vol_adj_sq.sqrt();

    // Time to last fixing
    let t_last = future_forwards
        .iter()
        .map(|(t, _)| *t)
        .fold(0.0_f64, f64::max);

    if vol_adj <= 0.0 || t_last <= 0.0 {
        let intrinsic = match option_type {
            OptionType::Call => (geo_mean_fwd - strike).max(0.0),
            OptionType::Put => (strike - geo_mean_fwd).max(0.0),
        };
        return intrinsic * df;
    }

    // Black-76 style pricing with geometric mean forward
    // Use vol_adj_sq directly (it represents total variance) rather than vol_adj * sqrt(t)
    let total_vol = vol_adj_sq.sqrt();
    // d1/d2 intentionally inline: Pre-computed adjusted variance, not decomposable into sigma,t
    let d1 = ((geo_mean_fwd / strike).ln() + 0.5 * vol_adj_sq) / total_vol;
    let d2 = d1 - total_vol;

    let price = match option_type {
        OptionType::Call => {
            geo_mean_fwd * finstack_core::math::norm_cdf(d1)
                - strike * finstack_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_core::math::norm_cdf(-d2)
                - geo_mean_fwd * finstack_core::math::norm_cdf(-d1)
        }
    };

    price * df
}

/// Seasoned geometric Asian pricing with adjusted strike.
///
/// For a seasoned geometric Asian with `hist_count` realized fixings and
/// `m` future fixings remaining, we compute the adjusted strike:
/// ```text
/// K_adj = (K^n / exp(hist_prod_log))^(1/m)
/// ```
/// Then price a fresh geometric Asian on the remaining fixings with the
/// adjusted strike. This maintains consistent hedge ratios as fixings
/// are observed (no discontinuous jump from geometric to arithmetic).
///
/// # References
///
/// Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
/// Based on Average Asset Values." - Section on seasoned options.
#[allow(clippy::too_many_arguments)]
fn price_seasoned_geometric_commodity(
    future_forwards: &[(f64, f64)], // (time, forward_price)
    strike: f64,
    sigma: f64,
    df: f64,
    option_type: OptionType,
    hist_prod_log: f64,
    _hist_count: usize,
    total_fixings: usize,
) -> f64 {
    let n = total_fixings as f64;
    let m = future_forwards.len() as f64;

    if m == 0.0 {
        return 0.0;
    }

    // Adjusted strike: K_adj = (K^n / exp(hist_prod_log))^(1/m)
    // In log space: ln(K_adj) = (n * ln(K) - hist_prod_log) / m
    let ln_k_adj = (n * strike.ln() - hist_prod_log) / m;

    // If adjusted strike is degenerate (non-finite from bad inputs), return intrinsic
    if !ln_k_adj.is_finite() {
        let log_sum: f64 = future_forwards.iter().map(|(_, f)| f.ln()).sum();
        let geo_avg_all = ((hist_prod_log + log_sum) / n).exp();
        let payoff = match option_type {
            OptionType::Call => (geo_avg_all - strike).max(0.0),
            OptionType::Put => (strike - geo_avg_all).max(0.0),
        };
        return payoff * df;
    }

    let k_adj = ln_k_adj.exp();

    // Price a fresh geometric Asian on remaining fixings with adjusted strike.
    // The adjusted-strike transform already encodes the realized-fixing history,
    // so the result should not be scaled again by m/n.
    price_geometric_kv_commodity(future_forwards, k_adj, sigma, df, option_type)
}

/// Arithmetic Asian pricing with commodity forwards (Turnbull-Wakeman adapted).
///
/// Uses moment matching on the forward prices. For commodity forwards, the
/// first moment is simply the average of forward prices, and the second moment
/// accounts for correlations between forward prices.
///
/// # Seasoned Option Handling
///
/// For seasoned options with `hist_count > 0` realized fixings:
/// - Effective strike: `K_eff = (n × K - hist_sum) / m`
/// - Scale factor: `m / n` applied to the result
fn price_arithmetic_tw_commodity(
    future_forwards: &[(f64, f64)], // (time, forward_price)
    strike: f64,
    sigma: f64,
    df: f64,
    option_type: OptionType,
    hist_sum: f64,
    total_fixings: usize,
) -> f64 {
    let n = total_fixings as f64;
    let m = future_forwards.len() as f64;

    if m == 0.0 {
        return 0.0;
    }

    // Effective strike adjustment for seasoned options
    let k_eff = (n * strike - hist_sum) / m;
    let scale = m / n;

    // If effective strike is negative, option is deep ITM
    if k_eff < 0.0 {
        let sum_fwd: f64 = future_forwards.iter().map(|(_, f)| f).sum();
        let avg_fwd = (hist_sum + sum_fwd) / n;
        let payoff = match option_type {
            OptionType::Call => (avg_fwd - strike).max(0.0),
            OptionType::Put => 0.0,
        };
        return payoff * df;
    }

    // First moment: E[A_future] = (1/m) Σ F(t_i)
    let m1 = future_forwards.iter().map(|(_, f)| f).sum::<f64>() / m;

    // Second moment: E[A_future²]
    // E[F(t_i) × F(t_j)] = F(t_i) × F(t_j) × exp(σ² × min(t_i, t_j))
    // This is the Turnbull-Wakeman moment for correlated commodity forwards
    let mut sum_m2 = 0.0;
    for (t_i, f_i) in future_forwards.iter() {
        for (t_j, f_j) in future_forwards.iter() {
            let t_min = t_i.min(*t_j);
            sum_m2 += f_i * f_j * (sigma * sigma * t_min).exp();
        }
    }
    let m2 = sum_m2 / (m * m);

    // Match to lognormal
    if m2 <= m1 * m1 {
        return df * scale * (m1 - k_eff).max(0.0);
    }

    let var = (m2 / (m1 * m1)).ln();
    if var <= 0.0 {
        return df * scale * (m1 - k_eff).max(0.0);
    }

    let sigma_star = var.sqrt();
    let mu_star = m1.ln() - 0.5 * var;

    let d1 = (mu_star - k_eff.ln() + var) / sigma_star;
    let d2 = d1 - sigma_star;

    let price = match option_type {
        OptionType::Call => {
            m1 * finstack_core::math::norm_cdf(d1) - k_eff * finstack_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            k_eff * finstack_core::math::norm_cdf(-d2) - m1 * finstack_core::math::norm_cdf(-d1)
        }
    };

    (price * df * scale).max(0.0)
}

// ========================= REGISTRY PRICER =========================

/// Commodity Asian option analytical pricer (Turnbull-Wakeman / Kemna-Vorst).
pub struct CommodityAsianOptionAnalyticalPricer;

impl CommodityAsianOptionAnalyticalPricer {
    /// Create a new commodity Asian option pricer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommodityAsianOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CommodityAsianOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::CommodityAsianOption,
            ModelKey::AsianTurnbullWakeman,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian = instrument
            .as_any()
            .downcast_ref::<CommodityAsianOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CommodityAsianOption, instrument.key())
            })?;

        let pv = compute_pv(asian, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::CommodityUnderlyingParams;
    use crate::instruments::exotics::asian_option::AveragingMethod;
    use crate::instruments::PricingOverrides;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn flat_vol_surface(id: &str, expiries: &[f64], strikes: &[f64], vol: f64) -> VolSurface {
        let mut builder = VolSurface::builder(id).expiries(expiries).strikes(strikes);
        for _ in expiries {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        builder.build().expect("vol surface should build in tests")
    }

    fn build_commodity_market(
        as_of: Date,
        flat_forward_price: f64,
        vol: f64,
        rate: f64,
    ) -> MarketContext {
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [60.0, 70.0, 75.0, 80.0, 90.0];

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("CL-FORWARD")
            .base_date(as_of)
            .spot_price(flat_forward_price)
            .knots([(0.0, flat_forward_price), (2.0, flat_forward_price)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .insert(disc)
            .insert(price_curve)
            .insert_surface(flat_vol_surface("CL-VOL", &expiries, &strikes, vol))
    }

    fn build_contango_market(
        as_of: Date,
        spot: f64,
        far_price: f64,
        vol: f64,
        rate: f64,
    ) -> MarketContext {
        let expiries = [0.25, 0.5, 1.0, 2.0];
        let strikes = [60.0, 70.0, 75.0, 80.0, 90.0];

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("CL-FORWARD")
            .base_date(as_of)
            .spot_price(spot)
            .knots([(0.0, spot), (1.0, far_price)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .insert(disc)
            .insert(price_curve)
            .insert_surface(flat_vol_surface("CL-VOL", &expiries, &strikes, vol))
    }

    fn base_option(fixing_dates: Vec<Date>, settlement: Date) -> CommodityAsianOption {
        CommodityAsianOption::builder()
            .id(InstrumentId::new("TEST-ASIAN"))
            .underlying(CommodityUnderlyingParams::new(
                "Energy",
                "CL",
                "BBL",
                Currency::USD,
            ))
            .strike(75.0)
            .option_type(OptionType::Call)
            .averaging_method(AveragingMethod::Arithmetic)
            .fixing_dates(fixing_dates)
            .quantity(1000.0)
            .expiry(settlement)
            .forward_curve_id(CurveId::new("CL-FORWARD"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_surface_id(CurveId::new("CL-VOL"))
            .day_count(DayCount::Act365F)
            .pricing_overrides(PricingOverrides::default())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("should build")
    }

    #[test]
    fn test_flat_forward_call_positive() {
        let as_of = date(2025, 1, 3);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
            date(2025, 6, 30),
        ];
        let settlement = date(2025, 7, 2);
        let option = base_option(fixing_dates, settlement);
        let market = build_commodity_market(as_of, 80.0, 0.30, 0.05);

        let pv = option
            .value(&market, as_of)
            .expect("pricing should succeed");
        assert!(
            pv.amount() > 0.0,
            "ITM call should have positive value, got {}",
            pv.amount()
        );
    }

    #[test]
    fn test_flat_forward_put_positive() {
        let as_of = date(2025, 1, 3);
        let fixing_dates = vec![date(2025, 1, 31), date(2025, 2, 28), date(2025, 3, 31)];
        let settlement = date(2025, 4, 2);

        let mut option = base_option(fixing_dates, settlement);
        option.option_type = OptionType::Put;
        option.strike = 80.0;

        let market = build_commodity_market(as_of, 75.0, 0.30, 0.05);

        let pv = option
            .value(&market, as_of)
            .expect("pricing should succeed");
        assert!(
            pv.amount() > 0.0,
            "ITM put should have positive value, got {}",
            pv.amount()
        );
    }

    #[test]
    fn test_geometric_vs_arithmetic_ordering() {
        // Geometric average ≤ Arithmetic average (AM-GM inequality)
        // So geometric Asian call ≤ arithmetic Asian call
        let as_of = date(2025, 1, 3);
        let fixing_dates = vec![
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
            date(2025, 6, 30),
        ];
        let settlement = date(2025, 7, 2);

        let arith = base_option(fixing_dates.clone(), settlement);

        let mut geom = base_option(fixing_dates, settlement);
        geom.averaging_method = AveragingMethod::Geometric;

        let market = build_commodity_market(as_of, 76.0, 0.25, 0.05);

        let arith_pv = arith
            .value(&market, as_of)
            .expect("arith should succeed")
            .amount();
        let geom_pv = geom
            .value(&market, as_of)
            .expect("geom should succeed")
            .amount();

        assert!(
            arith_pv >= geom_pv - 0.01 * 1000.0, // allow small tolerance scaled by quantity
            "Arithmetic {} should be >= geometric {} for calls",
            arith_pv,
            geom_pv
        );
    }

    #[test]
    fn test_seasoned_option_uses_realized_fixings() {
        let as_of = date(2025, 4, 15);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
            date(2025, 6, 30),
        ];
        let settlement = date(2025, 7, 2);

        let mut option = base_option(fixing_dates, settlement);
        // Realized fixings at high prices (ITM)
        option.realized_fixings = vec![
            (date(2025, 1, 31), 80.0),
            (date(2025, 2, 28), 82.0),
            (date(2025, 3, 31), 78.0),
        ];

        let market = build_commodity_market(as_of, 79.0, 0.25, 0.05);

        let pv = option
            .value(&market, as_of)
            .expect("seasoned pricing should succeed");
        assert!(
            pv.amount() > 0.0,
            "Seasoned ITM call should have positive value, got {}",
            pv.amount()
        );
    }

    #[test]
    fn test_expired_option_returns_intrinsic() {
        // Use as_of = expiry (not after) to avoid date range issues
        let settlement = date(2025, 7, 2);
        let as_of = settlement;
        let fixing_dates = vec![date(2025, 4, 30), date(2025, 5, 31), date(2025, 6, 30)];

        let mut option = base_option(fixing_dates, settlement);
        option.realized_fixings = vec![
            (date(2025, 4, 30), 80.0),
            (date(2025, 5, 31), 82.0),
            (date(2025, 6, 30), 78.0),
        ];

        let market = build_commodity_market(as_of, 79.0, 0.25, 0.05);

        let pv = option
            .value(&market, as_of)
            .expect("expired should succeed");
        // Average = (80+82+78)/3 = 80.0, strike = 75, intrinsic = 5 * 1000 = 5000
        let expected = (80.0 - 75.0) * 1000.0;
        assert!(
            (pv.amount() - expected).abs() < 1.0,
            "Expired call should return intrinsic {}, got {}",
            expected,
            pv.amount()
        );
    }

    #[test]
    fn test_contango_curve_affects_pricing() {
        let as_of = date(2025, 1, 3);
        let fixing_dates = vec![date(2025, 3, 31), date(2025, 6, 30), date(2025, 9, 30)];
        let settlement = date(2025, 10, 2);

        let option = base_option(fixing_dates, settlement);

        // Flat forward curve
        let flat_market = build_commodity_market(as_of, 75.0, 0.25, 0.05);
        let flat_pv = option
            .value(&flat_market, as_of)
            .expect("flat should succeed")
            .amount();

        // Contango: forward prices increase (spot=70, 1Y=80)
        let contango_market = build_contango_market(as_of, 70.0, 80.0, 0.25, 0.05);
        let contango_pv = option
            .value(&contango_market, as_of)
            .expect("contango should succeed")
            .amount();

        // With contango and ATM strike of 75, the later fixings have higher forwards
        // So the contango option should differ from flat
        assert!(
            (flat_pv - contango_pv).abs() > 0.01,
            "Contango should produce different pricing than flat (flat={}, contango={})",
            flat_pv,
            contango_pv
        );
    }

    #[test]
    fn test_registry_pricer() {
        let as_of = date(2025, 1, 3);
        let fixing_dates = vec![date(2025, 2, 28), date(2025, 3, 31), date(2025, 4, 30)];
        let settlement = date(2025, 5, 2);

        let option = base_option(fixing_dates, settlement);
        let market = build_commodity_market(as_of, 80.0, 0.25, 0.05);

        let pricer = CommodityAsianOptionAnalyticalPricer::new();
        let result = pricer
            .price_dyn(&option, &market, as_of)
            .expect("registry pricer should succeed");

        assert!(
            result.value.amount() > 0.0,
            "Registry pricer should return positive value"
        );
    }
}
