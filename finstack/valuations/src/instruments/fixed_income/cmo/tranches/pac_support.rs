//! PAC/Support tranche logic.
//!
//! PAC (Planned Amortization Class) tranches receive principal according
//! to a pre-determined schedule as long as prepayments stay within the
//! PAC collar. Support tranches absorb prepayment variability to protect
//! the PAC.

use crate::instruments::fixed_income::cmo::types::PacCollar;
use crate::instruments::fixed_income::structured_credit::{cpr_to_smm, psa_to_cpr};

/// PAC amortization schedule.
#[derive(Debug, Clone)]
pub struct PacSchedule {
    /// Monthly scheduled principal payments
    pub scheduled_payments: Vec<f64>,
    /// PAC collar
    pub collar: PacCollar,
}

impl PacSchedule {
    /// Generate PAC schedule from collateral characteristics.
    ///
    /// The PAC schedule is the minimum principal at each period
    /// across the collar range. For each period, we project total
    /// principal (scheduled amortization + prepayment) at both the
    /// lower and upper PSA speeds and take the minimum.
    ///
    /// Reference: Fabozzi "Handbook of Mortgage-Backed Securities" Ch. 8
    pub fn generate(pac_balance: f64, wam: u32, wac: f64, collar: PacCollar) -> Self {
        // Project principal at lower PSA
        let lower_principals = project_principal_stream(pac_balance, wam, wac, collar.lower_psa);
        // Project principal at upper PSA
        let upper_principals = project_principal_stream(pac_balance, wam, wac, collar.upper_psa);

        // PAC schedule = minimum principal at each period
        let schedule: Vec<f64> = lower_principals
            .iter()
            .zip(upper_principals.iter())
            .map(|(lo, hi)| lo.min(*hi))
            .collect();

        Self {
            scheduled_payments: schedule,
            collar,
        }
    }

    /// Check if current prepayment is within collar.
    pub fn is_within_collar(&self, actual_psa: f64) -> bool {
        actual_psa >= self.collar.lower_psa && actual_psa <= self.collar.upper_psa
    }

    /// Get scheduled payment for a period.
    pub fn scheduled_at(&self, period: usize) -> f64 {
        self.scheduled_payments.get(period).cloned().unwrap_or(0.0)
    }

    /// Total scheduled principal.
    pub fn total_scheduled(&self) -> f64 {
        self.scheduled_payments.iter().sum()
    }
}

/// Project total principal (scheduled + prepaid) at a given PSA speed.
///
/// Uses standard level-pay mortgage math:
/// - Monthly payment = P * r * (1+r)^n / ((1+r)^n - 1)
/// - Scheduled principal = Monthly payment - Interest
/// - Prepayment = (Balance - Scheduled principal) * SMM
fn project_principal_stream(initial_balance: f64, wam: u32, wac: f64, psa_speed: f64) -> Vec<f64> {
    let monthly_rate = wac / 12.0;
    let mut remaining = initial_balance;
    let mut principals = Vec::with_capacity(wam as usize);

    for month in 1..=wam {
        if remaining <= 1e-10 {
            principals.push(0.0);
            continue;
        }

        let remaining_months = wam.saturating_sub(month - 1);

        // Scheduled principal from level-pay amortization
        let scheduled_principal = if monthly_rate > 1e-12 && remaining_months > 0 {
            let factor = (1.0 + monthly_rate).powi(remaining_months as i32);
            let monthly_payment = remaining * monthly_rate * factor / (factor - 1.0);
            let interest = remaining * monthly_rate;
            (monthly_payment - interest).max(0.0)
        } else if remaining_months > 0 {
            // Zero rate: simple linear amortization
            remaining / remaining_months as f64
        } else {
            remaining
        };

        let scheduled_principal = scheduled_principal.min(remaining);

        // Prepayment on post-scheduled balance
        let smm = psa_to_smm(psa_speed, month);
        let balance_after_scheduled = remaining - scheduled_principal;
        let prepayment = balance_after_scheduled * smm;

        let total_principal = scheduled_principal + prepayment;
        principals.push(total_principal);

        remaining -= total_principal;
    }

    principals
}

/// Convert PSA speed to SMM for a given month.
///
/// Delegates to the registry-backed canonical PSA curve
/// (`utils::rates::psa_to_cpr`) and the canonical CPR→SMM conversion,
/// keeping PAC/Support projection consistent with the rest of the workspace.
#[inline]
fn psa_to_smm(psa_speed: f64, month: u32) -> f64 {
    cpr_to_smm(psa_to_cpr(psa_speed, month))
}

