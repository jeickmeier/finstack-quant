//! Dollar roll carry and implied financing calculations.
//!
//! The dollar roll drop implies a financing rate that can be compared
//! to repo rates to assess roll "specialness".
//!
//! Carry inputs (coupon income, principal paydown) are derived from the
//! MBS cashflow engine rather than stylized amortization, ensuring
//! consistency with the TBA pricer.

use super::DollarRoll;
use crate::instruments::fixed_income::mbs_passthrough::pricer::{
    generate_cashflows, settlement_accrued_interest,
};
use crate::instruments::fixed_income::tba::pricer::create_assumed_pool;
use finstack_quant_core::Result;

/// Carry calculation result.
#[derive(Debug, Clone)]
pub struct CarryResult {
    /// Implied financing rate (annualized, ACT/360)
    pub implied_rate: f64,
    /// Dollar drop (front price - back price)
    pub drop: f64,
    /// Days between settlements
    pub settlement_days: i64,
    /// Expected coupon income during roll period (per $100 face)
    pub coupon_income: f64,
    /// Expected principal paydown during roll period (per $100 face)
    pub principal_paydown: f64,
}

/// Calculate implied financing rate from dollar roll drop.
///
/// Carry inputs (coupon income and principal paydown between settlement
/// dates) are computed from the MBS cashflow engine using the same generic
/// assumed pool, including its prepayment model, that the TBA pricer uses.
///
/// # Formula
///
/// The roll seller delivers the pool at the front settlement for the dirty
/// front price and forgoes the coupon income and the paydown's pull-to-par
/// gain over the roll, but buys back cheaper by the drop. Break-even
/// financing on the dirty proceeds therefore satisfies:
///
/// ```text
/// implied_rate = (coupon_income + paydown × (100 − back_price)/100 − drop)
///                / (front_price + front_accrued) × (360 / days)
/// ```
///
/// `front_accrued` is the pass-through coupon accrued from the first of the
/// front settlement month to the front settlement date, per 100 of face.
/// A larger drop *lowers* the implied financing rate (the roll is special /
/// cheap to finance). Principal paid down at par is not a full-par cost: the
/// seller only forgoes the pull-to-par component `(100 − back_price)` per 100
/// of paydown.
///
/// # Arguments
///
/// * `roll` - Dollar roll whose prices, settlement dates, coupon and generic
///   TBA pool assumptions determine the carry.
///
/// # Errors
///
/// Returns an error when the settlement dates or the assumed pool cannot be
/// resolved, or when the pool projection fails.
pub fn implied_financing_rate(roll: &DollarRoll) -> Result<CarryResult> {
    let days = roll.settlement_days()?;
    let drop = roll.drop();

    let front_leg = roll.front_leg()?;
    let front_settle = roll.front_settle_date()?;
    let back_settle = roll.back_settle_date()?;

    let pool = create_assumed_pool(&front_leg)?;
    let months = (back_settle.year() - front_settle.year()) * 12
        + i32::from(u8::from(back_settle.month()))
        - i32::from(u8::from(front_settle.month()));
    let cashflows = generate_cashflows(&pool, front_settle, Some(months as u32 + 1))?;
    let scale = 100.0 / pool.current_face.amount();
    let mut principal_paydown = 0.0;
    let mut coupon_income = 0.0;
    for cf in &cashflows {
        // Balance changes once at the next contractual monthly accrual boundary.
        let boundary = cf.period_end + time::Duration::days(1);
        let start = cf.period_start.max(front_settle);
        let end = boundary.min(back_settle);
        if end > start {
            coupon_income += cf.beginning_balance
                * pool.pass_through_rate
                * pool.day_count.year_fraction(
                    start,
                    end,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?
                * scale;
        }
        if boundary > front_settle && boundary <= back_settle {
            principal_paydown += (cf.scheduled_principal + cf.prepayment) * scale;
        }
    }

    // Net financing benefit forgone by the roll seller: coupon income plus the
    // paydown's pull-to-par gain, less the drop captured by buying back cheaper.
    let net_benefit = coupon_income + principal_paydown * (100.0 - roll.back_price) / 100.0 - drop;

    let dirty_front = roll.front_price + settlement_accrued_interest(&pool, front_settle)? * scale;
    let implied_rate = if days > 0 {
        (net_benefit / dirty_front) * (360.0 / days as f64)
    } else {
        0.0
    };

    Ok(CarryResult {
        implied_rate,
        drop,
        settlement_days: days,
        coupon_income,
        principal_paydown,
    })
}

/// Calculate roll specialness (implied rate vs. repo rate).
///
/// # Arguments
///
/// * `roll` - Dollar-roll contract whose front and back prices, settlement
///   dates, coupon, and TBA pool assumptions determine implied financing.
/// * `repo_rate` - Comparable annualized repo financing rate as a decimal on
///   the same ACT/360 basis used for the roll's implied financing rate.
///
/// # Returns
///
/// Roll specialness in basis points (positive = roll is special, i.e.
/// rolling is cheaper than repo financing).
///
/// # Errors
///
/// Propagates any error from [`implied_financing_rate`].
pub fn roll_specialness(roll: &DollarRoll, repo_rate: f64) -> Result<f64> {
    let carry = implied_financing_rate(roll)?;
    let specialness = repo_rate - carry.implied_rate;
    Ok(specialness * 10_000.0)
}

/// Calculate break-even drop given a target financing rate.
///
/// Inverts the implied-rate formula for the drop:
///
/// ```text
/// drop = coupon_income + paydown × (100 − back_price)/100
///        − target_rate × dirty_front_price × days/360
/// ```
///
/// # Arguments
///
/// * `target_rate` - Desired annualized financing rate as a decimal on the
///   ACT/360 convention used by dollar-roll carry.
/// * `dirty_front_price` - Front-month TBA price plus accrued interest at
///   front settlement, in points per 100 of face.
/// * `back_price` - Back-month TBA price in points per 100 of face.
/// * `coupon_income` - Expected accrued coupon income over the roll period in
///   points per 100 of face.
/// * `principal_paydown` - Expected scheduled and prepaid principal received
///   during the roll period in points per 100 of face.
/// * `days` - Actual number of calendar days between front and back settlement
///   dates, annualized on a 360-day basis.
///
/// # Returns
///
/// Break-even drop (in price points)
pub fn break_even_drop(
    target_rate: f64,
    dirty_front_price: f64,
    back_price: f64,
    coupon_income: f64,
    principal_paydown: f64,
    days: i64,
) -> f64 {
    let required_net = target_rate * dirty_front_price * (days as f64 / 360.0);
    coupon_income + principal_paydown * (100.0 - back_price) / 100.0 - required_net
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implied_financing_rate() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");

        let result = implied_financing_rate(&roll).expect("should calculate");

        assert!(result.implied_rate > -0.20);
        assert!(result.implied_rate < 0.20);
        assert!((result.drop - roll.drop()).abs() < 1e-10);
        assert!(result.coupon_income > 0.0, "should have coupon income");
        assert!(
            result.principal_paydown >= 0.0,
            "paydown should be non-negative"
        );
    }

    /// Item 13 regression: coupon income must accrue on the *declining* MBS
    /// balance, not a constant 100 face.
    ///
    /// Over the roll the pool amortizes and prepays, so the balance earning
    /// the coupon shrinks. Accruing on a flat 100 face overstates income. This
    /// test checks the carry result's `coupon_income` is strictly below the
    /// (incorrect) flat-100 accrual whenever there is any principal paydown.
    #[test]
    fn coupon_income_accrues_on_declining_balance() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let result = implied_financing_rate(&roll).expect("ok");

        // There must be some paydown for the test to be meaningful.
        assert!(
            result.principal_paydown > 0.0,
            "expected positive principal paydown over the roll"
        );

        let front = roll.front_settle_date().expect("front");
        let back = roll.back_settle_date().expect("back");
        let boundary =
            finstack_quant_core::dates::Date::from_calendar_date(back.year(), back.month(), 1)
                .expect("boundary");
        let day_count = finstack_quant_core::dates::DayCount::Thirty360;
        let first = day_count
            .year_fraction(front, boundary, Default::default())
            .expect("first");
        let second = day_count
            .year_fraction(boundary, back, Default::default())
            .expect("second");
        let expected = roll.coupon * (100.0 * first + (100.0 - result.principal_paydown) * second);
        assert!((result.coupon_income - expected).abs() < 1e-9);
    }

    #[test]
    fn test_roll_specialness() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let repo_rate = 0.05;

        let specialness = roll_specialness(&roll, repo_rate).expect("should calculate");

        // Definition: specialness = (repo − implied) in bp.
        let carry = implied_financing_rate(&roll).expect("carry");
        let expected = (repo_rate - carry.implied_rate) * 10_000.0;
        assert!(
            (specialness - expected).abs() < 1e-9,
            "specialness {specialness} should equal (repo − implied)·1e4 = {expected}"
        );

        // The example's 0.5-point drop over ~1 month dwarfs the carry, so the
        // roll screens strongly special (implied rate well below repo).
        assert!(specialness > 0.0, "example roll should be special");
        assert!(specialness < 2_000.0);
    }

    #[test]
    fn test_break_even_drop() {
        let target_rate = 0.04;
        let front_price = 98.5;
        let back_price = 98.0;
        let coupon_income = 0.333;
        let principal_paydown = 0.5;
        let days = 30;

        let break_even = break_even_drop(
            target_rate,
            front_price,
            back_price,
            coupon_income,
            principal_paydown,
            days,
        );
        assert!(break_even.abs() < 2.0);
    }

    #[test]
    fn test_carry_round_trip_consistency() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let result = implied_financing_rate(&roll).expect("ok");
        let front = roll.front_settle_date().expect("front");
        let pool = create_assumed_pool(&roll.front_leg().expect("leg")).expect("pool");
        let front_accrued = settlement_accrued_interest(&pool, front).expect("accrued") * 100.0
            / pool.current_face.amount();

        let be = break_even_drop(
            result.implied_rate,
            roll.front_price + front_accrued,
            roll.back_price,
            result.coupon_income,
            result.principal_paydown,
            result.settlement_days,
        );
        assert!(
            (be - roll.drop()).abs() < 1e-10,
            "break-even at implied rate should ≈ actual drop, got {be} vs {}",
            roll.drop()
        );
    }

    /// Regression: a larger drop must *lower* the implied financing
    /// rate (the roll is special / cheap to finance) and therefore *raise*
    /// specialness vs a fixed repo rate. The pre-fix convention
    /// (`net_benefit = drop + coupon − paydown`) moved both the wrong way.
    #[test]
    fn larger_drop_lowers_implied_rate_and_raises_specialness() {
        let base = DollarRoll::example().expect("DollarRoll example is valid");
        // Widen the drop by cheapening the back price.
        let mut wide = base.clone();
        wide.back_price = base.back_price - 0.25;
        assert!(wide.drop() > base.drop());

        let r_base = implied_financing_rate(&base).expect("base carry");
        let r_wide = implied_financing_rate(&wide).expect("wide carry");
        assert!(
            r_wide.implied_rate < r_base.implied_rate,
            "larger drop must lower implied financing rate: base={}, wide={}",
            r_base.implied_rate,
            r_wide.implied_rate
        );

        let repo = 0.05;
        let s_base = roll_specialness(&base, repo).expect("base specialness");
        let s_wide = roll_specialness(&wide, repo).expect("wide specialness");
        assert!(
            s_wide > s_base,
            "larger drop must raise specialness: base={s_base}bp, wide={s_wide}bp"
        );
    }
}

