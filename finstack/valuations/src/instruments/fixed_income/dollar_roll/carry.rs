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
/// ```text
/// implied_rate = (drop + coupon_income - paydown) / price × (360 / days)
/// ```
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

    // Coupon income accrued between the two settlement dates (per $100).
    // Dollar-roll carry uses accrued income, not payment-date cashflows,
    // because the payment delay for agency MBS (55–75 days) typically
    // pushes the first payment past the back settlement date.
    // Accrue on the actual roll horizon (ACT/360) rather than rounding to a
    // whole number of months — a 28-day roll overstated income ~7% under the
    // previous `round(days/30)` whole-month convention.
    let coupon_income = roll.coupon * (days as f64 / 360.0) * 100.0;

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

    let net_benefit = drop + coupon_income - principal_paydown;

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
/// # Returns
///
/// Break-even drop (in price points)
pub fn break_even_drop(
    target_rate: f64,
    front_price: f64,
    coupon_income: f64,
    principal_paydown: f64,
    days: i64,
) -> f64 {
    let required_net = target_rate * front_price * (days as f64 / 360.0);
    required_net - coupon_income + principal_paydown
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

    #[test]
    fn test_roll_specialness() {
        let roll = DollarRoll::example().expect("DollarRoll example is valid");
        let prepay_rate = 0.005;
        let repo_rate = 0.05;

        let specialness =
            roll_specialness(&roll, prepay_rate, repo_rate).expect("should calculate");
        assert!(specialness > -500.0);
        assert!(specialness < 500.0);
    }

    #[test]
    fn test_break_even_drop() {
        let target_rate = 0.04;
        let front_price = 98.5;
        let coupon_income = 0.333;
        let principal_paydown = 0.5;
        let days = 30;

        let break_even = break_even_drop(
            target_rate,
            front_price,
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
}
