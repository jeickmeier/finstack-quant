//! Shared per-period fee accrual accumulator for deterministic and stochastic cashflow engines.

use super::super::types::RevolvingCredit;
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Balances a sub-interval's fees accrue on.
pub(super) struct FeeBalances {
    /// Drawn balance (usage-fee base).
    pub(super) drawn: Money,
    /// Undrawn availability net of letters of credit (commitment-fee base).
    pub(super) undrawn: Money,
    /// Commitment in force (facility-fee base).
    pub(super) commitment: Money,
    /// Letters of credit outstanding (LC and fronting fee base).
    pub(super) lc_outstanding: Money,
}

/// Per-period fee accrual shared by the deterministic and path engines:
/// accrued amounts and `rate × dt` sums for the commitment, usage, facility,
/// LC and fronting fees.
pub(super) struct FeeAccrual {
    commitment: Money,
    usage: Money,
    facility: Money,
    lc: Money,
    fronting: Money,
    weighted_commitment: f64,
    weighted_usage: f64,
    weighted_facility: f64,
    weighted_lc: f64,
    weighted_fronting: f64,
}

impl FeeAccrual {
    pub(super) fn new(ccy: finstack_quant_core::currency::Currency) -> Self {
        let zero = Money::from((0_i64, ccy));
        Self {
            commitment: zero,
            usage: zero,
            facility: zero,
            lc: zero,
            fronting: zero,
            weighted_commitment: 0.0,
            weighted_usage: 0.0,
            weighted_facility: 0.0,
            weighted_lc: 0.0,
            weighted_fronting: 0.0,
        }
    }

    /// Accrue every fee over a sub-interval of year fraction `dt` starting on
    /// `sub_start`, with tiers evaluated at `utilization`.
    pub(super) fn accrue(
        &mut self,
        facility: &RevolvingCredit,
        utilization: f64,
        sub_start: Date,
        balances: &FeeBalances,
        dt: f64,
    ) -> Result<()> {
        let commitment_bp = facility.fees.commitment_fee_bp_at(utilization, sub_start)?;
        if commitment_bp > 0.0 {
            self.commitment = self.commitment.checked_add(
                balances
                    .undrawn
                    .checked_mul_f64(commitment_bp * 1e-4 * dt)?,
            )?;
            self.weighted_commitment += commitment_bp * 1e-4 * dt;
        }
        let usage_bp = facility.fees.usage_fee_bp_at(utilization, sub_start)?;
        if usage_bp > 0.0 {
            self.usage = self
                .usage
                .checked_add(balances.drawn.checked_mul_f64(usage_bp * 1e-4 * dt)?)?;
            self.weighted_usage += usage_bp * 1e-4 * dt;
        }
        let facility_bp = facility.fees.facility_fee_bp_at(sub_start);
        if facility_bp > 0.0 {
            self.facility = self.facility.checked_add(
                balances
                    .commitment
                    .checked_mul_f64(facility_bp * 1e-4 * dt)?,
            )?;
            self.weighted_facility += facility_bp * 1e-4 * dt;
        }
        if balances.lc_outstanding.amount() > 0.0 {
            let lc_bp = facility.lc_fee_bp_at(sub_start);
            if lc_bp > 0.0 {
                self.lc = self
                    .lc
                    .checked_add(balances.lc_outstanding.checked_mul_f64(lc_bp * 1e-4 * dt)?)?;
                self.weighted_lc += lc_bp * 1e-4 * dt;
            }
            let fronting_bp = facility.fronting_fee_bp();
            if fronting_bp > 0.0 {
                self.fronting = self.fronting.checked_add(
                    balances
                        .lc_outstanding
                        .checked_mul_f64(fronting_bp * 1e-4 * dt)?,
                )?;
                self.weighted_fronting += fronting_bp * 1e-4 * dt;
            }
        }
        Ok(())
    }

    /// Append one non-zero flow per fee, paid on `payment_date` over `accrual`
    /// years, with its time-weighted average rate; errors when a row is invalid.
    pub(super) fn emit(
        self,
        flows: &mut Vec<CashFlow>,
        payment_date: Date,
        accrual: f64,
        rc: &RoundingContext,
    ) -> finstack_quant_core::Result<()> {
        let avg = |weighted: f64| (accrual > 0.0 && weighted > 0.0).then(|| weighted / accrual);
        for (amount, kind, rate) in [
            (
                self.commitment,
                CFKind::CommitmentFee,
                avg(self.weighted_commitment),
            ),
            (self.usage, CFKind::UsageFee, avg(self.weighted_usage)),
            (
                self.facility,
                CFKind::FacilityFee,
                avg(self.weighted_facility),
            ),
            (self.lc, CFKind::LcFee, avg(self.weighted_lc)),
            (
                self.fronting,
                CFKind::FrontingFee,
                avg(self.weighted_fronting),
            ),
        ] {
            if !rc.is_effectively_zero(amount.amount(), ZeroKind::Money(amount.currency())) {
                flows.push(
                    CashFlowParams::new(payment_date, None, amount, kind, accrual, rate).build()?,
                );
            }
        }
        Ok(())
    }
}
