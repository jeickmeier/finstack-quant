use super::MetricId;
use std::borrow::Cow;

#[allow(non_upper_case_globals)] // PascalCase names for metric ID constants
impl MetricId {
    // CDS Metrics

    /// CDS par spread under the instrument's premium-leg convention.
    ///
    /// The running spread that makes the CDS have zero PV under the current
    /// discount and survival curves.
    ///
    /// Units: decimal spread per annum unless a quoting layer converts it to bp
    /// for display.
    pub const ParSpread: Self = Self(Cow::Borrowed("par_spread"));

    /// Risky PV01 for CDS premium-leg valuation.
    ///
    /// Present value of one basis point of running premium paid over the risky
    /// premium leg, including default-contingent survival weighting.
    ///
    /// Units: currency per 1bp running spread.
    pub const RiskyPv01: Self = Self(Cow::Borrowed("risky_pv01"));

    /// Risky annuity (premium leg PV per 1bp)
    pub const RiskyAnnuity: Self = Self(Cow::Borrowed("risky_annuity"));

    /// Protection leg present value
    pub const ProtectionLegPv: Self = Self(Cow::Borrowed("protection_leg_pv"));

    /// Premium leg present value
    pub const PremiumLegPv: Self = Self(Cow::Borrowed("premium_leg_pv"));

    /// Jump-to-default amount.
    ///
    /// Immediate P&L impact of an instantaneous default event under the
    /// instrument's loss and settlement convention.
    ///
    /// Units: currency.
    pub const JumpToDefault: Self = Self(Cow::Borrowed("jump_to_default"));

    /// Clean default exposure.
    ///
    /// Signed LGD payout net of the current mark, excluding accrued premium on
    /// default. This matches dealer-screen "default exposure" style measures
    /// more closely than accrued-premium-adjusted jump-to-default.
    ///
    /// Units: currency.
    pub const DefaultExposure: Self = Self(Cow::Borrowed("default_exposure"));

    /// Expected loss under the current credit model.
    ///
    /// Expected discounted credit loss implied by default probabilities and
    /// recovery assumptions.
    ///
    /// Units: currency.
    pub const ExpectedLoss: Self = Self(Cow::Borrowed("expected_loss"));

    /// PV change per +1% (0.01 absolute) recovery-rate move.
    ///
    /// Units: currency.
    pub const Recovery01: Self = Self(Cow::Borrowed("recovery01"));

    // Structured Credit Metrics

    /// Weighted Average Life (WAL), the expected principal repayment life.
    ///
    /// Units: years.
    pub const WAL: Self = Self(Cow::Borrowed("wal"));

    /// Weighted Average Maturity (WAM) of the underlying pool.
    ///
    /// Units: years.
    pub const WAM: Self = Self(Cow::Borrowed("wam"));

    /// Constant Prepayment Rate (CPR), annualized.
    ///
    /// Units: decimal annual prepayment rate.
    pub const CPR: Self = Self(Cow::Borrowed("cpr"));

    /// Constant Default Rate (CDR), annualized.
    ///
    /// Units: decimal annual default rate.
    pub const CDR: Self = Self(Cow::Borrowed("cdr"));

    /// Spread duration, a time-weighted sensitivity to spread changes.
    ///
    /// Units: years.
    pub const SpreadDuration: Self = Self(Cow::Borrowed("spread_duration"));

    // ABS-specific Metrics

    /// Delinquency rate - Percentage of pool in delinquency
    pub const AbsDelinquency: Self = Self(Cow::Borrowed("abs_delinquency"));

    /// Charge-off rate - Percentage of pool charged off
    pub const AbsChargeOff: Self = Self(Cow::Borrowed("abs_charge_off"));

    /// Excess spread - Spread available to absorb losses
    pub const AbsExcessSpread: Self = Self(Cow::Borrowed("abs_excess_spread"));

    /// Credit enhancement level - Subordination as % of pool
    pub const AbsCreditEnhancement: Self = Self(Cow::Borrowed("abs_ce_level"));

    /// Monthly principal payment rate of a card master trust, percent per month
    pub const AbsPaymentRate: Self = Self(Cow::Borrowed("abs_payment_rate"));

    // CLO-specific Metrics

    /// Weighted Average Rating Factor
    pub const CloWarf: Self = Self(Cow::Borrowed("clo_warf"));

    /// Weighted average spread of the performing collateral, in basis points.
    pub const CloWas: Self = Self(Cow::Borrowed("clo_was"));

    // CMBS-specific Metrics

    /// Debt Service Coverage Ratio
    pub const CmbsDscr: Self = Self(Cow::Borrowed("cmbs_dscr"));

    // Asset-backed facility metrics

    /// Borrowing base on the closing collateral, in currency units
    pub const AbfBorrowingBase: Self = Self(Cow::Borrowed("abf_borrowing_base"));

    /// Borrowing-base cushion, (base − drawn) / base in percent
    pub const AbfBorrowingBaseCushion: Self = Self(Cow::Borrowed("abf_borrowing_base_cushion"));

    /// Advance-rate utilization, drawn / borrowing base
    pub const AbfAdvanceRateUtilization: Self = Self(Cow::Borrowed("abf_advance_rate_utilization"));

    /// Lender IRR of the facility flows (annual decimal)
    pub const AbfFacilityIrr: Self = Self(Cow::Borrowed("abf_facility_irr"));

    /// Residual IRR of the synthetic deal (annual decimal)
    pub const AbfResidualIrr: Self = Self(Cow::Borrowed("abf_residual_irr"));
}
