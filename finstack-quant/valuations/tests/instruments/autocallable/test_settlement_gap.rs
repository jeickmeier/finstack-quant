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
use time::macros::date;

fn pv(inst: &finstack_quant_valuations::instruments::equity::autocallable::Autocallable) -> f64 {
    use finstack_quant_valuations::prelude::Instrument;
    let as_of = date!(2025 - 01 - 01);
    // Near-zero vol makes every path deterministic: S(t) = S0 / DF(as_of, t)
    // (q = 0), so the MC mean equals the analytic value to high precision.
    let market = build_market_with_dc(as_of, 100.0, 1e-4, 0.03, 0.0, DayCount::Act365F);
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

    let mut deferred = at_obs.clone();
    deferred.expiry = date!(2026 - 07 - 01); // six-month settlement gap

    let as_of = date!(2025 - 01 - 01);
    let market = build_market_with_dc(as_of, 100.0, 1e-4, 0.03, 0.0, DayCount::Act365F);
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
