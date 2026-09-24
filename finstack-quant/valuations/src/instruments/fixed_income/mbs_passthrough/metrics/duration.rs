//! Effective duration and convexity for agency MBS.
//!
//! Effective duration and convexity measure the price sensitivity of MBS
//! to parallel shifts in interest rates, accounting for the change in
//! prepayment behavior as rates change.

use crate::cashflow::builder::specs::{PrepaymentCurve, PrepaymentModelSpec};
use crate::instruments::fixed_income::mbs_passthrough::pricer::price_mbs;
use crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use finstack_quant_core::dates::{Date, DateExt, DayCountContext};
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

use super::PREPAY_RATE_SENSITIVITY;

/// Scale a prepayment model's speed by a rate-shift multiplier.
///
/// `rate_shift` is the parallel curve shift in decimal (e.g. `+0.0025` for a
/// +25 bp bump). The prepayment speed is multiplied by `exp(-β·rate_shift)`
/// with the module-shared β ([`PREPAY_RATE_SENSITIVITY`]): a positive shift
/// (rates up) slows prepayment, a negative shift speeds it up. For a PSA
/// model the `speed_multiplier` is scaled; for constant/lockout models the
/// annual `cpr` is scaled.
fn rate_shift_prepayment(model: &PrepaymentModelSpec, rate_shift: f64) -> PrepaymentModelSpec {
    let multiplier = (-PREPAY_RATE_SENSITIVITY * rate_shift).exp();
    match &model.curve {
        Some(PrepaymentCurve::Psa { speed_multiplier }) => PrepaymentModelSpec {
            cpr: model.cpr,
            curve: Some(PrepaymentCurve::Psa {
                speed_multiplier: (speed_multiplier * multiplier).max(0.0),
            }),
        },
        Some(PrepaymentCurve::Abs { speed }) => {
            PrepaymentModelSpec::abs((speed * multiplier).clamp(0.0, 1.0))
        }
        Some(PrepaymentCurve::Vector { monthly_cpr }) => PrepaymentModelSpec::vector(
            monthly_cpr
                .iter()
                .map(|cpr| (cpr * multiplier).clamp(0.0, 1.0))
                .collect(),
        ),
        _ => PrepaymentModelSpec {
            cpr: (model.cpr * multiplier).max(0.0),
            curve: model.curve.clone(),
        },
    }
}

/// Reproject the pool's prepayment speed under a changed financing curve.
/// The refinancing proxy is the continuously compounded rate through current
/// WAM, measured from valuation date. Parallel and bucketed risk use this same
/// rate difference and the existing empirical prepayment multiplier.
pub(crate) fn rate_risk_pool(
    mbs: &AgencyMbsPassthrough,
    base: &MarketContext,
    bumped: &MarketContext,
    as_of: Date,
) -> Result<AgencyMbsPassthrough> {
    let end = as_of.add_months(mbs.wam as i32);
    let base_curve = base.get_discount(&mbs.discount_curve_id)?;
    let bumped_curve = bumped.get_discount(&mbs.discount_curve_id)?;
    let horizon = base_curve
        .day_count()
        .year_fraction(as_of, end, DayCountContext::default())?;
    if horizon <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "MBS rate risk requires positive remaining WAM".into(),
        ));
    }
    let shift = (base_curve.df_between_dates(as_of, end)?.ln()
        - bumped_curve.df_between_dates(as_of, end)?.ln())
        / horizon;
    let mut pool = mbs.clone();
    pool.prepayment_model = rate_shift_prepayment(&mbs.prepayment_model, shift);
    Ok(pool)
}

/// Duration and convexity result.
#[derive(Debug, Clone)]
pub(crate) struct DurationResult {
    /// Effective duration (years)
    pub duration: f64,
    /// Effective convexity (years^2)
    pub convexity: f64,
}

