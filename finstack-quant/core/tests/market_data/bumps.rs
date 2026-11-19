//! Tests for the surrounding crate component and its documented behavior.
//!
use super::test_helpers::{
    sample_base_correlation_curve, sample_base_date, sample_discount_curve, sample_forward_curve,
    sample_hazard_curve, sample_inflation_curve,
};
use finstack_quant_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;

#[test]
fn bump_spec_constructors_normalize_values() {
    let additive = BumpSpec::parallel_bp(25.0);
    assert_eq!(additive.mode, BumpMode::Additive);
    assert_eq!(additive.units, BumpUnits::RateBp);
    assert_eq!(additive.value, 25.0);

    let inflation = BumpSpec::inflation_shift_pct(2.5);
    assert_eq!(inflation.mode, BumpMode::Additive);
    assert_eq!(inflation.units, BumpUnits::Percent);
    assert_eq!(inflation.value, 2.5);

    let multiplier = BumpSpec::multiplier(1.1);
    assert_eq!(multiplier.mode, BumpMode::Multiplicative);
    assert_eq!(multiplier.units, BumpUnits::Factor);
    assert_eq!(multiplier.value, 1.1);
}

// Removed test for private ID helper functions

#[test]
fn discount_curve_parallel_bump_applies_rate_shift() {
    let curve = sample_discount_curve("USD-OIS");
    let bumped = curve
        .apply_bump(BumpSpec::parallel_bp(10.0))
        .expect("bump should succeed");
    assert_eq!(bumped.id().as_str(), "USD-OIS_bump_10bp");
    let original = curve.df(1.0).expect("df");
    let bumped_df = bumped.df(1.0).expect("bumped df");
    assert!(
        bumped_df < original,
        "Additive parallel bump should decrease DF values"
    );
}

#[test]
fn forward_curve_supports_additive_zero_bumps_and_rejects_multiplier() {
    let curve = sample_forward_curve("USD-LIBOR");

    let additive = curve
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: 1.5,
            bump_type: BumpType::Parallel,
        })
        .expect("percent bumps supported");
    // Fractional magnitudes keep one decimal so ±0.4bp-style bumps get
    // distinct IDs instead of colliding at "{:.0}" rounding.
    assert_eq!(additive.id().as_str(), "USD-LIBOR_bump_1.5pct");
    assert!(additive.df(1.0).expect("df") < curve.df(1.0).expect("df"));

    assert!(
        curve.apply_bump(BumpSpec::multiplier(1.1)).is_err(),
        "multiplicative bumps are rejected on ForwardCurve"
    );
}

#[test]
fn forward_curve_parallel_bump_bp_is_additive_in_continuous_zero_space() {
    let curve = ForwardCurve::builder("USD-SOFR3M")
        .base_date(sample_base_date())
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.04_f64).exp()),
            (2.0, (-0.08_f64).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let bumped = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();
    // 10bp = 0.001 continuous zero shift: P_b(t) = P(t) * exp(-0.001 * t)
    for (&t, (&orig, &b)) in curve
        .knots()
        .iter()
        .zip(curve.factors().iter().zip(bumped.factors().iter()))
    {
        let expected = orig * (-0.001 * t).exp();
        assert!((b - expected).abs() < 1e-12);
    }
}

#[test]
fn forward_curve_triangular_key_rate_weights_match_market_standard() {
    let curve = ForwardCurve::builder("USD-SOFR3M")
        .base_date(sample_base_date())
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.5, (-0.015_f64).exp()),
            (1.0, (-0.032_f64).exp()),
            (2.0, (-0.070_f64).exp()),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Bucketed bump centered at 1.0 with neighbors at 0.0 and 2.0
    let bp = 20.0;
    let bumped = curve
        .apply_bump(BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, bp))
        .unwrap();

    let bump_rate = bp / 10_000.0;
    let expected = [(0.5, 0.5), (1.0, 1.0), (2.0, 0.0)];

    for (t, w) in expected {
        let base_zero = -curve.df(t).unwrap().ln() / t;
        let bumped_zero = -bumped.df(t).unwrap().ln() / t;
        assert!(
            (bumped_zero - (base_zero + bump_rate * w)).abs() < 1e-12,
            "t={t}: expected {}, got {bumped_zero} (w={w})",
            base_zero + bump_rate * w
        );
    }
}

