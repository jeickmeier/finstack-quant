use super::super::VolQuoteType;
use super::conventions::{
    DEFAULT_FX_DELTA_REL_TOL, FxAtmConvention, FxButterflyConvention, FxDeltaConvention,
};
use super::delta_math::{
    fx_option_delta_to_strike, fx_smile_pillars_checked, interpolate_fx_wing_vols,
    recover_fx_wing_vols,
};
use super::surface::FxDeltaVolSurface;
use crate::{
    currency::Currency,
    dates::{DayCount, DayCountContextState, VolatilityClock},
    money::fx::CurrencyPair,
};
use time::macros::date;

fn eurusd() -> CurrencyPair {
    CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("distinct pair")
}

#[test]
fn rejects_quotes_with_non_positive_implied_wing_volatilities() {
    let err = FxDeltaVolSurface::builder("EURUSD-VOL", eurusd())
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(&[0.25, 1.0])
        .atm_vols(&[0.08, 0.08])
        .rr_25d(&[0.0, 0.0])
        .bf_25d(&[-0.09, -0.02])
        .build()
        .expect_err("negative implied 25d wing vol should be rejected");
    assert!(matches!(
        err,
        crate::Error::Input(crate::InputError::NonPositiveValue)
    ));
}

#[test]
fn exposes_explicit_quote_delta_atm_butterfly_pair_and_clock_conventions() {
    let clock = VolatilityClock::try_new(
        date!(2025 - 01 - 15),
        DayCount::Act365F,
        DayCountContextState::default(),
    )
    .expect("clock");
    let surface = FxDeltaVolSurface::builder("EURUSD-VOL", eurusd())
        .delta_convention(FxDeltaConvention::ForwardUnadjusted)
        .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
        .butterfly_convention(FxButterflyConvention::Smile)
        .expiries(&[0.25, 1.0])
        .atm_vols(&[0.08, 0.09])
        .rr_25d(&[0.01, 0.015])
        .bf_25d(&[0.005, 0.007])
        .expiry_clock(clock.clone())
        .build()
        .expect("valid FX delta vol surface");

    assert_eq!(surface.pair(), eurusd());
    assert_eq!(surface.expiry_clock(), Some(&clock));
    assert_eq!(surface.quote_type(), VolQuoteType::BlackLognormal);
    assert_eq!(
        surface.delta_convention(),
        FxDeltaConvention::ForwardUnadjusted
    );
    assert_eq!(
        surface.atm_convention(),
        FxAtmConvention::DeltaNeutralStraddle
    );
    assert_eq!(surface.butterfly_convention(), FxButterflyConvention::Smile);
    assert_eq!(surface.delta_convention().to_string(), "forward_unadjusted");
    assert_eq!(
        surface.atm_convention().to_string(),
        "delta_neutral_straddle"
    );
    assert_eq!(surface.butterfly_convention().to_string(), "smile");
}

#[test]
fn checked_wing_recovery_and_interpolation_handle_cancellation_and_subnormals() {
    // Cancellation: ATM=1, BF=1e-20, RR=-2 -> call wing ~1e-20, put wing ~2+1e-20.
    let (put, call) = recover_fx_wing_vols(1.0, -2.0, 1e-20).expect("valid cancelled wing");
    assert!((put - 2.0).abs() < 1e-12);
    assert!((call - 1e-20).abs() < 1e-34);

    // Minimum positive subnormal: put = 0.5 * min_subnormal rounds to 0 -> error.
    let min_sub = f64::from_bits(1);
    assert!(recover_fx_wing_vols(min_sub, min_sub, 0.0).is_err());

    // Single-rounding interpolation across endpoints with midpoint 5 * 2^-55.
    let (_, _, call_mid) = interpolate_fx_wing_vols(
        0.5,
        (1.0, -2.5, 0.25_f64.next_up()),
        0.5,
        (1.0_f64.next_up(), -2.5, 0.25),
    )
    .expect("single-rounded wing interpolation");
    let expected = 5.0 * f64::from_bits((1023 - 55) << 52);
    assert_eq!(call_mid, expected);
}

#[test]
fn checked_delta_inversion_handles_extreme_tails_scaling_and_tol() {
    // Tiny tail put delta 1e-20.
    let k_tail = fx_option_delta_to_strike(false, 1e-20, 1.0, 0.2, 1.0, DEFAULT_FX_DELTA_REL_TOL)
        .expect("tiny tail strike");
    assert!((k_tail - 0.160_018_15).abs() < 1e-6, "got {k_tail}");

    // Extreme forward * exp(w^2 / 2) where exp(800) alone overflows binary64.
    let k_scaled =
        fx_option_delta_to_strike(true, 0.5, 1e-300, 40.0, 1.0, DEFAULT_FX_DELTA_REL_TOL)
            .expect("scaled DNS-like half-delta strike");
    assert!((k_scaled / 2.726_374_572_1e47 - 1.0).abs() < 1e-9);

    // Extremely small vol/time rounds K -> 1.0 so call delta snaps to 0.5 and fails 1e-3 check.
    assert!(
        fx_option_delta_to_strike(true, 0.25, 1.0, 1e-17, 1.0, DEFAULT_FX_DELTA_REL_TOL).is_err()
    );
}

#[test]
fn strike_ordering_rejects_crossed_pillars() {
    let (put, call) = recover_fx_wing_vols(0.2, -2.8, 1.4).expect("positive wings");
    assert!(
        fx_smile_pillars_checked(1.0, 1.0, 0.2, (put, call), None, DEFAULT_FX_DELTA_REL_TOL)
            .is_err()
    );
}
