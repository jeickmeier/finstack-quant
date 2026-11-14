//! Prepayment model specifications for credit instruments.

use super::vector_at;

/// Prepayment curve shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "curve", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrepaymentModelSpec {
    /// Constant CPR (no seasoning effect)
    Constant {
        /// Annual decimal CPR in `[0, 1]`.
        cpr: f64,
    },
    /// PSA standard curve: ramps to 6% CPR over 30 months
    Psa {
        /// Speed multiplier (1.0 = 100% PSA)
        speed_multiplier: f64,
    },
    /// CMBS-style lockout: zero prepayment for an initial period, then constant CPR.
    ///
    /// Commercial mortgage-backed securities typically have prepayment lockout
    /// periods (defeasance/yield maintenance) lasting 5-10 years, after which
    /// voluntary prepayment resumes at the specified CPR.
    CmbsLockout {
        /// Number of months with zero prepayment (e.g., 60 for 5-year lockout)
        lockout_months: u32,
        /// Annual decimal CPR after lockout, in `[0, 1]`.
        post_lockout_cpr: f64,
    },
    /// ABS speed: each month a constant share of the *original* balance
    /// prepays, so the single-month mortality rises with seasoning,
    /// `SMM_t = ABS / (1 − ABS·(t − 1))` (Fabozzi, *Handbook of Fixed Income
    /// Securities*, auto-loan ABS convention).
    Abs {
        /// Monthly prepayment as a decimal fraction of the original balance
        /// (`0.015` = 1.5% ABS).
        speed: f64,
    },
    /// Explicit annual CPR for each month of seasoning; the last value is
    /// held for later months.
    Vector {
        /// Annual CPR per month of seasoning as decimals, month 1 first.
        monthly_cpr: Vec<f64>,
    },
}

