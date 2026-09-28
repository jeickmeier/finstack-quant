//! Loan terms shared by term loans, revolving credit facilities, asset-backed
//! facilities and structured-credit notes.
//!
//! These are the contractual terms a credit agreement carries: the coupon
//! ([`RateSpec`]), commitment changes, draws, margin and fee steps, letters of
//! credit, upfront economics and the effective-interest-rate reporting switch.
//! `TermLoan`, `RevolvingCredit` and `AssetBackedFacility` compose them so an
//! analyst enters a term sheet the same way for any facility.
//!
//! # Quick Example
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
//!     CommitmentStep, MarginStep,
//! };
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use rust_decimal_macros::dec;
//! use time::macros::date;
//!
//! // Commitment steps down to 6M on 2027-01-15 with a 25 bp reduction fee.
//! let step = CommitmentStep {
//!     date: date!(2027 - 01 - 15),
//!     amount: Money::new(6_000_000.0, Currency::USD)?,
//!     reduction_fee_bp: dec!(25),
//! };
//! // Margin rises 100 bp on the same date.
//! let margin = MarginStep { date: step.date, delta_bp: dec!(100) };
//! assert_eq!(margin.delta_bp, dec!(100));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::cashflow::builder::FloatingRateSpec;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Contractual coupon of a loan facility or note: a fixed all-in rate or a
/// floating index plus spread.
///
/// Shared by `TermLoan.rate`, `RevolvingCredit.rate`, `AssetBackedFacility.rate`
/// and structured-credit `Tranche.coupon`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
///
/// let fixed = RateSpec::Fixed { rate: 0.06 }; // 6% all-in
/// assert!(matches!(fixed, RateSpec::Fixed { .. }));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[allow(clippy::large_enum_variant)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RateSpec {
    /// Fixed all-in annual rate.
    Fixed {
        /// Annual rate as a decimal (`0.06` = 6%).
        rate: f64,
    },
    /// Floating index plus spread, with the canonical floor, cap, gearing and
    /// reset conventions of [`FloatingRateSpec`].
    Floating(FloatingRateSpec),
}

impl RateSpec {
    /// Fixed-rate spec from a typed rate.
    ///
    /// # Arguments
    ///
    /// * `rate` - All-in annual rate; stored as its decimal value.
    pub fn fixed_rate(rate: Rate) -> Self {
        Self::Fixed {
            rate: rate.as_decimal(),
        }
    }

    /// Current rate for a date without an index lookup, as a decimal.
    ///
    /// Returns the fixed rate, or only the spread of a floating spec (use
    /// [`Self::try_current_rate_with_index`] for the projected all-in rate).
    ///
    /// # Arguments
    ///
    /// * `_date` - Date the rate is wanted for; unused because neither arm
    ///   reads market data.
    pub fn current_rate(&self, _date: Date) -> f64 {
        match self {
            Self::Fixed { rate } => *rate,
            Self::Floating(spec) => spec.spread_bp.to_f64().unwrap_or_default() / 10_000.0,
        }
    }

    /// Current rate including the index forward where applicable, as a
    /// decimal.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual start the rate is projected for.
    /// * `market` - Market holding the forward curve and fixings named by
    ///   the floating spec's `forward_curve_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the forward curve or a required fixing is missing
    /// or the projection fails.
    pub fn try_current_rate_with_index(
        &self,
        date: Date,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        let as_of = match self {
            Self::Fixed { .. } => date,
            Self::Floating(spec) => market
                .get_forward(spec.forward_curve_id.as_str())?
                .base_date(),
        };
        self.try_rate_for_period(date, as_of, market)
    }

