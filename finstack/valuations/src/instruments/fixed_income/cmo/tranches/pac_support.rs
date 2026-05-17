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
    /// The PAC schedule is the minimum principal **the collateral pool**
    /// throws off at each period across the collar range. For each period,
    /// we project total principal (scheduled amortization + prepayment) off
    /// the *collateral* balance at both the lower and upper PSA speeds and
    /// take the minimum. The resulting collateral-derived stream is then
    /// carved to the PAC tranche: cumulative scheduled principal is capped
    /// at the PAC balance (the PAC cannot receive more principal than its
    /// own face), with any excess flowing to the support tranche.
    ///
    /// Projecting off the (smaller) PAC balance — as a prior implementation
    /// did — understates absolute scheduled principal and shifts the
    /// effective collar, so the collateral balance is the correct basis.
    ///
    /// # Arguments
    ///
    /// * `collateral_balance` - Current balance of the underlying collateral pool
    /// * `pac_balance` - Current balance of the PAC tranche being carved
    /// * `wam` - Weighted average maturity in months
    /// * `wac` - Weighted average coupon (annual)
    /// * `collar` - PAC collar (lower/upper PSA bounds)
    ///
    /// Reference: Fabozzi "Handbook of Mortgage-Backed Securities" Ch. 8
    pub fn generate(
        collateral_balance: f64,
        pac_balance: f64,
        wam: u32,
        wac: f64,
        collar: PacCollar,
    ) -> Self {
        // Project collateral principal at lower PSA
        let lower_principals =
            project_principal_stream(collateral_balance, wam, wac, collar.lower_psa);
        // Project collateral principal at upper PSA
        let upper_principals =
            project_principal_stream(collateral_balance, wam, wac, collar.upper_psa);

        // Collateral-derived PAC band = minimum principal at each period.
        let collateral_schedule = lower_principals
            .iter()
            .zip(upper_principals.iter())
            .map(|(lo, hi)| lo.min(*hi));

        // Carve to the PAC tranche: cap cumulative scheduled principal at the
        // PAC balance so the PAC never receives more than its own face.
        let mut cumulative = 0.0;
        let schedule: Vec<f64> = collateral_schedule
            .map(|principal| {
                let room = (pac_balance - cumulative).max(0.0);
                let carved = principal.min(room);
                cumulative += carved;
                carved
            })
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
        // Collateral pool of 100k, PAC tranche of 60k carved from it.
        let schedule =
            PacSchedule::generate(100_000.0, 60_000.0, 360, 0.045, PacCollar::standard());

        assert!(!schedule.scheduled_payments.is_empty());
        assert!(schedule.total_scheduled() > 0.0);
        // Carved PAC schedule cannot exceed the PAC balance.
        assert!(schedule.total_scheduled() <= 60_000.0 + 1e-6);
    }

    #[test]
    fn test_within_collar() {
        let schedule =
            PacSchedule::generate(100_000.0, 100_000.0, 360, 0.045, PacCollar::standard());

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
    fn test_pac_schedule_generated_off_collateral_not_pac_balance() {
        // A PAC tranche smaller than the collateral pool.
        let collateral_balance = 100_000.0;
        let pac_balance = 40_000.0;
        let wam = 360;
        let wac = 0.05;
        let collar = PacCollar::standard();
        let lower_psa = collar.lower_psa;
        let upper_psa = collar.upper_psa;

        let schedule = PacSchedule::generate(collateral_balance, pac_balance, wam, wac, collar);

        // The carved schedule's early-period principal must equal the
        // collateral-derived minimum-principal stream (before the PAC
        // balance cap binds), NOT a PAC-balance-derived stream.
        let lo = project_principal_stream(collateral_balance, wam, wac, lower_psa);
        let hi = project_principal_stream(collateral_balance, wam, wac, upper_psa);
        let collateral_min: Vec<f64> = lo.iter().zip(hi.iter()).map(|(l, h)| l.min(*h)).collect();

        // The (incorrect) PAC-balance-derived stream, for contrast.
        let pac_lo = project_principal_stream(pac_balance, wam, wac, lower_psa);
        let pac_hi = project_principal_stream(pac_balance, wam, wac, upper_psa);
        let pac_balance_min: Vec<f64> = pac_lo
            .iter()
            .zip(pac_hi.iter())
            .map(|(l, h)| l.min(*h))
            .collect();

        // Month 1: cap (40k) far exceeds first-period principal, so the carve
        // is a no-op. Schedule must equal the collateral-derived value and be
        // strictly larger than the PAC-balance-derived value.
        assert!(
            (schedule.scheduled_payments[0] - collateral_min[0]).abs() < 1e-9,
            "period 0: expected collateral-derived {}, got {}",
            collateral_min[0],
            schedule.scheduled_payments[0]
        );
        assert!(
            schedule.scheduled_payments[0] > pac_balance_min[0] * 1.5,
            "collateral-derived principal ({}) should dwarf PAC-balance-derived ({})",
            schedule.scheduled_payments[0],
            pac_balance_min[0]
        );

        // Carved cumulative principal never exceeds the PAC balance.
        assert!(schedule.total_scheduled() <= pac_balance + 1e-6);
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
                assert!(
                    got.is_finite(),
                    "SMM not finite at psa={psa}, month={month}"
                );
            }
        }
    }
}
