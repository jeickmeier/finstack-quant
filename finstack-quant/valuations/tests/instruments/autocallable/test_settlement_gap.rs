//! Settlement-gap regression: an autocallable whose `expiry` (settlement) is
//! strictly after the final observation date must FIX the terminal payoff on
//! the final observation date and only DEFER payment to expiry.
//!
//! Before the fix, the Monte Carlo path state kept overwriting `final_spot` on
//! every step at-or-past the last observation date, so the terminal payoff
//! sampled the spot at the grid endpoint (`expiry`) — picking up an extra
//! `e^{(r-q)·gap}` of risk-neutral drift that the contract does not pay.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::helpers::*;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

fn pv(inst: &finstack_quant_valuations::instruments::equity::autocallable::Autocallable) -> f64 {
    let as_of = date!(2025 - 01 - 01);
    // Near-zero vol makes every path deterministic: S(t) = S0 / DF(as_of, t)
    // (q = 0), so the MC mean equals the analytic value to high precision.
    let market = build_market_with_day_count(as_of, 100.0, 1e-4, 0.03, 0.0, DayCount::Act365F);
    inst.value(&market, as_of)
        .expect("autocallable pv")
        .amount()
}

#[test]
fn terminal_payoff_is_fixed_on_last_observation_not_at_expiry() {
    let observation_dates = vec![
        date!(2025 - 04 - 01),
        date!(2025 - 07 - 01),
        date!(2025 - 10 - 01),
        date!(2026 - 01 - 01),
    ];
    let last_obs = *observation_dates.last().unwrap();

    let mut at_obs =
        create_quarterly_autocallable(observation_dates.clone(), DayCount::Act365F, Some("base"));
    // Barriers far above the deterministic ~3%/yr drift so the note never
    // autocalls and always reaches the terminal payoff.
    at_obs.autocall_barriers = vec![1.5; observation_dates.len()];
    at_obs.coupons = vec![0.0; observation_dates.len()];

    let mut deferred = at_obs.clone();
    deferred.expiry = date!(2026 - 07 - 01); // six-month settlement gap
    let last_payment = deferred.payment_dates.len() - 1;
    deferred.payment_dates[last_payment] = deferred.expiry;

    let as_of = date!(2025 - 01 - 01);
    let market = build_market_with_day_count(as_of, 100.0, 1e-4, 0.03, 0.0, DayCount::Act365F);
    let disc = market.get_discount(DISC_ID).expect("curve");
    let df_obs = disc
        .df_between_dates(as_of, last_obs)
        .expect("df to last obs");
    let df_exp = disc
        .df_between_dates(as_of, deferred.expiry)
        .expect("df to expiry");
    assert!(df_exp < df_obs, "test needs a genuine settlement gap");

    // Deterministic terminal fixing at the LAST OBSERVATION date:
    // S_T/S0 = 1/DF(as_of, last_obs); participation payoff = 1 + (S_T/S0 - 1).
    let growth = 1.0 / df_obs;
    let expected_payoff = 1.0 + (growth - 1.0); // rate = 1.0, well under the 1.5 cap
    let expected_pv = expected_payoff * 100_000.0 * df_exp;

    let pv_deferred = pv(&deferred);
    let rel_err = (pv_deferred - expected_pv).abs() / expected_pv;
    assert!(
        rel_err < 1e-4,
        "deferred-settlement autocallable must fix the terminal payoff on the \
         final observation date and discount from expiry: pv={pv_deferred}, \
         expected={expected_pv}, rel_err={rel_err}"
    );

    // Cross-check: the payoff must be IDENTICAL to the expiry==last_obs
    // variant, differing only by the discount-factor ratio.
    let pv_at_obs = pv(&at_obs);
    let expected_from_ratio = pv_at_obs / df_obs * df_exp;
    let rel_err_ratio = (pv_deferred - expected_from_ratio).abs() / expected_from_ratio;
    assert!(
        rel_err_ratio < 1e-4,
        "settlement gap must only defer payment, not change the fixing: \
         pv_deferred={pv_deferred}, pv_at_obs={pv_at_obs}, \
         expected={expected_from_ratio}, rel_err={rel_err_ratio}"
    );

    // Sanity on the time basis the assertion relies on.
    let t_obs = DayCount::Act365F
        .year_fraction(as_of, last_obs, DayCountContext::default())
        .unwrap();
    assert!(t_obs > 0.9 && t_obs < 1.1);
}

