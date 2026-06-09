//! Dollar roll carry and implied financing calculations.
//!
//! The dollar roll drop implies a financing rate that can be compared
//! to repo rates to assess roll "specialness".
//!
//! Carry inputs (coupon income, principal paydown) are derived from the
//! MBS cashflow engine rather than stylized amortization, ensuring
//! consistency with the TBA pricer.

use super::DollarRoll;
use crate::instruments::fixed_income::mbs_passthrough::pricer::generate_cashflows;
use crate::instruments::fixed_income::tba::pricer::create_assumed_pool;
use finstack_core::Result;

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
/// dates) are computed from the MBS cashflow engine using the same
/// assumed pool that the TBA pricer uses.
///
/// # Formula
///
/// The roll seller (drops the bond, invests proceeds) forgoes the coupon
/// income and the paydown's pull-to-par gain over the roll, but buys back
/// cheaper by the drop. Break-even financing therefore satisfies:
///
/// ```text
/// implied_rate = (coupon_income + paydown × (100 − back_price)/100 − drop)
///                / front_price × (360 / days)
/// ```
///
/// A larger drop *lowers* the implied financing rate (the roll is special /
/// cheap to finance). Principal paid down at par is not a full-par cost: the
/// seller only forgoes the pull-to-par component `(100 − back_price)` per 100
/// of paydown.
///
/// # Arguments
///
/// * `roll` - Dollar roll instrument
/// * `prepay_rate` - Expected monthly prepayment rate (SMM). When set to
///   `0.0`, only scheduled amortization is included.
pub fn implied_financing_rate(roll: &DollarRoll, prepay_rate: f64) -> Result<CarryResult> {
    let days = roll.settlement_days()?;
    let drop = roll.drop();

    let front_leg = roll.front_leg()?;
    let front_settle = roll.front_settle_date()?;
    let back_settle = roll.back_settle_date()?;

    let pool = create_assumed_pool(&front_leg, front_settle)?;

    // Principal paydown: use the model's projection for the roll period.
    let max_months = ((days as f64 / 28.0).ceil() as u32).max(2) + 1;
    let cashflows = generate_cashflows(&pool, front_settle, Some(max_months))?;

    let original_face = pool.current_face.amount();
    let scale = if original_face.abs() > 1e-12 {
        100.0 / original_face
    } else {
        0.0
    };

    // Sum accrual-period principal between the two settlement dates
    let mut principal_paydown: f64 = cashflows
        .iter()
        .filter(|cf| cf.period_end > front_settle && cf.period_start < back_settle)
        .map(|cf| cf.scheduled_principal + cf.prepayment)
        .sum::<f64>()
        * scale;

    // Layer in user-supplied SMM if it exceeds the model's prepayment
    if prepay_rate > 0.0 {
        let model_smm_paydown: f64 = cashflows
            .iter()
            .filter(|cf| cf.period_end > front_settle && cf.period_start < back_settle)
            .map(|cf| cf.prepayment)
            .sum::<f64>()
            * scale;
        let user_smm_paydown = 100.0 * prepay_rate;
        if user_smm_paydown > model_smm_paydown {
            principal_paydown += user_smm_paydown - model_smm_paydown;
        }
    }

    // Coupon income accrued between the two settlement dates (per $100).
    // Dollar-roll carry uses accrued income, not payment-date cashflows,
    // because the payment delay for agency MBS (55–75 days) typically
    // pushes the first payment past the back settlement date.
    //
    // Interest accrues on the *declining* MBS balance: as the pool amortizes
    // and prepays over the roll, the balance earning the coupon shrinks.
    // Accruing the coupon on a constant 100 face overstates income —
    // materially for fast pools / long rolls. We accrue on the time-weighted
    // average balance, approximated by the mean of the front-settle balance
    // (100 per $100) and the back-settle balance (100 − principal_paydown).
    // `principal_paydown` here is the *total* roll-period paydown, including
    // any user-supplied SMM layered in above, so the declining balance is
    // consistent with the realized paydown. The accrual horizon uses the
    // actual roll days (ACT/360).
    let avg_balance = (100.0 + (100.0 - principal_paydown).max(0.0)) / 2.0;
    let coupon_income = roll.coupon * (days as f64 / 360.0) * avg_balance;

    // Net financing benefit forgone by the roll seller: coupon income plus the
    // paydown's pull-to-par gain, less the drop captured by buying back cheaper.
    let net_benefit = coupon_income + principal_paydown * (100.0 - roll.back_price) / 100.0 - drop;

    let price = roll.front_price;
    let implied_rate = if days > 0 {
        (net_benefit / price) * (360.0 / days as f64)
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
/// # Returns
///
/// Roll specialness in basis points (positive = roll is special, i.e.
/// rolling is cheaper than repo financing).
pub fn roll_specialness(roll: &DollarRoll, prepay_rate: f64, repo_rate: f64) -> Result<f64> {
    let carry = implied_financing_rate(roll, prepay_rate)?;
    let specialness = repo_rate - carry.implied_rate;
    Ok(specialness * 10_000.0)
}

/// Calculate break-even drop given a target financing rate.
///
/// Inverts the implied-rate formula for the drop:
///
/// ```text
/// drop = coupon_income + paydown × (100 − back_price)/100
///        − target_rate × front_price × days/360
/// ```
///
/// # Returns
///
/// Break-even drop (in price points)
pub fn break_even_drop(
    target_rate: f64,
    front_price: f64,
    back_price: f64,
    coupon_income: f64,
    principal_paydown: f64,
    days: i64,
) -> f64 {
    let required_net = target_rate * front_price * (days as f64 / 360.0);
    coupon_income + principal_paydown * (100.0 - back_price) / 100.0 - required_net
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implied_financing_rate() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let prepay_rate = 0.005;

        let result = implied_financing_rate(&roll, prepay_rate).expect("should calculate");

        assert!(result.implied_rate > -0.20);
        assert!(result.implied_rate < 0.20);
        assert!((result.drop - roll.drop()).abs() < 1e-10);
        assert!(result.coupon_income > 0.0, "should have coupon income");
        assert!(
            result.principal_paydown >= 0.0,
            "paydown should be non-negative"
        );
    }

    #[test]
    fn test_implied_financing_zero_prepay() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");

        let result = implied_financing_rate(&roll, 0.0).expect("should calculate");
        assert!(result.implied_rate > -0.20);
        assert!(result.implied_rate < 0.20);
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
        let result = implied_financing_rate(&roll, 0.005).expect("ok");

        // There must be some paydown for the test to be meaningful.
        assert!(
            result.principal_paydown > 0.0,
            "expected positive principal paydown over the roll"
        );

        // Flat-100-face accrual (the pre-fix formula).
        let days = result.settlement_days;
        let flat_100_income = roll.coupon * (days as f64 / 360.0) * 100.0;

        // Declining-balance accrual must be strictly smaller.
        assert!(
            result.coupon_income < flat_100_income,
            "coupon income {} should be below flat-100 accrual {flat_100_income} \
             once the balance declines",
            result.coupon_income
        );
        // And it must equal the mean-balance accrual.
        let avg_balance = (100.0 + (100.0 - result.principal_paydown).max(0.0)) / 2.0;
        let expected = roll.coupon * (days as f64 / 360.0) * avg_balance;
        assert!(
            (result.coupon_income - expected).abs() < 1e-9,
            "coupon income {} should equal mean-balance accrual {expected}",
            result.coupon_income
        );
    }

    #[test]
    fn test_roll_specialness() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let prepay_rate = 0.005;
        let repo_rate = 0.05;

        let specialness =
            roll_specialness(&roll, prepay_rate, repo_rate).expect("should calculate");

        // Definition: specialness = (repo − implied) in bp.
        let carry = implied_financing_rate(&roll, prepay_rate).expect("carry");
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
        let result = implied_financing_rate(&roll, 0.005).expect("ok");

        let be = break_even_drop(
            result.implied_rate,
            roll.front_price,
            roll.back_price,
            result.coupon_income,
            result.principal_paydown,
            result.settlement_days,
        );
        assert!(
            (be - roll.drop()).abs() < 0.01,
            "break-even at implied rate should ≈ actual drop, got {be} vs {}",
            roll.drop()
        );
    }

    /// Blocker B3 regression: a larger drop must *lower* the implied financing
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

        let prepay = 0.005;
        let r_base = implied_financing_rate(&base, prepay).expect("base carry");
        let r_wide = implied_financing_rate(&wide, prepay).expect("wide carry");
        assert!(
            r_wide.implied_rate < r_base.implied_rate,
            "larger drop must lower implied financing rate: base={}, wide={}",
            r_base.implied_rate,
            r_wide.implied_rate
        );

        let repo = 0.05;
        let s_base = roll_specialness(&base, prepay, repo).expect("base specialness");
        let s_wide = roll_specialness(&wide, prepay, repo).expect("wide specialness");
        assert!(
            s_wide > s_base,
            "larger drop must raise specialness: base={s_base}bp, wide={s_wide}bp"
        );
    }
}
