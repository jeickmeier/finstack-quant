//! Revolving credit fee structures, utilization tiers, and dated fee steps.

use crate::loan_terms::{FeeStep, UpfrontFee};
use finstack_quant_cashflows::builder::{FeeTier, evaluate_fee_tiers};
use finstack_quant_core::dates::Date;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Fee structure for a revolving credit facility.
///
/// Contains the various fees charged on the facility:
/// - Upfront: one-time fee at commitment
/// - Commitment: annual fee on undrawn amount (can be tiered by utilization)
/// - Usage: annual fee on drawn amount (can be tiered by utilization)
/// - Facility: annual fee on total commitment
///
/// Flat fees can be represented as single-tier vectors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RevolvingCreditFees {
    /// One-time upfront (arrangement or OID) fee paid by the borrower to the
    /// lender on the commitment date, as an absolute amount or a fraction of
    /// the opening commitment. Enters the present value only while the
    /// commitment date lies after the valuation date, and the effective-rate
    /// metrics always.
    #[serde(default)]
    pub upfront_fee: Option<UpfrontFee>,

    /// Commitment fee tiers (utilization-based). Empty vector means no commitment fee.
    /// Tiers should be sorted by threshold ascending.
    #[serde(default)]
    pub commitment_fee_tiers: Vec<FeeTier>,

    /// Usage fee tiers (utilization-based). Empty vector means no usage fee.
    /// Tiers should be sorted by threshold ascending.
    #[serde(default)]
    pub usage_fee_tiers: Vec<FeeTier>,

    /// Annual facility fee rate on total commitment (basis points).
    /// Facility fee is not tiered (applies to total commitment regardless of utilization).
    pub facility_fee_bp: f64,

    /// Dated fee changes, cumulative from their dates, each shifting every
    /// tier of the corresponding fee in basis points per annum. Dates must be
    /// strictly increasing and strictly inside the facility life. Empty by
    /// default.
    #[serde(default)]
    pub steps: Vec<FeeStep>,
}

impl Default for RevolvingCreditFees {
    fn default() -> Self {
        Self {
            upfront_fee: None,
            commitment_fee_tiers: Vec::new(),
            usage_fee_tiers: Vec::new(),
            facility_fee_bp: 0.0,
            steps: Vec::new(),
        }
    }
}

/// Cumulative fee deltas in force on a date, in basis points per annum.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FeeDeltas {
    /// Shift of every commitment-fee tier.
    pub commitment_bp: f64,
    /// Shift of every usage-fee tier.
    pub usage_bp: f64,
    /// Shift of the facility fee.
    pub facility_bp: f64,
}

