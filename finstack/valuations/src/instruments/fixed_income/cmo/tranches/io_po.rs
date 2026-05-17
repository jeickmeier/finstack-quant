//! IO/PO strip logic.
//!
//! Interest-Only (IO) and Principal-Only (PO) strips separate the
//! interest and principal components of MBS cashflows.

use crate::instruments::fixed_income::cmo::types::CmoTranche;
use crate::instruments::fixed_income::structured_credit::{cpr_to_smm, psa_to_cpr};

/// Single monthly mortality (SMM) at a PSA speed for a given month.
///
/// Delegates to the registry-backed canonical PSA curve
/// (`utils::rates::psa_to_cpr`) and the canonical CPR→SMM conversion,
/// so IO/PO valuations stay consistent with the rest of the workspace.
#[inline]
fn psa_to_smm(psa_speed: f64, month: u32) -> f64 {
    cpr_to_smm(psa_to_cpr(psa_speed, month))
}

/// IO strip characteristics.
#[derive(Debug, Clone)]
pub struct IoStripCharacteristics {
    /// Notional amount (used for interest calculation)
    pub notional: f64,
    /// Coupon rate
    pub coupon: f64,
    /// Expected average life
    pub expected_avg_life: Option<f64>,
}

impl IoStripCharacteristics {
    /// Create from tranche.
    pub fn from_tranche(tranche: &CmoTranche) -> Self {
        Self {
            notional: tranche.original_face.amount(),
            coupon: tranche.coupon,
            expected_avg_life: None,
        }
    }

    /// Calculate monthly interest payment at given factor.
    pub fn interest_at_factor(&self, factor: f64) -> f64 {
        self.notional * factor * self.coupon / 12.0
    }

    /// IO value sensitivity to prepayment.
    ///
    /// IOs have negative convexity to prepayment:
    /// - Faster prepay -> lower value (less interest received)
    /// - Slower prepay -> higher value (more interest received)
    pub fn prepay_sensitivity(&self, base_value: f64, psa_change: f64) -> f64 {
        // Simplified: ~5% value change per 100 PSA change
        -base_value * psa_change * 0.05
    }
}

/// PO strip characteristics.
#[derive(Debug, Clone)]
pub struct PoStripCharacteristics {
    /// Face amount
    pub face: f64,
    /// Expected average life
    pub expected_avg_life: Option<f64>,
}

impl PoStripCharacteristics {
    /// Create from tranche.
    pub fn from_tranche(tranche: &CmoTranche) -> Self {
        Self {
            face: tranche.original_face.amount(),
            expected_avg_life: None,
        }
    }

    /// Calculate principal payment.
    ///
    /// PO receives all principal (scheduled + prepay).
    pub fn principal_at_smm(&self, current_balance: f64, scheduled: f64, smm: f64) -> f64 {
        let prepay = (current_balance - scheduled) * smm;
        scheduled + prepay
    }

    /// PO value sensitivity to prepayment.
    ///
    /// POs have positive convexity to prepayment:
    /// - Faster prepay -> higher value (principal returned sooner)
    /// - Slower prepay -> lower value (principal returned later)
    pub fn prepay_sensitivity(&self, base_value: f64, psa_change: f64) -> f64 {
        // Simplified: ~3% value change per 100 PSA change
        base_value * psa_change * 0.03
    }
}

/// Calculate IO/PO split from passthrough.
///
/// Given a passthrough coupon, splits into IO (interest) and PO (principal).
///
/// # Arguments
///
/// * `passthrough_coupon` - Pass-through rate of underlying MBS
/// * `face_amount` - Face amount to split
///
/// # Returns
///
/// (io_notional, po_face) tuple
pub fn split_io_po(_passthrough_coupon: f64, face_amount: f64) -> (f64, f64) {
    // IO notional is same as face (interest calculated on this)
    // PO face is same as original face (receives all principal)
    (face_amount, face_amount)
}

