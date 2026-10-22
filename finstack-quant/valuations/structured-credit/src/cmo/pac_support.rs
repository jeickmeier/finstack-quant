//! PAC/Support tranche logic.
//!
//! PAC (Planned Amortization Class) tranches receive principal according
//! to a pre-determined schedule as long as prepayments stay within the
//! PAC collar. Support tranches absorb prepayment variability to protect
//! the PAC.

use crate::cmo::types::PacCollar;
use finstack_quant_cashflows::builder::{clamped_cpr_to_smm, psa_cpr};
use finstack_quant_valuations_fixed_income::mbs_passthrough::pricer::pool_month_step;

/// Dated contractual PAC principal target, in the tranche currency's major units.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PacPayment {
    /// Contractual payment date, including the collateral's payment delay.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: finstack_quant_core::dates::Date,
    /// Planned principal payment, finite and nonnegative.
    pub principal: f64,
}

/// Original contractual PAC schedule. Pricing never regenerates these targets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PacSchedule {
    /// Strictly increasing dated principal targets fixed at construction.
    pub payments: Vec<PacPayment>,
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
    /// * `payment_dates` - Strictly increasing actual monthly payment dates at
    ///   construction; the number of entries is the collateral's remaining WAM.
    ///   Dates must include contractual payment delay and calendar adjustment.
    /// * `wac` - Weighted average coupon (annual)
    /// * `seasoning_months` - Current collateral age (WALA) in months;
    ///   the PSA seasoning ramp is anchored at this age, not at age 0, so
    ///   seasoned collateral projects at the correct (post-ramp) speeds
    /// * `collar` - PAC collar (lower/upper PSA bounds)
    ///
    /// Reference: Fabozzi "Handbook of Mortgage-Backed Securities" Ch. 8
    pub fn generate(
        collateral_balance: f64,
        pac_balance: f64,
        payment_dates: &[finstack_quant_core::dates::Date],
        wac: f64,
        seasoning_months: u32,
        collar: PacCollar,
    ) -> finstack_quant_core::Result<Self> {
        let wam_months = u32::try_from(payment_dates.len()).map_err(|_| {
            finstack_quant_core::Error::Validation("PAC payment count exceeds supported WAM".into())
        })?;
        if payment_dates.is_empty()
            || payment_dates.windows(2).any(|dates| dates[0] >= dates[1])
            || !collateral_balance.is_finite()
            || collateral_balance <= 0.0
            || !pac_balance.is_finite()
            || pac_balance < 0.0
            || pac_balance > collateral_balance
            || !wac.is_finite()
            || wac < 0.0
        {
            return Err(finstack_quant_core::Error::Validation("PAC generation requires ordered dates, positive collateral, nonnegative WAC, and PAC face within collateral".into()));
        }
        // Project collateral principal at lower PSA
        let lower_principals = project_principal_stream(
            collateral_balance,
            wam_months,
            wac,
            seasoning_months,
            collar.lower_speed_multiplier,
        );
        // Project collateral principal at upper PSA
        let upper_principals = project_principal_stream(
            collateral_balance,
            wam_months,
            wac,
            seasoning_months,
            collar.upper_speed_multiplier,
        );

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

        let result = Self {
            payments: payment_dates
                .iter()
                .copied()
                .zip(schedule)
                .map(|(date, principal)| PacPayment { date, principal })
                .collect(),
            collar,
        };
        result.validate()?;
        Ok(result)
    }

    /// Return the original principal target on a contractual payment date.
    ///
    /// # Arguments
    ///
    /// * `date` - Actual payment date. Dates absent from the contract have a
    ///   zero target; no period index is restarted at valuation.
    pub fn get_payment(&self, date: finstack_quant_core::dates::Date) -> f64 {
        self.payments
            .binary_search_by_key(&date, |payment| payment.date)
            .map_or(0.0, |index| self.payments[index].principal)
    }

    /// Sum all original contractual principal targets.
    pub fn get_total(&self) -> f64 {
        self.payments.iter().map(|payment| payment.principal).sum()
    }

    /// Reject unordered dates, invalid principal targets or an invalid PSA collar.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.payments.is_empty()
            || self
                .payments
                .windows(2)
                .any(|pair| pair[0].date >= pair[1].date)
            || self
                .payments
                .iter()
                .any(|payment| !payment.principal.is_finite() || payment.principal < 0.0)
            || !self.collar.lower_speed_multiplier.is_finite()
            || !self.collar.upper_speed_multiplier.is_finite()
            || self.collar.lower_speed_multiplier < 0.0
            || self.collar.lower_speed_multiplier > self.collar.upper_speed_multiplier
        {
            return Err(finstack_quant_core::Error::Validation("PAC contract requires increasing dates, finite nonnegative targets, and an ordered nonnegative collar".into()));
        }
        Ok(())
    }
}

