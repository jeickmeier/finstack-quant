//! Delinquency, servicer advancing and loan-modification model for asset and
//! rep-line pools (auto/consumer ABS, non-agency RMBS).
//!
//! Without a model, a defaulting balance leaves the pool the period the
//! default rate says it does. With a [`DelinquencyModel`] the default model
//! instead feeds the first delinquency bucket, balances roll bucket to
//! bucket (30 → 60 → 90 days) or cure, and only the roll out of the last
//! bucket is a charge-off. Delinquent balances stay in the pool's par (they
//! count for OC tests and step-down metrics) but pay no interest or
//! scheduled principal until they cure; a servicer may advance the missed
//! amounts and is reimbursed from recovery proceeds ahead of the waterfall.

use serde::{Deserialize, Serialize};

/// Roll-rate delinquency model with optional servicer advancing and loan
/// modification.
///
/// `roll_rates[b]` and `cure_rates[b]` are monthly transition probabilities
/// for the balance in bucket `b` (bucket 0 = 30 days past due). The balance
/// that neither rolls nor cures stays in its bucket. The roll out of the last
/// bucket is the charge-off, so a 30/60/90 model with charge-off at 120 days
/// has three entries.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
///     AdvancingPolicy, DelinquencyModel,
/// };
///
/// let model = DelinquencyModel::new(vec![0.6, 0.7, 0.9], vec![0.3, 0.2, 0.05])
///     .with_advancing(AdvancingPolicy::PrincipalAndInterest {
///         recoverability_cap_pct: 100.0,
///     });
/// assert_eq!(model.buckets(), 3);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DelinquencyModel {
    /// Monthly probability, per bucket, that the bucket's balance rolls to
    /// the next bucket; the last entry rolls to charge-off. Decimals in
    /// `[0, 1]`; the length is the number of buckets.
    pub roll_rates: Vec<f64>,
    /// Monthly probability, per bucket, that the bucket's balance returns to
    /// current. Decimals in `[0, 1]`, same length as `roll_rates`, with
    /// `roll + cure ≤ 1` for every bucket.
    pub cure_rates: Vec<f64>,
    /// Servicer advancing of the interest and scheduled principal that
    /// delinquent balances miss. Defaults to no advancing.
    #[serde(default)]
    pub advancing: AdvancingPolicy,
    /// Loan modification program applied to delinquent balances every
    /// month; `None` for no modifications.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modification: Option<ModificationSpec>,
}

/// Servicer advancing policy for missed principal and interest.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "policy", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdvancingPolicy {
    /// The servicer advances nothing; delinquent balances simply stop paying.
    #[default]
    None,
    /// The servicer advances the interest and scheduled principal that
    /// delinquent balances miss, as long as the advances outstanding stay
    /// within `recoverability_cap_pct` percent of the delinquent balance.
    /// Advances are reimbursed from the recovery proceeds of charge-offs
    /// before that cash reaches the waterfall.
    PrincipalAndInterest {
        /// Cap on advances outstanding as a percent of the delinquent
        /// balance (`100.0` = advance up to the full delinquent balance).
        recoverability_cap_pct: f64,
    },
}

/// Loan modification program: every month a share of each delinquency
/// bucket is modified back to current with a lower coupon and, for
/// level-pay collateral, a recast payment over an extended term.
///
/// On a rep line the modified share blends into the line's coupon and level
/// payment; the line's maturity is unchanged, so principal the extended term
/// pushes past it is paid as a balloon on that date.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ModificationSpec {
    /// Coupon reduction granted to modified balances, in basis points.
    pub rate_reduction_bp: f64,
    /// Term extension granted to modified level-pay balances, in months.
    pub term_extension_months: u32,
    /// Share of each bucket's balance modified per month, as a decimal in
    /// `[0, 1]`.
    pub share_of_delinquent: f64,
}