/// Calculate theoretical IO value.
///
/// IO value = PV of expected interest payments.
///
/// # Arguments
///
/// * `notional` - IO notional amount
/// * `coupon` - IO coupon rate
/// * `wam` - Remaining weighted average maturity
/// * `discount_rate` - Discount rate (annual)
/// * `psa` - Expected prepayment speed
///
/// # Returns
///
/// Theoretical IO price (as % of notional)
pub fn theoretical_io_value(
    notional: f64,
    coupon: f64,
    wam: u32,
    discount_rate: f64,
    psa: f64,
) -> f64 {
    let monthly_rate = discount_rate / 12.0;
    let monthly_coupon = coupon / 12.0;

    let mut value = 0.0;
    let mut remaining_notional = notional;

    for month in 1..=wam {
        if remaining_notional <= 0.0 {
            break;
        }

        // SMM from PSA (registry-backed canonical curve)
        let smm = psa_to_smm(psa, month);

        // Interest payment this month
        let interest = remaining_notional * monthly_coupon;

        // Discount factor
        let df = 1.0 / (1.0 + monthly_rate).powi(month as i32);

        value += interest * df;

        // Update remaining notional
        remaining_notional *= 1.0 - smm;
    }

    value / notional * 100.0 // Return as percentage of notional
}

/// Calculate theoretical PO value.
///
/// PO value = PV of expected principal payments.
///
/// # Arguments
///
/// * `face` - PO face amount
/// * `wam` - Remaining weighted average maturity
/// * `discount_rate` - Discount rate (annual)
/// * `psa` - Expected prepayment speed
///
/// # Returns
///
/// Theoretical PO price (as % of face)
pub fn theoretical_po_value(face: f64, wam: u32, discount_rate: f64, psa: f64) -> f64 {
    theoretical_po_value_with_wac(face, wam, discount_rate, psa, 0.0)
}

