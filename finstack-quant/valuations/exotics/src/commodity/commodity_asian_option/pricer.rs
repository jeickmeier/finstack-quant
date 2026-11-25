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
//!   Based on Average Asset Values." `docs/REFERENCES.md#kemna-vorst-1990`
//! - Turnbull, S. M., & Wakeman, L. M. (1991). "A Quick Algorithm for Pricing
//!   European Average Options." `docs/REFERENCES.md#turnbull-wakeman-1991`

use crate::commodity::commodity_asian_option::types::CommodityAsianOption;
#[cfg(test)]
use crate::exotics::asian_option::pricer::moments::{
    arithmetic_moments, geometric_moments,
    price_arithmetic_tw_asian as price_arithmetic_tw_commodity,
    price_geometric_asian as price_geometric_commodity,
};
use crate::exotics::asian_option::pricer::moments::{
    price_arithmetic_tw_asian, price_geometric_asian,
};
use crate::exotics::asian_option::types::{average_intrinsic, realized_average};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, expect_inst,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// Present value for a commodity Asian option.
pub(crate) fn compute_pv(
    inst: &CommodityAsianOption,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    if as_of > inst.expiry {
        return Ok(Money::from((0_i64, inst.underlying.currency)));
    }
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

    // Reject duplicate / date-mismatched / missing past fixings before they
    // can silently distort the average or the seasoned effective strike
    // .
    inst.validate_past_fixings(as_of)?;

    let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);
    let total_fixings = inst.fixing_dates.len();
    let realized_intrinsic = |count: usize| -> finstack_quant_core::Result<f64> {
        let average = realized_average(inst.averaging_method, hist_sum, hist_prod_log, count)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "realized Asian option payoff requires at least one observed fixing".into(),
                )
            })?;
        Ok(average_intrinsic(inst.option_type, average, inst.strike))
    };

    let future_count = inst
        .fixing_dates
        .iter()
        .filter(|date| **date > as_of)
        .count();
    if future_count == 0 && t > 0.0 {
        let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
        let payoff = realized_intrinsic(total_fixings)?;
        return Money::from_model_value(
            payoff * disc_curve.df_between_dates(as_of, inst.expiry)? * inst.quantity,
            inst.underlying.currency,
        );
    }

    // Handle expired / fully observed options
    if t <= 0.0 {
        let intrinsic = realized_intrinsic(hist_count)?;
        return Money::from_model_value(intrinsic * inst.quantity, inst.underlying.currency);
    }

    let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;

    let sigma = finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
        &inst.instrument_pricing_overrides.market_quotes,
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

    // Compute price based on averaging method using shared O(m) moment-matching helpers.
    let price = match inst.averaging_method {
        AveragingMethod::Geometric => price_geometric_asian(
            &future_forwards,
            inst.strike,
            sigma,
            df,
            inst.option_type,
            hist_prod_log,
            total_fixings,
        )?,
        AveragingMethod::Arithmetic => price_arithmetic_tw_asian(
            &future_forwards,
            inst.strike,
            sigma,
            df,
            inst.option_type,
            hist_sum,
            total_fixings,
        )?,
    };

    Money::from_model_value(price * inst.quantity, inst.underlying.currency)
}

/// Commodity Asian option analytical pricer (Turnbull-Wakeman / Kemna-Vorst).
pub struct CommodityAsianOptionAnalyticalPricer;

impl Pricer for CommodityAsianOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::CommodityAsianOption,
            ModelKey::AsianTurnbullWakeman,
        )
    }

    #[tracing::instrument(
        name = "commodity_asian_option.analytical.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let asian =
            expect_inst::<CommodityAsianOption>(instrument, InstrumentType::CommodityAsianOption)?;

        let pv = compute_pv(asian, market, as_of)?;

        Ok(ValuationResult::stamped(asian.id(), as_of, pv))
    }
}