impl DelinquencyModel {
    /// Build a model from its monthly roll and cure rates with no advancing
    /// and no modification.
    ///
    /// # Arguments
    ///
    /// * `roll_rates` - Monthly roll probability per bucket as decimals; the
    ///   last entry rolls to charge-off.
    /// * `cure_rates` - Monthly cure probability per bucket as decimals,
    ///   same length as `roll_rates`.
    ///
    /// # Returns
    ///
    /// The model; call [`validate`](Self::validate) or price the deal to
    /// check the rates.
    pub fn new(roll_rates: Vec<f64>, cure_rates: Vec<f64>) -> Self {
        Self {
            roll_rates,
            cure_rates,
            advancing: AdvancingPolicy::None,
            modification: None,
        }
    }

    /// Set the servicer advancing policy.
    ///
    /// # Arguments
    ///
    /// * `advancing` - Policy applied to missed principal and interest.
    ///
    /// # Returns
    ///
    /// The model with the policy attached.
    #[must_use]
    pub fn with_advancing(mut self, advancing: AdvancingPolicy) -> Self {
        self.advancing = advancing;
        self
    }

    /// Set the loan modification program.
    ///
    /// # Arguments
    ///
    /// * `modification` - Monthly modification terms applied to delinquent
    ///   balances.
    ///
    /// # Returns
    ///
    /// The model with the program attached.
    #[must_use]
    pub fn with_modification(mut self, modification: ModificationSpec) -> Self {
        self.modification = Some(modification);
        self
    }

    /// Number of delinquency buckets before charge-off.
    #[must_use]
    pub fn buckets(&self) -> usize {
        self.roll_rates.len()
    }

    /// Validate the transition probabilities, the advancing cap and the
    /// modification terms.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the model has no buckets, the roll
    /// and cure vectors differ in length, any rate is outside `[0, 1]`, a
    /// bucket's `roll + cure` exceeds 1, the advancing cap is negative or
    /// non-finite, or the modification share, reduction or extension is
    /// invalid.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        if self.roll_rates.is_empty() {
            return Err(invalid(
                "delinquency model needs at least one bucket (roll_rates is empty)".to_string(),
            ));
        }
        if self.roll_rates.len() != self.cure_rates.len() {
            return Err(invalid(format!(
                "delinquency roll_rates ({}) and cure_rates ({}) must have the same length",
                self.roll_rates.len(),
                self.cure_rates.len()
            )));
        }
        for (bucket, (roll, cure)) in self.roll_rates.iter().zip(&self.cure_rates).enumerate() {
            let unit = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
            if !unit(*roll) || !unit(*cure) {
                return Err(invalid(format!(
                    "delinquency bucket {bucket}: roll ({roll}) and cure ({cure}) must be decimals in [0, 1]"
                )));
            }
            if roll + cure > 1.0 + 1e-12 {
                return Err(invalid(format!(
                    "delinquency bucket {bucket}: roll + cure ({}) exceeds 1",
                    roll + cure
                )));
            }
        }
        if let AdvancingPolicy::PrincipalAndInterest {
            recoverability_cap_pct,
        } = self.advancing
        {
            if !recoverability_cap_pct.is_finite() || recoverability_cap_pct < 0.0 {
                return Err(invalid(format!(
                    "advancing recoverability_cap_pct ({recoverability_cap_pct}) must be a finite non-negative percent"
                )));
            }
        }
        if let Some(modification) = self.modification {
            if !modification.rate_reduction_bp.is_finite() || modification.rate_reduction_bp < 0.0 {
                return Err(invalid(format!(
                    "modification rate_reduction_bp ({}) must be finite and non-negative",
                    modification.rate_reduction_bp
                )));
            }
            if !modification.share_of_delinquent.is_finite()
                || !(0.0..=1.0).contains(&modification.share_of_delinquent)
            {
                return Err(invalid(format!(
                    "modification share_of_delinquent ({}) must be a decimal in [0, 1]",
                    modification.share_of_delinquent
                )));
            }
        }
        Ok(())
    }
}
