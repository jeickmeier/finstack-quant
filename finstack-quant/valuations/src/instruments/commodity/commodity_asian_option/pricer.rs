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
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Present value for a commodity Asian option.
pub(crate) fn compute_pv(
    inst: &CommodityAsianOption,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if as_of > inst.expiry {
        return Ok(Money::new(0.0, inst.underlying.currency));
    }
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

    // Reject duplicate / date-mismatched / missing past fixings before they
    // can silently distort the average or the seasoned effective strike
    // .
    inst.validate_realized_fixings(as_of)?;

    let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);
    let total_fixings = inst.fixing_dates.len();

    if total_fixings == 0 {
        return Err(finstack_quant_core::Error::Validation(
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
    if future_forwards
        .iter()
        .any(|(_, forward)| !forward.is_finite() || *forward <= 0.0)
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CommodityAsianOption '{}' requires finite positive forwards for its lognormal model",
            inst.id
        )));
    }

    // With a validated fixing history, every scheduled date is either realized
    // (<= as_of) or projected (> as_of). A violation here means an internal
    // bookkeeping bug (e.g. day-count rounding dropped a future fixing), not
    // bad user input .
    if hist_count + future_count != total_fixings {
        return Err(finstack_quant_core::Error::Validation(format!(
            "CommodityAsianOption '{}' fixing bookkeeping mismatch: {hist_count} realized \
             + {future_count} projected != {total_fixings} scheduled fixings",
            inst.id
        )));
    }

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
/// # Lognormal Moments (Kemna-Vorst drift correction)
///
/// Each forward is a martingale under its own delivery measure, so
/// `ln F_i(t_i) ~ N(ln F_i(0) − σ²t_i/2, σ²t_i)`. For the geometric average
/// `G = exp((1/m) Σ ln F_i(t_i))`:
///
/// ```text
/// E[ln G]   = ln(geo_mean) − (σ²/2m) Σ t_i
/// Var[ln G] = (σ²/m²) ΣΣ min(t_i, t_j)
/// E[G]      = geo_mean · exp(½·Var[ln G] − (σ²/2m) Σ t_i)
/// ```
///
/// Black-76 is then applied with forward `F_G = E[G]`. Using the raw
/// geometric mean as the forward (no drift correction) overstates `E[G]` by
/// `exp((σ²/2)(Σt/m − ΣΣmin/m²))` — several percent of an ATM premium.
///
/// # Variance Calculation
///
/// Uses the exact variance formula for non-equally-spaced observation times:
/// ```text
/// sigma_G^2 = (1/m^2) * sum_i sum_j sigma^2 * min(t_i, t_j)
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

    // Kemna-Vorst mean-log drift: E[ln G] = ln(geo_mean) − (σ²/2m) Σ t_i
    let sum_t: f64 = future_forwards.iter().map(|(t, _)| *t).sum();
    let mean_log_drift = -0.5 * sigma * sigma * sum_t / n;

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

    // Black-76 with the expected geometric average as the forward:
    // F_G = E[G] = geo_mean · exp(mean_log_drift + ½·vol_adj_sq).
    let fwd_g = geo_mean_fwd * (mean_log_drift + 0.5 * vol_adj_sq).exp();

    // Use vol_adj_sq directly (it represents total variance) rather than vol_adj * sqrt(t)
    let total_vol = vol_adj_sq.sqrt();
    // d1/d2 intentionally inline: Pre-computed adjusted variance, not decomposable into sigma,t
    let d1 = ((fwd_g / strike).ln() + 0.5 * vol_adj_sq) / total_vol;
    let d2 = d1 - total_vol;

    let price = match option_type {
        OptionType::Call => {
            fwd_g * finstack_quant_core::math::norm_cdf(d1)
                - strike * finstack_quant_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_quant_core::math::norm_cdf(-d2)
                - fwd_g * finstack_quant_core::math::norm_cdf(-d1)
        }
    };

    price * df
}