/// Allocate principal between PAC and support tranches.
///
/// # Arguments
///
/// * `available_principal` - Total principal available
/// * `pac_balance` - Current PAC balance
/// * `support_balance` - Current support balance
/// * `pac_scheduled` - PAC scheduled amount for this period
/// * `actual_psa` - Actual prepayment speed (PSA)
/// * `collar` - PAC collar
///
/// # Returns
///
/// (pac_allocation, support_allocation)
pub fn allocate_pac_support(
    available_principal: f64,
    pac_balance: f64,
    support_balance: f64,
    pac_scheduled: f64,
    actual_psa: f64,
    collar: &PacCollar,
) -> (f64, f64) {
    if available_principal <= 0.0 {
        return (0.0, 0.0);
    }

    let is_within_collar = actual_psa >= collar.lower_psa && actual_psa <= collar.upper_psa;

    if is_within_collar {
        // PAC gets scheduled, support gets excess
        let pac_alloc = pac_scheduled.min(pac_balance).min(available_principal);
        let support_alloc = (available_principal - pac_alloc).min(support_balance);
        (pac_alloc, support_alloc)
    } else if actual_psa < collar.lower_psa {
        // Slow prepay: PAC may not get full schedule, support depletes first
        // Support should absorb shortfall first
        let total_needed = pac_scheduled.min(pac_balance);
        if available_principal >= total_needed {
            (total_needed, available_principal - total_needed)
        } else {
            // Not enough for PAC schedule
            (available_principal, 0.0)
        }
    } else {
        // Fast prepay (above upper collar): PAC gets scheduled first, support absorbs excess
        let pac_alloc = pac_scheduled.min(pac_balance).min(available_principal);
        let remaining = available_principal - pac_alloc;
        let support_alloc = remaining.min(support_balance);
        (pac_alloc, support_alloc)
    }
}

/// Check if PAC collar is "broken" (support depleted).
pub fn is_collar_broken(support_balance: f64) -> bool {
    support_balance <= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pac_schedule_generation() {
        let schedule = PacSchedule::generate(100_000.0, 360, 0.045, PacCollar::standard());

        assert!(!schedule.scheduled_payments.is_empty());
        assert!(schedule.total_scheduled() > 0.0);
        assert!(schedule.total_scheduled() <= 100_000.0);
    }

    #[test]
    fn test_within_collar() {
        let schedule = PacSchedule::generate(100_000.0, 360, 0.045, PacCollar::standard());

        // 100% PSA is within 100-300 collar
        assert!(schedule.is_within_collar(1.0));

        // 200% PSA is within collar
        assert!(schedule.is_within_collar(2.0));

        // 50% PSA is below collar
        assert!(!schedule.is_within_collar(0.5));

        // 400% PSA is above collar
        assert!(!schedule.is_within_collar(4.0));
    }

    #[test]
    fn test_pac_support_allocation_within_collar() {
        let collar = PacCollar::standard();

        // Within collar: PAC gets schedule, support gets excess
        let (pac, support) = allocate_pac_support(
            10_000.0, // available
            50_000.0, // pac balance
            50_000.0, // support balance
            5_000.0,  // pac scheduled
            2.0,      // actual PSA (within collar)
            &collar,
        );

        assert!((pac - 5_000.0).abs() < 1.0);
        assert!((support - 5_000.0).abs() < 1.0);
    }

    #[test]
    fn test_pac_support_allocation_fast_prepay() {
        let collar = PacCollar::standard();

        // Above collar: PAC gets scheduled first, support absorbs excess
        let (pac, support) =
            allocate_pac_support(10_000.0, 50_000.0, 20_000.0, 5_000.0, 4.0, &collar);

        // PAC should get scheduled amount first
        assert!((pac - 5_000.0).abs() < 1.0);
        // Support gets remainder
        assert!((support - 5_000.0).abs() < 1.0);
    }

    #[test]
    fn test_psa_to_smm() {
        // 100% PSA at month 30 should give ~0.5% SMM
        let smm = psa_to_smm(1.0, 30);
        assert!(smm > 0.004 && smm < 0.006);

        // 200% PSA should be about double
        let smm_200 = psa_to_smm(2.0, 30);
        assert!(smm_200 > smm * 1.5);
    }

    #[test]
    fn test_psa_to_smm_matches_canonical_helper() {
        // PAC/Support SMM conversion must match the registry-backed canonical
        // helper across a PSA grid, including high-speed clamping behavior.
        for &psa in &[0.0, 0.5, 1.0, 2.0, 3.0, 18.0] {
            for &month in &[1u32, 15, 30, 60, 360] {
                let canonical = cpr_to_smm(psa_to_cpr(psa, month));
                let got = psa_to_smm(psa, month);
                assert!(
                    (got - canonical).abs() < 1e-12,
                    "PSA→SMM drift at psa={psa}, month={month}: got {got}, canonical {canonical}"
                );
                assert!(got.is_finite(), "SMM not finite at psa={psa}, month={month}");
            }
        }
    }
}