    /// Contractual rate for an explicit accrual period, as a decimal.
    ///
    /// A floating rate whose reset date (accrual start less `reset_lag_days`
    /// business days on the fixing calendar) is on or before `as_of` reads the
    /// fixing series; later resets project from the forward curve.
    ///
    /// # Arguments
    ///
    /// * `accrual_start` - Start of the accrual period.
    /// * `as_of` - Valuation date separating observed fixings from projections.
    /// * `market` - Market holding the forward curve and fixings.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixing calendar is unknown, a required fixing
    /// is missing, a seasoned compounded-overnight coupon is requested, or
    /// the projection fails.
    pub fn try_rate_for_period(
        &self,
        accrual_start: Date,
        as_of: Date,
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        match self {
            Self::Fixed { rate } => Ok(*rate),
            Self::Floating(spec) => {
                let fwd = market.get_forward(spec.forward_curve_id.as_str())?;
                let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                let calendar_id = spec
                    .fixing_calendar_id
                    .as_deref()
                    .unwrap_or("weekends_only");
                let calendar =
                    finstack_quant_core::dates::calendar_by_id(calendar_id).ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "structured-credit tranche fixing calendar '{}' is not registered",
                            calendar_id
                        ))
                    })?;
                let reset_date = finstack_quant_core::dates::DateExt::add_business_days(
                    accrual_start,
                    -spec.reset_lag_days,
                    calendar,
                )?;
                if reset_date <= as_of {
                    if spec.compounding.is_some() {
                        return Err(finstack_quant_core::Error::Validation(
                            "seasoned compounded-overnight tranche coupons require a canonical compounded fixing schedule"
                                .into(),
                        ));
                    }
                    let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
                        market,
                        spec.forward_curve_id.as_str(),
                    )?;
                    let raw =
                        finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                            Some(fixings),
                            spec.forward_curve_id.as_str(),
                            reset_date,
                            as_of,
                        )?;
                    return Ok(
                        crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                            raw, &params,
                        ),
                    );
                }
                crate::cashflow::builder::project_floating_rate(
                    accrual_start,
                    fwd.as_ref(),
                    &params,
                )
            }
        }
    }
}

/// A scheduled draw `{date, amount}` on a committed facility.
///
/// A delayed-draw term loan funds on `date` (inside its availability
/// window); an asset-backed facility funds on the first payment date on or
/// after `date`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DrawEvent {
    /// Date of the draw.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Amount drawn from the available commitment, in the facility currency
    /// (positive).
    pub amount: Money,
}

/// Dated margin step (covenant penalty, scheduled change or pricing-grid move).
///
/// Shifts the interest margin by `delta_bp` from `date` onward (effective-from
/// date). Steps are cumulative. A negative `delta_bp` steps the margin down (a
/// leverage-grid improvement); term loans accept only non-negative steps.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarginStep {
    /// Effective-from date of the margin change.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Change in margin, in basis points (`100` = 1%); negative steps down.
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub delta_bp: Decimal,
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
/// A delayed-draw term loan (`DdtlSpec::commitment_steps`) accepts only
/// non-increasing steps inside its availability window and no reduction fee
/// (`reduction_fee_bp` must be zero).
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
    /// One-off reduction or cancellation fee, in basis points of the reduced
    /// amount (not per annum), paid by the borrower on `date` when the
    /// commitment steps down. Ignored on a step up. Defaults to zero.
    #[serde(default, with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub reduction_fee_bp: Decimal,
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
/// Paid by the borrower to the lender on the issue date. Enters the
/// present value only while the commitment date lies after the valuation
/// date, and the effective-interest-rate metrics always.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UpfrontFee {
    /// Absolute amount in the facility currency.
    Amount(Money),
    /// Fraction of the opening commitment, as a decimal (`0.02` = 2%).
    FractionOfCommitment(f64),
}

impl UpfrontFee {
    /// Fee amount for a facility with the given opening commitment.
    ///
    /// # Arguments
    ///
    /// * `commitment` - Opening commitment the fractional form is applied to;
    ///   the absolute form ignores it.
    pub fn amount(&self, commitment: Money) -> Money {
        match self {
            Self::Amount(money) => *money,
            Self::FractionOfCommitment(fraction) => commitment * *fraction,
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
    fn upfront_fee_fraction_scales_with_the_commitment() {
        let commitment = Money::new(50_000_000.0, Currency::USD).expect("money");
        let pct = UpfrontFee::FractionOfCommitment(0.02).amount(commitment);
        assert!((pct.amount() - 1_000_000.0).abs() < 1e-9);
        let abs = UpfrontFee::Amount(Money::new(250_000.0, Currency::USD).expect("money"))
            .amount(commitment);
        assert!((abs.amount() - 250_000.0).abs() < 1e-9);
    }

    #[test]
    fn upfront_fee_wire_shape_is_tagged_snake_case() {
        let json = serde_json::to_value(UpfrontFee::FractionOfCommitment(0.02)).expect("json");
        assert_eq!(json, serde_json::json!({"fraction_of_commitment": 0.02}));
    }

    #[test]
    // schema-rejection-test
    fn retired_pct_of_commitment_key_is_rejected() {
        serde_json::from_value::<UpfrontFee>(serde_json::json!({"pct_of_commitment": 0.02}))
            .expect_err("pct_of_commitment is retired");
    }
}