#[test]
fn hazard_curve_requires_additive_fraction() {
    let curve = sample_hazard_curve("CDX");
    let additive = curve
        .apply_bump(BumpSpec::parallel_bp(50.0))
        .expect("hazard supports additive bp bumps");
    assert_eq!(additive.id().as_str(), "CDX_spread_50bp");

    let err = curve.apply_bump(BumpSpec::multiplier(1.2));
    assert!(err.is_err(), "hazard curves only support additive bumps");
}

#[test]
fn hazard_curve_direct_bump_works_at_full_recovery() {
    let curve = HazardCurve::builder("CDX-RISKLESS")
        .base_date(sample_base_date())
        .recovery_rate(1.0)
        .knots([(1.0, 0.01), (3.0, 0.015)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .expect("hazard curve construction should succeed in test");

    let bumped = curve
        .apply_bump(BumpSpec::parallel_bp(25.0))
        .expect("direct hazard intensity bumps do not divide by (1 - recovery_rate)");
    assert!((bumped.hazard_rate(1.0).unwrap() - 0.0125).abs() < 1e-12);
}

#[test]
fn inflation_curve_supports_percent_shifts() {
    let curve = sample_inflation_curve("USD-CPI");
    let bumped = curve
        .apply_bump(BumpSpec::inflation_shift_pct(2.0))
        .expect("inflation shift should succeed");
    assert_eq!(bumped.id(), curve.id());
    assert!(
        bumped.cpi(1.0).expect("bumped CPI") > curve.cpi(1.0).expect("base CPI"),
        "additive percent shifts increase CPI levels"
    );
}

#[test]
fn inflation_curve_parallel_shift_is_tenor_consistent_in_zero_rate_space() {
    let curve = sample_inflation_curve("USD-CPI");
    let bumped = curve
        .apply_bump(BumpSpec::inflation_shift_pct(1.0))
        .expect("inflation shift should succeed");

    let one_year_shift =
        bumped.inflation_rate(0.0, 1.0).unwrap() - curve.inflation_rate(0.0, 1.0).unwrap();
    let two_year_shift =
        bumped.inflation_rate(0.0, 2.0).unwrap() - curve.inflation_rate(0.0, 2.0).unwrap();

    assert!(
        (one_year_shift - two_year_shift).abs() < 1e-10,
        "parallel inflation shifts should move zero rates consistently across tenors"
    );
}

#[test]
fn base_correlation_curve_accepts_only_native_shocks() {
    let curve = sample_base_correlation_curve("CDX");
    let bumped = curve
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: 0.01,
            bump_type: BumpType::Parallel,
        })
        .expect("one correlation point");
    assert_eq!(bumped.id(), curve.id());
    assert_eq!(bumped.correlations()[0], curve.correlations()[0] + 0.01);
    assert!(curve.apply_bump(BumpSpec::parallel_bp(1.0)).is_err());
    assert!(
        curve
            .apply_bump(BumpSpec::inflation_shift_pct(1.0))
            .is_err()
    );
}

// Metadata threading through rebuild paths (// "Major — market data" item 4): bumps and rolls must preserve fx_policy and
// hazard issuer/seniority/currency metadata.