#[cfg(test)]
mod production_mortgage_audit {
    use super::*;
    use time::macros::date;

    /// Hand calculation for a Mar 11 -> Apr 11 2026 roll (31 days) on the
    /// generic 30-year 4% pool, which is issued Mar 1 and therefore has PSA
    /// seasoning 0 in March (0% CPR). March's principal is level-pay
    /// scheduled principal only, paid down at the Apr 1 boundary:
    /// `S/B = i/((1+i)^360 - 1)` with `i = WAC/12`.
    /// Coupon: 20 days (30/360) on 100, then 10 days on `100 - paydown`.
    /// Front accrued: Mar 1 -> Mar 11 = 10/360 of the 4% coupon.
    /// `implied = (coupon + paydown x (100 - 98)/100 - 0.5) / (98.5 + AI) x 360/31`.
    /// The old metric assumed a flat 0.5% SMM and divided by the clean price.
    #[test]
    fn implied_financing_uses_pool_prepayment_and_dirty_front_price() {
        let mut roll = DollarRoll::example().expect("roll");
        roll.front_settlement_date = Some(date!(2026 - 03 - 11));
        roll.back_settlement_date = Some(date!(2026 - 04 - 11));
        let pool = create_assumed_pool(&roll.front_leg().expect("leg")).expect("pool");
        let i = pool.wac / 12.0;
        let paydown = 100.0 * i / ((1.0 + i).powi(360) - 1.0);
        let coupon = 0.04 * (100.0 * 20.0 + (100.0 - paydown) * 10.0) / 360.0;
        let accrued = 100.0 * 0.04 * 10.0 / 360.0;
        let expected =
            (coupon + paydown * (100.0 - 98.0) / 100.0 - 0.5) / (98.5 + accrued) * 360.0 / 31.0;

        let carry = implied_financing_rate(&roll).expect("carry");
        assert!(
            (carry.principal_paydown - paydown).abs() < 1e-10,
            "paydown {} versus {paydown}",
            carry.principal_paydown
        );
        assert!(
            (carry.implied_rate - expected).abs() < 1e-12,
            "implied {} versus {expected}",
            carry.implied_rate
        );
        let clean_basis = (coupon + paydown * 0.02 - 0.5) / 98.5 * 360.0 / 31.0;
        assert!((carry.implied_rate - clean_basis).abs() > 1e-6);
    }

    #[test]
    fn carry_counts_one_paydown_and_coupon_on_30_360() {
        let mut roll = DollarRoll::example().expect("roll");
        roll.front_settlement_date = Some(date!(2026 - 03 - 11));
        roll.back_settlement_date = Some(date!(2026 - 04 - 11));
        let carry = implied_financing_rate(&roll).expect("carry");
        let pool = create_assumed_pool(&roll.front_leg().expect("leg")).expect("pool");
        let cf = generate_cashflows(&pool, date!(2026 - 03 - 11), Some(1)).expect("flows");
        let principal =
            (cf[0].scheduled_principal + cf[0].prepayment) / pool.current_face.amount() * 100.0;
        assert!(
            (carry.principal_paydown - principal).abs() < 1e-10,
            "paydown {} versus {principal}",
            carry.principal_paydown
        );
        // Opening face accrues March 11-April 1, remaining face April 1-11.
        let coupon = roll.coupon * (100.0 * 20.0 + (100.0 - principal) * 10.0) / 360.0;
        assert!(
            (carry.coupon_income - coupon).abs() < 1e-10,
            "coupon {} versus {coupon}",
            carry.coupon_income
        );
    }
}