#[test]
fn early_autocall_is_discounted_to_contractual_payment_date() {
    let as_of = date!(2025 - 01 - 01);
    let observation_dates = vec![
        date!(2025 - 04 - 01),
        date!(2025 - 07 - 01),
        date!(2025 - 10 - 01),
        date!(2026 - 01 - 01),
    ];
    let mut same_day =
        create_quarterly_autocallable(observation_dates.clone(), DayCount::Act365F, Some("base"));
    same_day.autocall_barriers = vec![1.0; observation_dates.len()];
    same_day.coupon_barriers = vec![2.0; observation_dates.len()];
    same_day.coupons = vec![0.0; observation_dates.len()];

    let mut delayed = same_day.clone();
    delayed.payment_dates[0] = date!(2025 - 05 - 01);

    let market = build_market_with_day_count(as_of, 100.0, 1e-4, 0.03, 0.0, DayCount::Act365F);
    let curve = market.get_discount(DISC_ID).expect("curve");
    let df_same = curve
        .df_between_dates(as_of, observation_dates[0])
        .expect("observation df");
    let df_delayed = curve
        .df_between_dates(as_of, delayed.payment_dates[0])
        .expect("payment df");

    let same_pv = same_day
        .value(&market, as_of)
        .expect("same-day pv")
        .amount();
    let delayed_pv = delayed.value(&market, as_of).expect("delayed pv").amount();
    let expected = same_pv * df_delayed / df_same;

    assert!((delayed_pv - expected).abs() / expected < 1e-4);
}

#[test]
fn terminal_principal_and_last_coupon_use_their_own_payment_dates() {
    use finstack_quant_valuations::instruments::equity::autocallable::FinalPayoffType;

    let as_of = date!(2025 - 01 - 01);
    let observations = vec![date!(2025 - 07 - 01), date!(2026 - 01 - 01)];
    let market = build_market_with_day_count(as_of, 100.0, 0.0, 0.08, 0.0, DayCount::Act365F);
    let curve = market.get_discount(DISC_ID).unwrap();

    for coupon in [0.0, 0.02] {
        for final_coupon_payment in [date!(2026 - 01 - 01), date!(2026 - 02 - 01)] {
            let mut note = create_quarterly_autocallable(
                observations.clone(),
                DayCount::Act365F,
                Some("separate-settlement"),
            );
            note.expiry = date!(2026 - 07 - 01);
            note.payment_dates[1] = final_coupon_payment;
            note.autocall_barriers = vec![10.0; 2];
            note.coupon_barriers = vec![0.1; 2];
            note.coupons = vec![coupon; 2];
            note.final_payoff_type = FinalPayoffType::CapitalProtection {
                floor: 1.0,
                participation_rate: 0.0,
            };

            let principal = curve.df_between_dates(as_of, note.expiry).unwrap();
            let coupons: f64 = note
                .payment_dates
                .iter()
                .map(|&date| coupon * curve.df_between_dates(as_of, date).unwrap())
                .sum();
            let expected = note.notional.amount() * (principal + coupons);
            let actual = note.value(&market, as_of).unwrap().amount();
            assert!(
                (actual - expected).abs() < 1e-7,
                "coupon={coupon}, last_coupon={final_coupon_payment}: {actual} vs {expected}"
            );

            // Roll across the final observation with its known fixing. Remove
            // coupons already paid, then compare the same remaining cashflows
            // at a common valuation date on each side of the live/fixed branch.
            let roll_date = observations[1];
            note.initial_level = Some(100.0);
            note.past_fixings = observations.iter().map(|&date| (date, 100.0)).collect();
            let rolled_pv = note.value(&market, roll_date).unwrap().amount();
            let paid_coupons: f64 = note
                .payment_dates
                .iter()
                .filter(|&&date| date <= roll_date)
                .map(|&date| {
                    coupon * note.notional.amount() * curve.df_between_dates(as_of, date).unwrap()
                })
                .sum();
            let rolled_discounted = rolled_pv * curve.df_between_dates(as_of, roll_date).unwrap();
            assert!((actual - paid_coupons - rolled_discounted).abs() < 1e-7);
        }
    }
}