#[test]
fn discount_curve_rebuild_paths_preserve_fx_policy() {
    use finstack_quant_core::market_data::term_structures::DiscountCurve;

    let curve = DiscountCurve::builder("USD-OIS-FX")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.96), (5.0, 0.90)])
        .fx_policy("triangulate_via_usd")
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let bumped = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();
    assert_eq!(bumped.fx_policy(), Some("triangulate_via_usd"));

    let key_rate = curve
        .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 2.0, 5.0, 10.0))
        .unwrap();
    assert_eq!(key_rate.fx_policy(), Some("triangulate_via_usd"));

    let rolled = curve
        .condition(sample_base_date() + time::Duration::days(183))
        .unwrap();
    assert_eq!(rolled.fx_policy(), Some("triangulate_via_usd"));

    let via_trait = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();
    assert_eq!(via_trait.fx_policy(), Some("triangulate_via_usd"));
}

#[test]
fn forward_curve_rebuild_paths_preserve_fx_policy() {
    let curve = ForwardCurve::builder("USD-SOFR-3M-FX")
        .base_date(sample_base_date())
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.96), (5.0, 0.88)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .fx_policy("triangulate_via_usd")
        .build()
        .unwrap();

    let bumped = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();
    assert_eq!(bumped.fx_policy(), Some("triangulate_via_usd"));

    let key_rate = curve
        .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 2.0, 5.0, 10.0))
        .unwrap();
    assert_eq!(key_rate.fx_policy(), Some("triangulate_via_usd"));

    let rolled = curve
        .condition(sample_base_date() + time::Duration::days(183))
        .unwrap();
    assert_eq!(rolled.fx_policy(), Some("triangulate_via_usd"));

    let via_trait = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();
    assert_eq!(via_trait.fx_policy(), Some("triangulate_via_usd"));

    assert!(curve.apply_bump(BumpSpec::multiplier(1.1)).is_err());
}

#[test]
fn hazard_curve_apply_bump_preserves_metadata_and_shifts_intensity() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::Seniority;

    let recovery = 0.40;
    let curve = HazardCurve::builder("ACME-SNR")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .recovery_rate(recovery)
        .issuer("ACME Corp")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .fx_policy("triangulate_via_usd")
        .knots([(1.0, 0.01), (5.0, 0.01)])
        .build()
        .unwrap();

    let bumped = curve.apply_bump(BumpSpec::parallel_bp(10.0)).unwrap();

    // Full metadata threaded through the rebuild.
    assert_eq!(bumped.issuer(), Some("ACME Corp"));
    assert_eq!(bumped.seniority(), Some(Seniority::Senior));
    assert_eq!(bumped.currency(), Some(Currency::USD));
    assert_eq!(bumped.fx_policy(), Some("triangulate_via_usd"));
    assert_eq!(bumped.day_count(), curve.day_count());
    assert_eq!(bumped.recovery_rate(), recovery);

    // Parallel bump shifts the raw hazard intensity directly by +10bp (0.0010),
    // without fabricating CDS par spreads or dividing by (1 - R).
    let bumped_lambda = bumped.hazard_rate(3.0).unwrap();
    let expected = 0.01 + 0.0010;
    assert!(
        (bumped_lambda - expected).abs() < 1e-12,
        "bumped hazard rate must equal {expected}, got {bumped_lambda}"
    );
}

#[test]
fn hazard_curve_roll_forward_preserves_metadata() {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::Seniority;

    let curve = HazardCurve::builder("ACME-ROLL")
        .base_date(sample_base_date())
        .issuer("ACME Corp")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .fx_policy("triangulate_via_usd")
        .knots([(1.0, 0.01), (3.0, 0.015), (5.0, 0.02)])
        .recovery_rate(0.40)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .unwrap();

    let rolled = curve
        .condition(sample_base_date() + time::Duration::days(183))
        .unwrap();
    assert_eq!(rolled.issuer(), Some("ACME Corp"));
    assert_eq!(rolled.seniority(), Some(Seniority::Senior));
    assert_eq!(rolled.currency(), Some(Currency::USD));
    assert_eq!(rolled.fx_policy(), Some("triangulate_via_usd"));
}