/// Seasoned geometric Asian pricing on commodity forwards.
///
/// Once `n − m` of the `n` fixings are realized, the geometric average factors
/// as `G = A · G_fut^(m/n)`, where `A = exp(hist_prod_log / n)` is the realized
/// contribution and `G_fut = exp((1/m) Σ ln F_j)` is the geometric mean of the
/// `m` remaining forwards. The terminal quantity `X = A · G_fut^(m/n)` is
/// lognormal, so the option prices in closed form via Black-76 against the
/// original strike `K`.
///
/// Crucially the remaining geometric mean enters at power `m/n`, not `1`: a
/// realized fixing both shrinks the remaining uncertainty
/// (`Var[ln X] = (m/n)²·Var[ln G_fut]`) and reshapes the effective forward.
/// Pricing a *fresh* geometric Asian on `G_fut` (power `1`) with an adjusted
/// strike — as a naive strike transform does — overstates the remaining
/// volatility by a factor of `n/m` and uses the wrong moneyness. This
/// formulation reduces continuously to [`price_geometric_kv_commodity`] as
/// `m → n` (no realized fixings), preserving hedge ratios across fixings.
///
/// # References
///
/// Kemna, A. G. Z., & Vorst, A. C. F. (1990). "A Pricing Method for Options
/// Based on Average Asset Values." *Journal of Banking & Finance*, 14(1),
/// 113-129 — partially-averaged (seasoned) geometric options.
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

    if m == 0.0 || n <= 0.0 {
        return 0.0;
    }

    // Geometric mean and exact log-variance of the m remaining forwards
    // (same basis as the unseasoned Kemna-Vorst pricer).
    let log_sum: f64 = future_forwards.iter().map(|(_, f)| f.ln()).sum();
    let geo_mean_fwd = (log_sum / m).exp();
    let mut var_sum = 0.0;
    for (t_i, _) in future_forwards.iter() {
        for (t_j, _) in future_forwards.iter() {
            var_sum += sigma * sigma * t_i.min(*t_j);
        }
    }
    let vol_adj_sq = var_sum / (m * m); // Var[ln G_fut]

    // Kemna-Vorst mean-log drift : each forward is a
    // martingale under its own measure, so
    //   E[ln G_fut] = ln(geo_mean_fwd) − (σ²/2m) Σ t_j.
    let sum_t: f64 = future_forwards.iter().map(|(t, _)| *t).sum();
    let mean_log_drift = -0.5 * sigma * sigma * sum_t / m;

    // X = A · G_fut^(m/n), with the Kemna-Vorst lognormal law for G_fut:
    //   ln G_fut ~ N(ln geo_mean_fwd + mean_log_drift, vol_adj_sq).
    // Then, with r = m/n:
    //   Var[ln X] = r² · vol_adj_sq
    //   E[X]      = A · geo_mean_fwd^r · exp(r · mean_log_drift + ½ · r² · vol_adj_sq)
    let realized_factor = (hist_prod_log / n).exp();
    let ratio = m / n;
    let var_x = ratio * ratio * vol_adj_sq;
    let fwd_x =
        realized_factor * geo_mean_fwd.powf(ratio) * (ratio * mean_log_drift + 0.5 * var_x).exp();

    let t_last = future_forwards
        .iter()
        .map(|(t, _)| *t)
        .fold(0.0_f64, f64::max);

    // Degenerate / zero-variance: settle at the deterministic geometric average
    // A · geo_mean_fwd^(m/n) = exp((hist_prod_log + Σ ln F_j) / n).
    if !fwd_x.is_finite() || fwd_x <= 0.0 || var_x <= 0.0 || t_last <= 0.0 {
        let geo_avg_all = ((hist_prod_log + log_sum) / n).exp();
        let payoff = match option_type {
            OptionType::Call => (geo_avg_all - strike).max(0.0),
            OptionType::Put => (strike - geo_avg_all).max(0.0),
        };
        return payoff * df;
    }

    // Black-76 on X against the ORIGINAL strike K.
    let total_vol = var_x.sqrt();
    let d1 = ((fwd_x / strike).ln() + 0.5 * var_x) / total_vol;
    let d2 = d1 - total_vol;
    let price = match option_type {
        OptionType::Call => {
            fwd_x * finstack_quant_core::math::norm_cdf(d1)
                - strike * finstack_quant_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_quant_core::math::norm_cdf(-d2)
                - fwd_x * finstack_quant_core::math::norm_cdf(-d1)
        }
    };

    price * df
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
            m1 * finstack_quant_core::math::norm_cdf(d1)
                - k_eff * finstack_quant_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            k_eff * finstack_quant_core::math::norm_cdf(-d2)
                - m1 * finstack_quant_core::math::norm_cdf(-d1)
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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use finstack_quant_core::types::{CurveId, InstrumentId};
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

    /// a scheduled fixing date on/before as_of with no
    /// realized value must be a hard error, not a silent distortion of the
    /// effective strike.
    #[test]
    fn missing_past_fixing_is_an_error() {
        let as_of = date(2025, 4, 15);
        let fixing_dates = vec![
            date(2025, 1, 31),
            date(2025, 2, 28),
            date(2025, 3, 31),
            date(2025, 4, 30),
            date(2025, 5, 31),
        ];
        let settlement = date(2025, 6, 2);

        let mut option = base_option(fixing_dates, settlement);
        // 2025-02-28 is missing.
        option.realized_fixings = vec![(date(2025, 1, 31), 80.0), (date(2025, 3, 31), 78.0)];

        let market = build_commodity_market(as_of, 79.0, 0.25, 0.05);
        let err = option
            .value(&market, as_of)
            .expect_err("missing past fixing must error");
        assert!(
            err.to_string().contains("2025-02-28"),
            "error should name the missing fixing date, got: {err}"
        );
    }

    /// duplicate realized fixings would be double-counted
    /// by `accumulated_state` and must be rejected.
    #[test]
    fn duplicate_realized_fixing_is_an_error() {
        let as_of = date(2025, 3, 15);
        let fixing_dates = vec![date(2025, 1, 31), date(2025, 2, 28), date(2025, 4, 30)];
        let settlement = date(2025, 5, 2);

        let mut option = base_option(fixing_dates, settlement);
        option.realized_fixings = vec![
            (date(2025, 1, 31), 80.0),
            (date(2025, 1, 31), 81.0),
            (date(2025, 2, 28), 78.0),
        ];

        let market = build_commodity_market(as_of, 79.0, 0.25, 0.05);
        let err = option
            .value(&market, as_of)
            .expect_err("duplicate fixing must error");
        assert!(
            err.to_string().contains("duplicate"),
            "error should mention the duplicate, got: {err}"
        );
    }

    /// a realized fixing whose date is not on the fixing
    /// schedule is a date mismatch, not silently-ignorable data.
    #[test]
    fn date_mismatched_realized_fixing_is_an_error() {
        let as_of = date(2025, 3, 15);
        let fixing_dates = vec![date(2025, 1, 31), date(2025, 2, 28), date(2025, 4, 30)];
        let settlement = date(2025, 5, 2);

        let mut option = base_option(fixing_dates, settlement);
        option.realized_fixings = vec![
            (date(2025, 1, 31), 80.0),
            // 2025-03-01 is not a scheduled fixing date.
            (date(2025, 3, 1), 79.5),
            (date(2025, 2, 28), 78.0),
        ];

        let market = build_commodity_market(as_of, 79.0, 0.25, 0.05);
        let err = option
            .value(&market, as_of)
            .expect_err("date-mismatched fixing must error");
        assert!(
            err.to_string().contains("not a scheduled fixing date"),
            "error should flag the mismatch, got: {err}"
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

    /// Fresh geometric Asian must match a direct numerical integration of
    /// `E[df·max(G − K, 0)]` under the Kemna-Vorst lognormal law for the
    /// geometric average of martingale forwards :
    /// `ln G ~ N(ln geo_mean − (σ²/2m)Σt_i, (σ²/m²)ΣΣmin(t_i,t_j))`.
    /// The pre-fix pricer used the raw geometric mean as the Black-76 forward,
    /// overstating `E[G]` by `exp((σ²/2)(Σt/m − ΣΣmin/m²))`.
    #[test]
    fn geometric_kv_matches_lognormal_integration() {
        let fwds = [(0.25_f64, 80.0_f64), (0.5, 82.0), (0.75, 84.0), (1.0, 86.0)];
        let (strike, sigma, df) = (83.0_f64, 0.30_f64, 0.97_f64);

        let analytic = price_geometric_kv_commodity(&fwds, strike, sigma, df, OptionType::Call);

        // Kemna-Vorst lognormal moments of G.
        let m = fwds.len() as f64;
        let log_sum: f64 = fwds.iter().map(|(_, f)| f.ln()).sum();
        let geo_mean = (log_sum / m).exp();
        let mut var_sum = 0.0;
        for (ti, _) in &fwds {
            for (tj, _) in &fwds {
                var_sum += sigma * sigma * ti.min(*tj);
            }
        }
        let v = var_sum / (m * m);
        let sum_t: f64 = fwds.iter().map(|(t, _)| *t).sum();
        let mu = geo_mean.ln() - 0.5 * sigma * sigma * sum_t / m;
        let sd = v.sqrt();

        let dz = 0.0005;
        let mut z = -8.0;
        let mut integral = 0.0;
        while z < 8.0 {
            let g = (mu + z * sd).exp();
            let payoff = (g - strike).max(0.0);
            let phi = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
            integral += payoff * phi * dz;
            z += dz;
        }
        let numeric = integral * df;

        assert!(
            (analytic - numeric).abs() < 2e-3 * numeric.max(1.0),
            "fresh geometric KV analytic ({analytic}) must match lognormal \
             integration ({numeric})"
        );

        // Sanity: E[G] must be *below* the raw geometric mean of forwards
        // (drift dominates the variance convexity for monotone schedules).
        let e_g = (mu + 0.5 * v).exp();
        assert!(
            e_g < geo_mean,
            "Kemna-Vorst E[G]={e_g} must be below the raw geo mean {geo_mean}"
        );
    }

    /// Seasoned geometric Asian must reduce to the unseasoned Kemna-Vorst price
    /// when no fixings are realized (m = n, A = 1): the seasoning is continuous,
    /// with no jump in price/hedge as fixings begin to season.
    #[test]
    fn seasoned_geometric_reduces_to_unseasoned_when_no_history() {
        let fwds = [(0.25_f64, 80.0_f64), (0.5, 82.0), (0.75, 84.0), (1.0, 86.0)];
        let (strike, sigma, df) = (83.0_f64, 0.30_f64, 0.97_f64);

        let fresh = price_geometric_kv_commodity(&fwds, strike, sigma, df, OptionType::Call);
        // hist_prod_log = 0, total_fixings = m  =>  A = 1, ratio = 1.
        let seasoned = price_seasoned_geometric_commodity(
            &fwds,
            strike,
            sigma,
            df,
            OptionType::Call,
            0.0,
            0,
            fwds.len(),
        );
        assert!(
            (fresh - seasoned).abs() < 1e-10,
            "seasoned must equal unseasoned at m=n: fresh={fresh} seasoned={seasoned}"
        );
    }

    /// The seasoned geometric price must match a direct numerical integration of
    /// `E[df·max(A·G_fut^(m/n) − K, 0)]` under the same lognormal model the pricer
    /// assumes for `G_fut`. This validates the (forward, variance) moments of
    /// `X = A·G_fut^(m/n)`. The previous naive strike-transform priced
    /// `max(G_fut − K_adj, 0)` — overstating the remaining variance by `n/m` — and
    /// is shown here to differ materially.
    #[test]
    fn seasoned_geometric_matches_lognormal_integration() {
        let fwds = [(0.20_f64, 78.0_f64), (0.45, 81.0), (0.70, 85.0)];
        let (strike, sigma, df) = (80.0_f64, 0.35_f64, 0.96_f64);
        // Three realized fixings (n = 6 total, m = 3 remaining).
        let hist_prod_log: f64 = [77.0_f64.ln(), 79.0_f64.ln(), 76.0_f64.ln()].iter().sum();
        let total_fixings = 6usize;

        let analytic = price_seasoned_geometric_commodity(
            &fwds,
            strike,
            sigma,
            df,
            OptionType::Call,
            hist_prod_log,
            3,
            total_fixings,
        );

        // Reconstruct the lognormal moments of G_fut used by the pricer.
        let m = fwds.len() as f64;
        let n = total_fixings as f64;
        let log_sum: f64 = fwds.iter().map(|(_, f)| f.ln()).sum();
        let geo_mean_fwd = (log_sum / m).exp();
        let mut var_sum = 0.0;
        for (ti, _) in &fwds {
            for (tj, _) in &fwds {
                var_sum += sigma * sigma * ti.min(*tj);
            }
        }
        let v = var_sum / (m * m);
        let a = (hist_prod_log / n).exp();
        let r = m / n;

        // E[df·max(A·G_fut^r − K, 0)] via fine left-Riemann quadrature in z, with
        // ln G_fut = mu + z·sd under the Kemna-Vorst law :
        // mu = ln(geo_mean) − (σ²/2m) Σ t_j.
        let sum_t: f64 = fwds.iter().map(|(t, _)| *t).sum();
        let mu = geo_mean_fwd.ln() - 0.5 * sigma * sigma * sum_t / m;
        let sd = v.sqrt();
        let dz = 0.0005;
        let mut z = -8.0;
        let mut integral = 0.0;
        while z < 8.0 {
            let g_fut = (mu + z * sd).exp();
            let x = a * g_fut.powf(r);
            let payoff = (x - strike).max(0.0);
            let phi = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
            integral += payoff * phi * dz;
            z += dz;
        }
        let numeric = integral * df;

        assert!(
            (analytic - numeric).abs() < 2e-3 * numeric.max(1.0),
            "seasoned geometric analytic ({analytic}) must match lognormal integration ({numeric})"
        );

        // The corrected price must differ materially from the old naive transform.
        let k_adj = ((n * strike.ln() - hist_prod_log) / m).exp();
        let old_naive = price_geometric_kv_commodity(&fwds, k_adj, sigma, df, OptionType::Call);
        assert!(
            (analytic - old_naive).abs() / numeric.max(1.0) > 0.05,
            "fix must materially change the price vs the naive transform: \
             corrected={analytic} naive={old_naive}"
        );
    }

    #[test]
    fn post_expiry_option_is_zero_without_market_data() {
        let option = CommodityAsianOption::example();
        let as_of = option.expiry + time::Duration::days(1);
        let pv = compute_pv(&option, &MarketContext::new(), as_of)
            .expect("settled Asian option must be zero");
        assert_eq!(pv.amount(), 0.0);
    }
}