/// Theoretical PO strip value with explicit WAC for scheduled amortization.
///
/// PO strips receive both scheduled (level-pay) principal and prepaid principal.
/// Without WAC, only prepayment principal is captured (understating PO value).
///
/// # Arguments
///
/// * `face` - Face value
/// * `wam` - Weighted average maturity in months
/// * `discount_rate` - Discount rate (annual)
/// * `psa` - Expected prepayment speed
/// * `wac` - Weighted average coupon (annual mortgage rate)
pub fn theoretical_po_value_with_wac(
    face: f64,
    wam: u32,
    discount_rate: f64,
    psa: f64,
    wac: f64,
) -> f64 {
    let monthly_rate = discount_rate / 12.0;
    let monthly_mortgage_rate = wac / 12.0;

    let mut value = 0.0;
    let mut remaining = face;

    for month in 1..=wam {
        if remaining <= 0.0 {
            break;
        }

        // Scheduled amortization (level-pay formula)
        let remaining_months = wam - month + 1;
        let scheduled = if remaining_months <= 1 {
            remaining
        } else if monthly_mortgage_rate > 1e-12 {
            let factor = (1.0 + monthly_mortgage_rate).powi(remaining_months as i32);
            let payment = remaining * monthly_mortgage_rate * factor / (factor - 1.0);
            (payment - remaining * monthly_mortgage_rate)
                .max(0.0)
                .min(remaining)
        } else {
            remaining / remaining_months as f64
        };

        // SMM from PSA (registry-backed canonical curve), applied to the
        // post-scheduled balance per Fabozzi Ch. 4.
        let smm = psa_to_smm(psa, month);
        let prepayment = (remaining - scheduled).max(0.0) * smm;

        // PO receives all principal: scheduled + prepaid
        let principal = scheduled + prepayment;

        // Discount factor
        let df = 1.0 / (1.0 + monthly_rate).powi(month as i32);

        value += principal * df;

        remaining -= principal;
    }

    // Add terminal principal (if any remaining at maturity)
    if remaining > 0.0 {
        let df = 1.0 / (1.0 + monthly_rate).powi(wam as i32);
        value += remaining * df;
    }

    value / face * 100.0 // Return as percentage of face
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::cmo::types::CmoTranche;
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;

    #[test]
    fn test_io_characteristics() {
        let io = CmoTranche::io_strip("IO", Money::new(100_000.0, Currency::USD), 0.04);
        let chars = IoStripCharacteristics::from_tranche(&io);

        assert!((chars.notional - 100_000.0).abs() < 1.0);
        assert!((chars.coupon - 0.04).abs() < 1e-10);

        // Interest at 100% factor
        let interest = chars.interest_at_factor(1.0);
        // 100,000 × 0.04 / 12 = 333.33
        assert!((interest - 333.33).abs() < 1.0);
    }

    #[test]
    fn test_po_characteristics() {
        let po = CmoTranche::po_strip("PO", Money::new(100_000.0, Currency::USD));
        let chars = PoStripCharacteristics::from_tranche(&po);

        assert!((chars.face - 100_000.0).abs() < 1.0);

        // Principal at 1% SMM
        let principal = chars.principal_at_smm(100_000.0, 500.0, 0.01);
        // scheduled (500) + prepay (99,500 × 0.01 = 995)
        assert!((principal - 1495.0).abs() < 1.0);
    }

    #[test]
    fn test_io_prepay_sensitivity() {
        let io = CmoTranche::io_strip("IO", Money::new(100_000.0, Currency::USD), 0.04);
        let chars = IoStripCharacteristics::from_tranche(&io);

        let base_value = 10_000.0;

        // Faster prepay should reduce IO value
        let sensitivity = chars.prepay_sensitivity(base_value, 1.0);
        assert!(sensitivity < 0.0);
    }

    #[test]
    fn test_po_prepay_sensitivity() {
        let po = CmoTranche::po_strip("PO", Money::new(100_000.0, Currency::USD));
        let chars = PoStripCharacteristics::from_tranche(&po);

        let base_value = 90_000.0;

        // Faster prepay should increase PO value
        let sensitivity = chars.prepay_sensitivity(base_value, 1.0);
        assert!(sensitivity > 0.0);
    }

    #[test]
    fn test_theoretical_io_value() {
        let value = theoretical_io_value(100_000.0, 0.04, 360, 0.05, 1.0);

        // IO should be worth significantly less than notional
        assert!(value > 0.0);
        assert!(value < 50.0); // Typically 5-20% of notional
    }

    #[test]
    fn test_theoretical_po_value() {
        let value = theoretical_po_value(100_000.0, 360, 0.05, 1.0);

        // PO should be worth less than face (time value of money)
        assert!(value > 0.0);
        assert!(value < 100.0);
    }

    #[test]
    fn test_psa_to_smm_matches_canonical_helper() {
        // The IO/PO SMM conversion must match the registry-backed canonical
        // helper across a PSA grid, including ramp, terminal, and high-speed
        // points. The previous hard-coded copy diverged at high PSA where the
        // unclamped CPR exceeded 1.0 and produced NaN via `.powf`.
        for &psa in &[0.0, 0.5, 1.0, 1.5, 3.0, 18.0] {
            for &month in &[1u32, 15, 30, 60, 360] {
                let canonical = cpr_to_smm(psa_to_cpr(psa, month));
                let got = psa_to_smm(psa, month);
                assert!(
                    (got - canonical).abs() < 1e-12,
                    "PSA→SMM drift at psa={psa}, month={month}: got {got}, canonical {canonical}"
                );
                assert!(
                    got.is_finite(),
                    "SMM not finite at psa={psa}, month={month}"
                );
            }
        }
    }

    #[test]
    fn test_io_po_prepay_impact() {
        // Faster prepay should hurt IO, help PO
        let io_slow = theoretical_io_value(100_000.0, 0.04, 360, 0.05, 0.5);
        let io_fast = theoretical_io_value(100_000.0, 0.04, 360, 0.05, 2.0);
        assert!(io_slow > io_fast);

        let po_slow = theoretical_po_value(100_000.0, 360, 0.05, 0.5);
        let po_fast = theoretical_po_value(100_000.0, 360, 0.05, 2.0);
        assert!(po_fast > po_slow);
    }
}