/// Project total principal (scheduled + prepaid) at a given PSA speed.
///
/// Uses the shared level-pay pool step
/// ([`pool_month_step`](finstack_quant_valuations_fixed_income::mbs_passthrough::pricer::pool_month_step)):
/// scheduled principal from the level-pay annuity, prepayment = SMM × the
/// post-scheduled balance.
///
/// `seasoning_months` anchors the PSA seasoning ramp: projection month `m` of a
/// pool aged `a` months uses the PSA CPR at loan age `a + m`, so seasoned
/// collateral does not restart the 30-month ramp at age 0.
fn project_principal_stream(
    initial_balance: f64,
    wam_months: u32,
    wac: f64,
    seasoning_months: u32,
    speed_multiplier: f64,
) -> Vec<f64> {
    let mut remaining = initial_balance;
    let mut principals = Vec::with_capacity(wam_months as usize);

    for month in 1..=wam_months {
        if remaining <= 1e-10 {
            principals.push(0.0);
            continue;
        }
        // Prepayment on post-scheduled balance, at the pool's actual loan age.
        let smm = psa_to_smm(speed_multiplier, seasoning_months.saturating_add(month));
        let step = pool_month_step(remaining, wam_months - (month - 1), wac, smm, 0.0, 0.0);
        principals.push(step.scheduled_principal + step.prepayment);
        remaining = step.ending_balance;
    }

    principals
}