/// Calculate both effective duration and convexity in one pass.
///
/// This is more efficient than calculating them separately as it
/// only requires three price calculations.
pub(crate) fn duration_convexity(
    mbs: &AgencyMbsPassthrough,
    market: &MarketContext,
    as_of: Date,
    shock_bp: Option<f64>,
) -> Result<DurationResult> {
    let shock_bp = shock_bp.unwrap_or(25.0);
    if !shock_bp.is_finite() || shock_bp <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "MBS rate shock must be finite and positive in basis points".into(),
        ));
    }
    let shock = shock_bp / 10_000.0; // Convert to decimal

    let base_price = price_mbs(mbs, market, as_of)?.amount();

    if base_price.abs() < 1e-10 {
        return Ok(DurationResult {
            duration: 0.0,
            convexity: 0.0,
        });
    }

    // Create bumped markets using shared calibration bump helpers (parallel bump in bp).
    let market_up = market.bump([MarketBump::Curve {
        id: mbs.discount_curve_id.clone(),
        spec: BumpSpec::parallel_bp(shock_bp),
    }])?;
    let market_down = market.bump([MarketBump::Curve {
        id: mbs.discount_curve_id.clone(),
        spec: BumpSpec::parallel_bp(-shock_bp),
    }])?;

    // Effective measures require the prepayment speed to respond to the rate
    // bump — otherwise the projected cashflows are identical up and down and
    // agency MBS cannot exhibit its characteristic negative convexity. We
    // re-shift the pool's prepayment model in each bumped scenario: rates up
    // slow prepayment, rates down speed it up. (A static PSA bump alone leaves
    // cashflows unchanged and yields near-zero convexity.)
    let mbs_up = rate_risk_pool(mbs, market, &market_up, as_of)?;
    let mbs_down = rate_risk_pool(mbs, market, &market_down, as_of)?;

    // Get bumped prices (curve bump + rate-dependent prepayment).
    let price_up = price_mbs(&mbs_up, &market_up, as_of)?.amount();
    let price_down = price_mbs(&mbs_down, &market_down, as_of)?.amount();

    // Calculate effective duration
    // Duration = -(dP/dY) / P.
    let duration = (price_down - price_up) / (2.0 * base_price * shock);

    // Calculate effective convexity
    // Convexity = (d²P/dY²) / P = (P_up + P_down - 2×P_base) / (P_base × shock²)
    let convexity = (price_up + price_down - 2.0 * base_price) / (base_price * shock * shock);

    Ok(DurationResult {
        duration,
        convexity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::specs::PrepaymentModelSpec;
    use crate::instruments::fixed_income::mbs_passthrough::{AgencyProgram, PoolType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::bumps::BumpSpec;
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn create_test_mbs() -> AgencyMbsPassthrough {
        AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_factor(1.0)
            .wac(0.045)
            .pass_through_rate(0.04)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(1.0))
            .discount_curve_id(CurveId::new("USD-TSY"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs")
    }

    fn create_test_market(as_of: Date) -> MarketContext {
        let disc = DiscountCurve::builder("USD-TSY")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (5.0, 0.80),
                (10.0, 0.60),
                (30.0, 0.30),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("valid curve");

        let fixings = ScalarTimeSeries::new(
            "FIXING:USD-TSY",
            vec![
                (
                    Date::from_calendar_date(2024, Month::January, 1).expect("valid"),
                    0.03,
                ),
                (
                    Date::from_calendar_date(2024, Month::January, 15).expect("valid"),
                    0.03,
                ),
            ],
            None,
        )
        .expect("fixing series");

        MarketContext::new().insert(disc).insert_series(fixings)
    }

    #[test]
    fn test_effective_duration() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let duration = duration_convexity(&mbs, &market, as_of, Some(25.0))
            .expect("duration")
            .duration;

        // MBS duration can be positive or negative depending on rate environment
        // Just check it's a reasonable value
        assert!(duration.abs() < 30.0);
    }

    #[test]
    fn test_effective_convexity() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let convexity = duration_convexity(&mbs, &market, as_of, Some(25.0))
            .expect("convexity")
            .convexity;

        // MBS typically has negative convexity due to prepayment option
        // However, the sign depends on rate level and prepayment model
        // Just check it's a reasonable value
        assert!(convexity.abs() < 1000.0);
    }

    /// Item 9 regression: effective convexity must reflect rate-dependent
    /// prepayment.
    ///
    /// With a *static* PSA bump the projected cashflows are identical at the
    /// up and down shocks, so `P_up + P_down - 2*P_base ≈ 0` and the MBS shows
    /// essentially zero convexity — it can never exhibit the negative
    /// convexity that defines agency MBS. After the fix the prepayment speed
    /// re-shifts with the bump (rates down → faster prepay, rates up → slower),
    /// so a premium pool prices below the linear (duration-only) prediction in
    /// both directions and the effective convexity is materially negative.
    #[test]
    fn effective_convexity_is_negative_with_rate_dependent_prepayment() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");

        // Premium pool: pass-through well above the curve, so faster
        // prepayment (rates down) destroys value and slower prepayment
        // (rates up) extends a premium — the classic negative-convexity setup.
        let mbs = AgencyMbsPassthrough::builder()
            .id(InstrumentId::new("TEST-MBS-NEGCVX"))
            .pool_id("TEST-POOL".into())
            .agency(AgencyProgram::Fnma)
            .pool_type(PoolType::Generic)
            .original_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_face(Money::from((1_000_000_i64, Currency::USD)))
            .current_factor(1.0)
            .wac(0.075)
            .pass_through_rate(0.07)
            .servicing_fee_rate(0.0025)
            .guarantee_fee_rate(0.0025)
            .wam(360)
            .issue_date(Date::from_calendar_date(2024, Month::January, 1).expect("valid"))
            .maturity(Date::from_calendar_date(2054, Month::January, 1).expect("valid"))
            .prepayment_model(PrepaymentModelSpec::psa(2.0))
            .discount_curve_id(CurveId::new("USD-TSY"))
            .day_count(DayCount::Thirty360)
            .build()
            .expect("valid mbs");
        let market = create_test_market(as_of);

        let result = duration_convexity(&mbs, &market, as_of, Some(50.0)).expect("result");

        // The bumped prices must NOT be the symmetric (zero-convexity) pair a
        // static PSA bump produces: with rate-dependent prepayment the up/down
        // cashflows genuinely differ. Convexity = (P_up + P_down - 2 P_base) /
        // (P_base * shock^2), so the midpoint gap is convexity * P_base * shock^2 / 2.
        let base_price = price_mbs(&mbs, &market, as_of).expect("base").amount();
        let shock = 50.0 / 10_000.0;
        let midpoint_gap = result.convexity * base_price * shock * shock / 2.0;
        assert!(
            midpoint_gap.abs() > 1.0,
            "rate-dependent prepayment should make P_up+P_down-2*P_base \
             non-zero (negative convexity); got midpoint gap {midpoint_gap}"
        );

        // A premium agency MBS exhibits negative effective convexity.
        assert!(
            result.convexity < 0.0,
            "premium MBS should show negative effective convexity, got {}",
            result.convexity
        );
    }

    /// The rate-shift prepayment helper must speed up prepayment when rates
    /// fall and slow it when rates rise.
    #[test]
    fn rate_shift_prepayment_responds_to_rate_direction() {
        let base = PrepaymentModelSpec::psa(1.0);

        // Rates up (+50 bp): prepayment slows → speed multiplier below base.
        let up = rate_shift_prepayment(&base, 0.005);
        let up_smm = up.smm(60).expect("smm");
        // Rates down (-50 bp): prepayment speeds up → above base.
        let down = rate_shift_prepayment(&base, -0.005);
        let down_smm = down.smm(60).expect("smm");
        let base_smm = base.smm(60).expect("smm");

        assert!(
            up_smm < base_smm,
            "rates up should slow prepayment: up SMM {up_smm} >= base {base_smm}"
        );
        assert!(
            down_smm > base_smm,
            "rates down should speed prepayment: down SMM {down_smm} <= base {base_smm}"
        );
    }

    #[test]
    fn test_duration_convexity_combined() {
        let mbs = create_test_mbs();
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);

        let result = duration_convexity(&mbs, &market, as_of, Some(25.0)).expect("result");

        // The combined pass must agree with the single-measure entry points,
        // and a par-area pass-through loses value when rates rise.
        let duration = effective_duration(&mbs, &market, as_of, Some(25.0)).expect("duration");
        let convexity = effective_convexity(&mbs, &market, as_of, Some(25.0)).expect("convexity");
        assert_eq!(result.duration, duration);
        assert_eq!(result.convexity, convexity);
        assert!(result.duration > 0.0, "duration {}", result.duration);
    }

    #[test]
    fn test_bump_discount_curve() {
        let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid");
        let market = create_test_market(as_of);
        let curve_id = CurveId::new("USD-TSY");

        let mut bumped_market = market.clone();
        bumped_market
            .apply_curve_bump_in_place(&curve_id, BumpSpec::parallel_bp(100.0))
            .expect("bump");

        let original = market.get_discount(&curve_id).expect("original");
        let bumped = bumped_market.get_discount(&curve_id).expect("bumped");

        // Bumped curve should have lower discount factors (higher rates)
        let df_original = original.df(5.0);
        let df_bumped = bumped.df(5.0);

        assert!(df_bumped < df_original);
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use crate::instruments::{Instrument, PricingOptions};
    use crate::metrics::MetricId;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    #[test]
    fn mbs_dv01_reprojects_the_same_prepayments_as_duration() {
        let as_of = date!(2024 - 01 - 15);
        let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
        mbs.metric_pricing_overrides.bump_config.rate_bump_bp = Some(1.0);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (40.0, (-0.04_f64 * 40.0).exp())])
                .build()
                .expect("curve"),
        );
        let dynamic = duration_convexity(&mbs, &market, as_of, Some(1.0)).expect("duration");
        let result = mbs
            .price_with_metrics(&market, as_of, &[MetricId::Dv01], PricingOptions::default())
            .expect("metrics");
        // Duration = (P_down - P_up) / (2 P_base shock), so the 1bp central
        // difference (P_up - P_down) / 2 is -Duration * P_base * 1e-4.
        let base_price = price_mbs(&mbs, &market, as_of).expect("base").amount();
        let expected = -dynamic.duration * base_price * 1e-4;
        assert!(
            (result.measures["dv01"] - expected).abs() < 1e-7,
            "dv01 {} versus {expected}",
            result.measures["dv01"]
        );
    }

    #[test]
    fn mbs_bucketed_rate_risk_reconciles_with_parallel_projection_risk() {
        let as_of = date!(2024 - 01 - 15);
        let mbs = AgencyMbsPassthrough::example().expect("mbs");
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (40.0, (-0.04_f64 * 40.0).exp())])
                .build()
                .expect("curve"),
        );
        let result = mbs
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Dv01, MetricId::BucketedDv01],
                PricingOptions::default(),
            )
            .expect("metrics");
        let parallel = result.measures["dv01"];
        let bucketed = result.measures["bucketed_dv01"];
        assert!(
            (parallel - bucketed).abs() < 0.001 * parallel.abs(),
            "parallel {parallel} versus buckets {bucketed}"
        );
    }

    #[test]
    fn duration_is_positive_for_a_fixed_prepayment_pool() {
        let as_of = date!(2024 - 01 - 15);
        let mut mbs = AgencyMbsPassthrough::example().expect("mbs");
        mbs.prepayment_model = PrepaymentModelSpec::psa(0.0);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (40.0, (-0.04_f64 * 40.0).exp())])
                .build()
                .expect("curve"),
        );
        assert!(
            duration_convexity(&mbs, &market, as_of, None)
                .expect("duration")
                .duration
                > 0.0
        );
    }
}