impl RevolvingCreditFees {
    /// Create fees with flat (non-tiered) commitment and usage fees.
    ///
    /// Convenience constructor for simple fee structures without utilization tiers.
    ///
    /// # Arguments
    ///
    /// * `commitment_fee_bp` - Annual commitment fee on the undrawn amount, in
    ///   basis points (e.g. `25.0` for 25bp). Must be finite; values `<= 0`
    ///   produce no commitment fee tier.
    /// * `usage_fee_bp` - Annual usage fee on the drawn amount, in basis
    ///   points. Must be finite; values `<= 0` produce no usage fee tier.
    /// * `facility_fee_bp` - Annual facility fee on the total commitment, in
    ///   basis points. Must be finite.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any fee input
    /// is not finite (previously non-finite inputs silently produced a zero
    /// fee in release builds, masking data-quality errors).
    pub fn flat(
        commitment_fee_bp: f64,
        usage_fee_bp: f64,
        facility_fee_bp: f64,
    ) -> finstack_quant_core::Result<Self> {
        for (name, bp) in [
            ("commitment_fee_bp", commitment_fee_bp),
            ("usage_fee_bp", usage_fee_bp),
            ("facility_fee_bp", facility_fee_bp),
        ] {
            if !bp.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "RevolvingCreditFees::flat: {name} must be finite, got {bp}"
                )));
            }
        }
        let make_tier = |bp: f64| -> Vec<FeeTier> {
            if bp > 0.0 {
                vec![FeeTier {
                    threshold: Decimal::ZERO,
                    bp: finstack_quant_core::decimal::f64_to_decimal(bp).unwrap_or(Decimal::ZERO),
                }]
            } else {
                Vec::new()
            }
        };

        Ok(Self {
            upfront_fee: None,
            commitment_fee_tiers: make_tier(commitment_fee_bp),
            usage_fee_tiers: make_tier(usage_fee_bp),
            facility_fee_bp,
            steps: Vec::new(),
        })
    }

    /// Cumulative fee deltas in force on `date`: the sum of every step dated
    /// on or before it.
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the running fees are wanted for.
    pub fn deltas_at(&self, date: Date) -> FeeDeltas {
        self.steps.iter().filter(|step| step.date <= date).fold(
            FeeDeltas::default(),
            |acc, step| FeeDeltas {
                commitment_bp: acc.commitment_bp + step.commitment_delta_bp,
                usage_bp: acc.usage_bp + step.usage_delta_bp,
                facility_bp: acc.facility_bp + step.facility_delta_bp,
            },
        )
    }

    /// Commitment fee in force on `date` for the given utilization: the tier
    /// rate plus the cumulative step delta, floored at zero.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment in force, as a
    ///   decimal in `[0, 1]`, used to select the tier.
    /// * `date` - Accrual date the steps are evaluated on.
    ///
    /// # Errors
    ///
    /// Same as [`Self::commitment_fee_bp`].
    pub fn commitment_fee_bp_at(
        &self,
        utilization: f64,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok((self.commitment_fee_bp(utilization)? + self.deltas_at(date).commitment_bp).max(0.0))
    }

    /// Usage fee in force on `date` for the given utilization (tier rate plus
    /// cumulative step delta, floored at zero).
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment in force, as a
    ///   decimal in `[0, 1]`, used to select the tier.
    /// * `date` - Accrual date the steps are evaluated on.
    ///
    /// # Errors
    ///
    /// Same as [`Self::usage_fee_bp`].
    pub fn usage_fee_bp_at(
        &self,
        utilization: f64,
        date: Date,
    ) -> finstack_quant_core::Result<f64> {
        Ok((self.usage_fee_bp(utilization)? + self.deltas_at(date).usage_bp).max(0.0))
    }

    /// Facility fee in force on `date` (flat rate plus cumulative step delta,
    /// floored at zero).
    ///
    /// # Arguments
    ///
    /// * `date` - Accrual date the steps are evaluated on.
    pub fn facility_fee_bp_at(&self, date: Date) -> f64 {
        (self.facility_fee_bp + self.deltas_at(date).facility_bp).max(0.0)
    }

    /// Commitment fee in basis points for a utilization: the rate of the
    /// highest tier whose threshold the utilization reaches, or `0.0` when no
    /// tier applies. Tiers must be sorted by threshold ascending.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment, as a
    ///   decimal (`0.4` = 40% used).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `utilization` is not finite or the
    /// tiers are not strictly ascending.
    pub fn commitment_fee_bp(&self, utilization: f64) -> finstack_quant_core::Result<f64> {
        tier_bp(&self.commitment_fee_tiers, utilization, "commitment")
    }

    /// Usage fee in basis points for a utilization: the rate of the highest
    /// tier whose threshold the utilization reaches, or `0.0` when no tier
    /// applies. Tiers must be sorted by threshold ascending.
    ///
    /// # Arguments
    ///
    /// * `utilization` - Drawn plus LC usage over the commitment, as a
    ///   decimal (`0.4` = 40% used).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `utilization` is not finite or the
    /// tiers are not strictly ascending.
    pub fn usage_fee_bp(&self, utilization: f64) -> finstack_quant_core::Result<f64> {
        tier_bp(&self.usage_fee_tiers, utilization, "usage")
    }
}

/// Tier rate in basis points for a finite utilization.
fn tier_bp(tiers: &[FeeTier], utilization: f64, label: &str) -> finstack_quant_core::Result<f64> {
    let util = finstack_quant_core::decimal::f64_to_decimal(utilization).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit {label} fee: utilization must be finite, got {utilization}"
        ))
    })?;
    evaluate_fee_tiers(tiers, util)?.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "RevolvingCredit {label} fee tier rate is not representable as f64"
        ))
    })
}