#[cfg(test)]
#[path = "../../../tests/performance/commodity_asian_option.rs"]
mod performance_accuracy;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::CommodityUnderlyingParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, PriceCurve};
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_models::types::OptionType;
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn flat_vol_surface(id: &str, expiries: &[f64], strikes: &[f64], vol: f64) -> VolSurface {
        let mut builder = VolSurface::builder(id)
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .expiries(expiries)
            .secondary_values(strikes);
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("CL-FORWARD")
            .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
            .base_date(as_of)
            .knots([(0.0, flat_forward_price), (2.0, flat_forward_price)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .try_insert(disc)
            .expect("insert market curve")
            .try_insert(price_curve)
            .expect("insert market curve")
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("discount curve");

        let price_curve = PriceCurve::builder("CL-FORWARD")
            .kind(finstack_quant_core::market_data::term_structures::PriceCurveKind::Price)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
            .base_date(as_of)
            .knots([(0.0, spot), (1.0, far_price)])
            .build()
            .expect("price curve");

        MarketContext::new()
            .try_insert(disc)
            .expect("insert market curve")
            .try_insert(price_curve)
            .expect("insert market curve")
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
            .attributes(finstack_quant_core::types::Attributes::new())
            .build()
            .expect("should build")
    }

    #[test]
    fn colliding_model_dates_cannot_turn_a_forward_quote_into_a_realized_fixing() {
        let as_of = date(2025, 1, 30);
        let fixing = date(2025, 1, 31);
        let mut option = base_option(vec![fixing], fixing);
        option.day_count = DayCount::ThirtyE360;
        let market = build_commodity_market(as_of, 80.0, 0.25, 0.05);
        let error = option.value(&market, as_of).unwrap_err();
        assert!(error.to_string().contains("observed fixing"), "{error}");
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
        option.past_fixings = vec![
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
        option.past_fixings = vec![(date(2025, 1, 31), 80.0), (date(2025, 3, 31), 78.0)];

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
        option.past_fixings = vec![
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

    /// a past fixing whose date is not on the fixing
    /// schedule is a date mismatch, not silently-ignorable data.
    #[test]
    fn date_mismatched_realized_fixing_is_an_error() {
        let as_of = date(2025, 3, 15);
        let fixing_dates = vec![date(2025, 1, 31), date(2025, 2, 28), date(2025, 4, 30)];
        let settlement = date(2025, 5, 2);

        let mut option = base_option(fixing_dates, settlement);
        option.past_fixings = vec![
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
        option.past_fixings = vec![
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

        let pricer = CommodityAsianOptionAnalyticalPricer;
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

        let analytic =
            price_geometric_commodity(&fwds, strike, sigma, df, OptionType::Call, 0.0, fwds.len())
                .expect("finite geometric commodity price");

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

        let analytic = price_geometric_commodity(
            &fwds,
            strike,
            sigma,
            df,
            OptionType::Call,
            hist_prod_log,
            total_fixings,
        )
        .expect("finite seasoned geometric price");

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
        let old_naive =
            price_geometric_commodity(&fwds, k_adj, sigma, df, OptionType::Call, 0.0, fwds.len())
                .expect("finite geometric price");
        assert!(
            (analytic - old_naive).abs() / numeric.max(1.0) > 0.05,
            "fix must materially change the price vs the naive transform: \
             corrected={analytic} naive={old_naive}"
        );
    }

    #[test]
    fn geometric_zero_vol_calls_and_puts_preserve_seasoning() {
        let forwards = [(0.15_f64, 70.0_f64), (0.8, 80.0)];
        for (historical_log, total_count) in [(0.0, 2), (60.0_f64.ln(), 3)] {
            let average =
                ((historical_log + 70.0_f64.ln() + 80.0_f64.ln()) / total_count as f64).exp();
            for strike in [average - 5.0, average, average + 5.0] {
                for kind in [OptionType::Call, OptionType::Put] {
                    let payoff = match kind {
                        OptionType::Call => (average - strike).max(0.0),
                        OptionType::Put => (strike - average).max(0.0),
                    };
                    let value = price_geometric_commodity(
                        &forwards,
                        strike,
                        0.0,
                        0.96,
                        kind,
                        historical_log,
                        total_count,
                    )
                    .expect("finite zero-vol price");
                    assert!((value - 0.96 * payoff).abs() < 1e-10);
                }
            }
        }
    }

    #[test]
    fn post_expiry_option_is_zero_without_market_data() {
        let option = CommodityAsianOption::example().expect("example");
        let as_of = option.expiry + time::Duration::days(1);
        let pv = compute_pv(&option, &MarketContext::new(), as_of)
            .expect("settled Asian option must be zero");
        assert_eq!(pv.amount(), 0.0);
    }

    #[test]
    fn arithmetic_asian_zero_vol_calls_and_puts_preserve_seasoning() {
        let forwards = [(0.25, 70.0), (0.75, 80.0)];
        let df = 0.96;
        for (historical_sum, total_count) in [(0.0, 2), (60.0, 3)] {
            let average = (historical_sum + 150.0) / total_count as f64;
            for strike in [average - 5.0, average, average + 5.0] {
                for sigma in [0.0, 1e-10, 1e-5] {
                    let call = price_arithmetic_tw_asian(
                        &forwards,
                        strike,
                        sigma,
                        df,
                        OptionType::Call,
                        historical_sum,
                        total_count,
                    )
                    .expect("finite call price");
                    let put = price_arithmetic_tw_asian(
                        &forwards,
                        strike,
                        sigma,
                        df,
                        OptionType::Put,
                        historical_sum,
                        total_count,
                    )
                    .expect("finite put price");
                    // At ATM, positive sigma retains a small time value.
                    let tolerance = if strike == average && sigma > 0.0 {
                        0.001
                    } else {
                        1e-10
                    };
                    assert!(
                        (call - df * (average - strike).max(0.0)).abs() < tolerance,
                        "call: sigma={sigma}, strike={strike}, price={call}"
                    );
                    assert!(
                        (put - df * (strike - average).max(0.0)).abs() < tolerance,
                        "put: sigma={sigma}, strike={strike}, price={put}"
                    );
                    assert!((call - put - df * (average - strike)).abs() < 1e-10);
                }
            }
        }
    }
}
