//! Dated loan terms shared by term loans and revolving credit facilities.
//!
//! These are the contractual schedules a credit agreement carries beyond the
//! coupon: commitment changes, margin and fee steps, letters of credit,
//! upfront economics and the effective-interest-rate reporting switch. Both
//! `TermLoan` and `RevolvingCredit` compose them so an analyst enters a term
//! sheet the same way for either instrument.
//!
//! # Quick Example
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
//!     CommitmentStep, MarginStepUp,
//! };
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use time::macros::date;
//!
//! // Commitment steps down to 6M on 2027-01-15 with a 25 bp reduction fee.
//! let step = CommitmentStep {
//!     date: date!(2027 - 01 - 15),
//!     amount: Money::new(6_000_000.0, Currency::USD)?,
//!     fee_bp: 25.0,
//! };
//! // Margin rises 100 bp on the same date.
//! let margin = MarginStepUp { date: step.date, delta_bp: 100 };
//! assert_eq!(margin.delta_bp, 100);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Margin step-up event (covenant penalty or scheduled increase).
///
/// Increases the interest margin by a fixed amount at a specified date,
/// typically triggered by covenant breach or scheduled rating migration. A
/// negative `delta_bp` steps the margin down (a leverage-grid improvement).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarginStepUp {
    /// Effective date of margin increase
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Increase in margin (basis points)
    pub delta_bp: i32,
}

/// Optional configuration for effective interest rate (EIR) amortization schedules.
///
/// When enabled, EIR amortization schedules are computed for reporting using
/// the loan's full cashflow schedule (including OID effects).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, default)]
pub struct OidEirSpec {
    /// Include fee cashflows (upfront, commitment, usage) in the EIR schedule.
    ///
    /// Defaults to true because these fees are typically part of the effective yield.
    pub include_fees: bool,
}

impl Default for OidEirSpec {
    fn default() -> Self {
        Self { include_fees: true }
    }
}

/// A scheduled change of a facility's commitment.
///
/// The commitment equals `amount` from `date` forward until the next step.
/// Steps down are amortizing commitments, availability expiries and voluntary
/// reductions; steps up are accordion exercises. Utilization is always drawn
/// balance over the commitment in force, so a stochastic revolving facility
/// books the implied principal change at the step.
///
/// A delayed-draw term loan (`DdtlSpec::commitment_step_downs`) accepts only
/// non-increasing steps inside its availability window and no reduction fee
/// (`fee_bp` must be `0.0`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommitmentStep {
    /// Date the new commitment takes effect (strictly after the commitment
    /// date, on or before maturity).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Commitment in force from `date`, in the facility currency (zero ends
    /// availability, as at a term-out). The drawn balance plus outstanding
    /// letters of credit must not exceed it.
    pub amount: Money,
    /// Reduction or cancellation fee, in basis points of the reduced amount,
    /// paid by the borrower on `date` when the commitment steps down. Ignored
    /// on a step up. Defaults to `0.0`.
    #[serde(default)]
    pub fee_bp: f64,
}

/// A dated change of a revolving facility's running fees.
///
/// Each delta shifts every tier of the corresponding fee, in basis points per
/// annum, from `date` forward. A leverage or ratings grid the analyst has
/// forecast is entered as one step per grid change.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FeeStep {
    /// Date the deltas take effect (strictly inside the facility life).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Change to the commitment fee on the undrawn commitment, in basis
    /// points per annum. Defaults to `0.0`.
    #[serde(default)]
    pub commitment_delta_bp: f64,
    /// Change to the usage fee on the drawn balance, in basis points per
    /// annum. Defaults to `0.0`.
    #[serde(default)]
    pub usage_delta_bp: f64,
    /// Change to the facility fee on the total commitment, in basis points
    /// per annum. Defaults to `0.0`.
    #[serde(default)]
    pub facility_delta_bp: f64,
}

/// A dated fixed fee under a facility: amendment, waiver, extension or
/// consent fees the borrower pays on a known date.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScheduledFee {
    /// Payment date (strictly after the commitment date, on or before
    /// maturity).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Fee amount in the facility currency (non-negative).
    pub amount: Money,
}

/// Upfront (arrangement or original-issue-discount) fee of a facility.
///
/// Paid by the borrower to the lender on the commitment date. Enters the
/// present value only while the commitment date lies after the valuation
/// date, and the effective-interest-rate metrics always.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UpfrontFee {
    /// Absolute amount in the facility currency.
    Amount(Money),
    /// Fraction of the opening commitment, as a decimal (`0.02` = 2%).
    PctOfCommitment(f64),
}

impl UpfrontFee {
    /// Fee amount for a facility with the given opening commitment.
    ///
    /// # Arguments
    ///
    /// * `commitment` - Opening commitment the percentage form is applied to;
    ///   the absolute form ignores it.
    pub fn amount(&self, commitment: Money) -> Money {
        match self {
            Self::Amount(money) => *money,
            Self::PctOfCommitment(pct) => commitment * *pct,
        }
    }
}

/// Issuance or expiry of a letter of credit under a facility's LC sublimit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LcEvent {
    /// Date the letter of credit is issued or expires (strictly after the
    /// simulation anchor, on or before maturity).
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Face amount issued or expiring, in the facility currency (positive).
    pub amount: Money,
    /// `true` for an issuance (LC outstanding rises), `false` for an expiry
    /// or cancellation.
    pub is_issue: bool,
}

/// Letter-of-credit sub-facility of a revolving credit facility.
///
/// Outstanding letters of credit reduce availability and the commitment-fee
/// base, count as usage for fee tiers, accrue an LC fee plus a fronting fee,
/// and are contingent exposure at default. LC usage is deterministic in both
/// pricing modes; a stochastic utilization process is capped at
/// `1 − LC(t) / C(t)`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LetterOfCreditSpec {
    /// Maximum LC face outstanding at any time, in the facility currency.
    pub sublimit: Money,
    /// LC face outstanding at the simulation anchor, in the facility currency.
    pub outstanding: Money,
    /// Future issuances and expiries, replayed on top of `outstanding`.
    #[serde(default)]
    pub events: Vec<LcEvent>,
    /// LC fee on the outstanding face, in basis points per annum. `None`
    /// accrues the facility's floating margin (the market convention for
    /// standby letters of credit); a fixed-rate facility must supply it.
    #[serde(default)]
    pub fee_bp: Option<f64>,
    /// Fronting fee paid to the issuing bank on the outstanding face, in
    /// basis points per annum. Defaults to `0.0`.
    #[serde(default)]
    pub fronting_fee_bp: f64,
    /// Fraction of the outstanding face assumed drawn at default (credit
    /// conversion factor), as a decimal in `[0, 1]`; funded at par and
    /// recovered at the facility recovery rate. Defaults to `0.0`.
    #[serde(default)]
    pub leq: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn upfront_fee_percentage_scales_with_the_commitment() {
        let commitment = Money::new(50_000_000.0, Currency::USD).expect("money");
        let pct = UpfrontFee::PctOfCommitment(0.02).amount(commitment);
        assert!((pct.amount() - 1_000_000.0).abs() < 1e-9);
        let abs = UpfrontFee::Amount(Money::new(250_000.0, Currency::USD).expect("money"))
            .amount(commitment);
        assert!((abs.amount() - 250_000.0).abs() < 1e-9);
    }

    #[test]
    fn upfront_fee_wire_shape_is_tagged_snake_case() {
        let json = serde_json::to_value(UpfrontFee::PctOfCommitment(0.02)).expect("json");
        assert_eq!(json, serde_json::json!({"pct_of_commitment": 0.02}));
    }
}