impl PrepaymentModelSpec {
    /// Calculate SMM (single-month mortality) for the supplied seasoning.
    ///
    /// # Formula
    ///
    /// For the constant curve, the method converts annual CPR to monthly SMM
    /// using:
    ///
    /// `SMM = 1 - (1 - CPR)^(1/12)`
    ///
    /// For the PSA curve, the annual CPR is first derived from the seasoning:
    ///
    /// - months `1..=30`: `CPR = min(1.0, speed_multiplier * 0.06 * seasoning / 30)`
    /// - months `> 30`: `CPR = min(1.0, speed_multiplier * 0.06)`
    ///
    /// For the CMBS lockout curve:
    ///
    /// - months `<= lockout_months`: `CPR = 0`
    /// - months `> lockout_months`: `CPR = post_lockout_cpr`
    ///
    /// For the ABS curve the SMM is `speed / (1 − speed·(t − 1))` with
    /// `t = seasoning >= 1`, capped at 1.0 once the original balance is
    /// exhausted. For the vector curve the annual CPR is the entry for the
    /// seasoning month (1-indexed; later months hold the last entry).
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Number of months since origination or pool start
    ///   (`>= 1`).
    ///
    /// # Returns
    ///
    /// Monthly prepayment rate as a decimal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if:
    /// - `seasoning_months == 0`
    /// - the constant `cpr` or `post_lockout_cpr` is non-finite or outside `[0, 1]`
    /// - the PSA `speed_multiplier` is non-finite (NaN/∞) or negative
    /// - the ABS `speed` is outside `[0, 1]`
    /// - the vector curve is empty or holds a value outside `[0, 1]`
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn smm(&self, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
        if seasoning_months == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "seasoning_months must be >= 1".to_string(),
            ));
        }
        let cpr = match self {
            PrepaymentModelSpec::Constant { cpr } => *cpr,
            PrepaymentModelSpec::Psa { speed_multiplier } => {
                crate::builder::credit_rates::psa_cpr(*speed_multiplier, seasoning_months)?
            }
            PrepaymentModelSpec::CmbsLockout {
                lockout_months,
                post_lockout_cpr,
            } => {
                // Zero prepayment during lockout, then constant CPR
                if seasoning_months <= *lockout_months {
                    0.0
                } else {
                    *post_lockout_cpr
                }
            }
            PrepaymentModelSpec::Abs { speed } => {
                return crate::builder::credit_rates::abs_to_smm(*speed, seasoning_months);
            }
            PrepaymentModelSpec::Vector { monthly_cpr } => {
                vector_at(monthly_cpr, seasoning_months, "monthly_cpr")?
            }
        };

        use crate::builder::credit_rates::annual_to_monthly;
        annual_to_monthly(cpr)
    }

    /// Validate the curve parameters without evaluating a particular month.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a constant or post-lockout CPR that is
    /// non-finite or outside `[0, 1]`, a negative or non-finite PSA multiplier,
    /// an ABS speed outside `[0, 1]`, or a vector curve that is empty or holds a
    /// value outside `[0, 1]`. Post-lockout CPR is checked even during lockout.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if let Self::Constant { cpr }
        | Self::CmbsLockout {
            post_lockout_cpr: cpr,
            ..
        } = self
        {
            crate::builder::credit_rates::annual_to_monthly(*cpr)?;
        }
        if let PrepaymentModelSpec::Vector { monthly_cpr } = self {
            super::check_unit_vector(monthly_cpr, "monthly_cpr")?;
        }
        // Month 1 and the PSA/SDA peak cover every branch of the piecewise curves.
        self.smm(1)?;
        self.smm(30)?;
        Ok(())
    }

    /// Constant CPR (no curve).
    ///
    /// # Arguments
    ///
    /// * `cpr` - Annual constant prepayment rate as a decimal share.
    ///
    /// # Returns
    ///
    /// Prepayment model with no seasoning curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    ///
    /// let spec = PrepaymentModelSpec::constant_cpr(0.06);
    /// assert!(spec.smm(12)? > 0.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn constant_cpr(cpr: f64) -> Self {
        Self::Constant { cpr }
    }

    /// PSA curve with multiplier (1.0 = 100% PSA).
    ///
    /// The implementation uses the standard PSA ramp to a 6% annual CPR over
    /// 30 months, then holds that terminal CPR flat (capping annual CPR at
    /// `1.0` for high multipliers).
    ///
    /// The multiplier is validated at evaluation time:
    /// [`Self::smm`] rejects non-finite or negative multipliers.
    ///
    /// # Arguments
    ///
    /// * `speed_multiplier` - PSA speed multiplier, where `1.0` means 100% PSA.
    ///
    /// # Returns
    ///
    /// Prepayment model using the PSA seasoning curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    ///
    /// let spec = PrepaymentModelSpec::psa(1.0);
    /// assert!(spec.smm(30)? > spec.smm(1)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn psa(speed_multiplier: f64) -> Self {
        Self::Psa { speed_multiplier }
    }

    /// CMBS-style lockout: zero prepayment for `lockout_months`, then constant CPR.
    ///
    /// This models the common CMBS pattern where commercial mortgage borrowers
    /// face defeasance or yield maintenance penalties for an initial period,
    /// effectively preventing voluntary prepayment.
    ///
    /// # Arguments
    ///
    /// * `lockout_months` - Number of months with zero prepayment (e.g., 60 for 5 years)
    /// * `post_lockout_cpr` - Annual CPR after lockout expires (e.g., 0.10 for 10%)
    ///
    /// # Example
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    ///
    /// // 5-year lockout, then 10% CPR.
    /// let spec = PrepaymentModelSpec::cmbs_with_lockout(60, 0.10);
    /// assert_eq!(spec.smm(30)?, 0.0);
    /// assert!(spec.smm(61)? > 0.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn cmbs_with_lockout(lockout_months: u32, post_lockout_cpr: f64) -> Self {
        Self::CmbsLockout {
            lockout_months,
            post_lockout_cpr,
        }
    }

    /// ABS speed curve (auto-loan and consumer ABS convention).
    ///
    /// Every month the same share `speed` of the *original* balance prepays,
    /// so the single-month mortality on the remaining balance rises with
    /// seasoning: `SMM_t = speed / (1 − speed·(t − 1))`.
    ///
    /// # Arguments
    ///
    /// * `speed` - Monthly prepayment as a decimal fraction of the original
    ///   balance (`0.015` = 1.5% ABS), in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// Prepayment model using the ABS curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    ///
    /// let spec = PrepaymentModelSpec::abs(0.015);
    /// assert!((spec.smm(1)? - 0.015).abs() < 1e-15);
    /// assert!(spec.smm(36)? > spec.smm(1)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - Fabozzi, F. J. (ed.), *The Handbook of Fixed Income Securities*,
    ///   auto-loan ABS prepayment conventions.
    pub fn abs(speed: f64) -> Self {
        Self::Abs { speed }
    }

    /// Explicit annual CPR per month of seasoning (the last value is held).
    ///
    /// # Arguments
    ///
    /// * `monthly_cpr` - Annual CPR per seasoning month as decimals in
    ///   `[0, 1]`, month 1 first; must be non-empty.
    ///
    /// # Returns
    ///
    /// Prepayment model using the vector curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::PrepaymentModelSpec;
    ///
    /// let spec = PrepaymentModelSpec::vector(vec![0.02, 0.04, 0.06]);
    /// assert_eq!(spec.smm(3)?, spec.smm(12)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn vector(monthly_cpr: Vec<f64>) -> Self {
        Self::Vector { monthly_cpr }
    }
}

#[cfg(test)]
mod tests {
    use super::PrepaymentModelSpec;

    #[test]
    fn lockout_validation_checks_the_post_lockout_cpr() {
        for lockout_months in [0, 30, 60, u32::MAX] {
            for cpr in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(
                    PrepaymentModelSpec::cmbs_with_lockout(lockout_months, cpr)
                        .validate()
                        .is_err()
                );
            }
            for cpr in [0.0, 0.1, 1.0] {
                PrepaymentModelSpec::cmbs_with_lockout(lockout_months, cpr)
                    .validate()
                    .expect("valid post-lockout CPR");
            }
        }
    }
}