/// Convert PSA speed to SMM for a given month.
///
/// Delegates to the registry-backed canonical PSA curve
/// (`finstack_quant_cashflows::builder::psa_cpr`) and the canonical CPR→SMM conversion,
/// keeping PAC/Support projection consistent with the rest of the workspace.
#[inline]
fn psa_to_smm(speed_multiplier: f64, month: u32) -> f64 {
    clamped_cpr_to_smm(psa_cpr(speed_multiplier, month))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{Date, DateExt};
    fn dates(count: u32) -> Vec<Date> {
        (0..count)
            .map(|month| {
                time::macros::date!(2024 - 02 - 25)
                    .add_months(month as i32)
                    .expect("date")
            })
            .collect()
    }

    #[test]
    fn test_pac_schedule_generation() {
        // Collateral pool of 100k, PAC tranche of 60k carved from it.
        let schedule = PacSchedule::generate(
            100_000.0,
            60_000.0,
            &dates(360),
            0.045,
            0,
            PacCollar::standard(),
        )
        .expect("PAC contract");

        assert!(!schedule.payments.is_empty());
        assert!(schedule.get_total() > 0.0);
        // Carved PAC schedule cannot exceed the PAC balance.
        assert!(schedule.get_total() <= 60_000.0 + 1e-6);
    }

    #[test]
    fn test_pac_schedule_generated_off_collateral_not_pac_balance() {
        // A PAC tranche smaller than the collateral pool.
        let collateral_balance = 100_000.0;
        let pac_balance = 40_000.0;
        let wam_months = 360;
        let wac = 0.05;
        let collar = PacCollar::standard();
        let lower_speed_multiplier = collar.lower_speed_multiplier;
        let upper_speed_multiplier = collar.upper_speed_multiplier;

        let schedule = PacSchedule::generate(
            collateral_balance,
            pac_balance,
            &dates(wam_months),
            wac,
            0,
            collar,
        )
        .expect("PAC contract");

        // The carved schedule's early-period principal must equal the
        // collateral-derived minimum-principal stream (before the PAC
        // balance cap binds), NOT a PAC-balance-derived stream.
        let lo = project_principal_stream(
            collateral_balance,
            wam_months,
            wac,
            0,
            lower_speed_multiplier,
        );
        let hi = project_principal_stream(
            collateral_balance,
            wam_months,
            wac,
            0,
            upper_speed_multiplier,
        );
        let collateral_min: Vec<f64> = lo.iter().zip(hi.iter()).map(|(l, h)| l.min(*h)).collect();

        // The (incorrect) PAC-balance-derived stream, for contrast.
        let pac_lo =
            project_principal_stream(pac_balance, wam_months, wac, 0, lower_speed_multiplier);
        let pac_hi =
            project_principal_stream(pac_balance, wam_months, wac, 0, upper_speed_multiplier);
        let pac_balance_min: Vec<f64> = pac_lo
            .iter()
            .zip(pac_hi.iter())
            .map(|(l, h)| l.min(*h))
            .collect();

        // Month 1: cap (40k) far exceeds first-period principal, so the carve
        // is a no-op. Schedule must equal the collateral-derived value and be
        // strictly larger than the PAC-balance-derived value.
        assert!(
            (schedule.payments[0].principal - collateral_min[0]).abs() < 1e-9,
            "period 0: expected collateral-derived {}, got {}",
            collateral_min[0],
            schedule.payments[0].principal
        );
        assert!(
            schedule.payments[0].principal > pac_balance_min[0] * 1.5,
            "collateral-derived principal ({}) should dwarf PAC-balance-derived ({})",
            schedule.payments[0].principal,
            pac_balance_min[0]
        );

        // Carved cumulative principal never exceeds the PAC balance.
        assert!(schedule.get_total() <= pac_balance + 1e-6);
    }

    #[test]
    fn test_pac_schedule_anchors_ramp_at_collateral_age() {
        // A pool seasoned past the 30-month PSA ramp prepays at terminal CPR
        // from projection month 1; a fresh pool is still ramping. The seasoned
        // schedule must therefore start with strictly more principal.
        let balance = 100_000.0;
        let wam_months = 330;
        let wac = 0.05;

        let fresh = PacSchedule::generate(
            balance,
            balance,
            &dates(wam_months),
            wac,
            0,
            PacCollar::standard(),
        )
        .expect("PAC contract");
        let seasoned = PacSchedule::generate(
            balance,
            balance,
            &dates(wam_months),
            wac,
            30,
            PacCollar::standard(),
        )
        .expect("PAC contract");

        assert!(
            seasoned.payments[0].principal > fresh.payments[0].principal,
            "seasoned month-1 principal ({}) must exceed fresh ({}) — the PSA \
             ramp must anchor at the pool's current age",
            seasoned.payments[0].principal,
            fresh.payments[0].principal
        );

        // At age ≥ 30 the ramp is flat, so adding more age changes nothing.
        let very_seasoned = PacSchedule::generate(
            balance,
            balance,
            &dates(wam_months),
            wac,
            120,
            PacCollar::standard(),
        )
        .expect("PAC contract");
        assert!(
            (very_seasoned.payments[0].principal - seasoned.payments[0].principal).abs() < 1e-9,
            "post-ramp ages must produce identical schedules"
        );
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
                let canonical = clamped_cpr_to_smm(psa_cpr(psa, month));
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
