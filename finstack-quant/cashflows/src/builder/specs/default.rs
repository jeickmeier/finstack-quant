//! Default model specifications for credit instruments.

/// Default curve shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "curve", rename_all = "snake_case")]
pub enum DefaultCurve {
    /// Constant CDR (no seasoning effect)
    Constant,
    /// SDA standard curve: ramps to peak then declines
    Sda {
        /// Speed multiplier (1.0 = 100% SDA)
        speed_multiplier: f64,
    },
}

/// Default model specification.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DefaultModelSpec {
    /// CDR: Constant Default Rate (annual, e.g., 0.02 for 2%).
    ///
    /// This field is **ignored** when [`DefaultCurve::Sda`] is active: the
    /// annual CDR is then derived entirely from the SDA seasoning curve and
    /// its `speed_multiplier`.
    pub cdr: f64,
    /// Optional curve shape (default: constant)
    #[serde(default)]
    pub curve: Option<DefaultCurve>,
}

impl DefaultModelSpec {
    /// Calculate MDR (monthly default rate) for the supplied seasoning.
    ///
    /// # Formula
    ///
    /// For the constant curve, the method converts annual CDR to monthly MDR
    /// using:
    ///
    /// `MDR = 1 - (1 - CDR)^(1/12)`
    ///
    /// For the SDA curve, the annual CDR is first derived from seasoning using
    /// the PSA/BMA Standard Default Assumption (100 SDA):
    ///
    /// - months `1..=30`: linear ramp of 0.02% CDR per month to a 0.60% annual
    ///   CDR peak at month 30
    /// - months `31..=60`: flat 0.60% annual CDR plateau
    /// - months `61..=120`: linear decline from 0.60% to a 0.03% terminal
    ///   annual CDR at month 120
    /// - months `> 120`: flat 0.03% annual CDR terminal level
    ///
    /// The `speed_multiplier` scales the resulting annual CDR before conversion
    /// into MDR (e.g. `2.0` = 200 SDA).
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Number of months since origination or pool start.
    ///
    /// # Returns
    ///
    /// Monthly default rate as a decimal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if:
    /// - the SDA `speed_multiplier` is non-finite (NaN/∞) or negative
    /// - the scaled annual CDR exceeds 1.0 (e.g. an over-unity multiplier)
    ///
    /// Returns `InputError::NegativeValue`/`InputError::Invalid` if the
    /// constant `cdr` is negative or non-finite.
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn mdr(&self, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
        let cdr = match &self.curve {
            None | Some(DefaultCurve::Constant) => self.cdr,
            Some(DefaultCurve::Sda { speed_multiplier }) => {
                if !speed_multiplier.is_finite() || *speed_multiplier < 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "SDA speed_multiplier ({speed_multiplier}) must be finite and non-negative"
                    )));
                }
                // PSA/BMA 100 SDA: 0.02%/month ramp to 0.60% CDR at month 30,
                // flat plateau through month 60, linear decline to 0.03% by
                // month 120, flat thereafter.
                const PEAK_MONTH: u32 = 30;
                const PLATEAU_END_MONTH: u32 = 60;
                const TERMINAL_MONTH: u32 = 120;
                const PEAK_CDR: f64 = 0.006;
                const TERMINAL_CDR: f64 = 0.0003;

                let base = if seasoning_months <= PEAK_MONTH {
                    (seasoning_months as f64 / PEAK_MONTH as f64) * PEAK_CDR
                } else if seasoning_months <= PLATEAU_END_MONTH {
                    PEAK_CDR
                } else if seasoning_months <= TERMINAL_MONTH {
                    let past_plateau = (seasoning_months - PLATEAU_END_MONTH) as f64;
                    let decline_months = (TERMINAL_MONTH - PLATEAU_END_MONTH) as f64;
                    PEAK_CDR - (past_plateau / decline_months) * (PEAK_CDR - TERMINAL_CDR)
                } else {
                    TERMINAL_CDR
                };
                base * speed_multiplier
            }
        };

        if cdr > 1.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "annual CDR ({cdr}) derived from the default curve exceeds 1.0; \
                 check the curve speed_multiplier"
            )));
        }

        use super::super::credit_rates::cpr_to_smm;
        cpr_to_smm(cdr)
    }

    /// Constant CDR (no curve).
    ///
    /// # Arguments
    ///
    /// * `cdr` - Annual constant default rate as a decimal share.
    ///
    /// # Returns
    ///
    /// Default model with no seasoning curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// let spec = DefaultModelSpec::constant_cdr(0.02);
    /// assert!(spec.mdr(12)? > 0.0);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn constant_cdr(cdr: f64) -> Self {
        Self { cdr, curve: None }
    }

    /// SDA curve with multiplier (1.0 = 100% SDA).
    ///
    /// Implements the PSA/BMA Standard Default Assumption: annual CDR ramps
    /// 0.02%/month to a 0.60% peak at month 30, stays flat through month 60,
    /// declines linearly to a 0.03% terminal annual CDR at month 120, and is
    /// flat thereafter.
    ///
    /// While the SDA curve is active, the `cdr` field is ignored by
    /// [`Self::mdr`]; the stored value (the 100 SDA terminal CDR) is only a
    /// serde placeholder. The multiplier is validated at evaluation time:
    /// [`Self::mdr`] rejects non-finite or negative multipliers and any
    /// multiplier large enough to push the scaled annual CDR above 1.0.
    ///
    /// # Arguments
    ///
    /// * `speed_multiplier` - SDA speed multiplier, where `1.0` means 100% SDA.
    ///
    /// # Returns
    ///
    /// Default model using the SDA seasoning curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// let spec = DefaultModelSpec::sda(1.0);
    /// assert!(spec.mdr(30)? > spec.mdr(1)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn sda(speed_multiplier: f64) -> Self {
        Self {
            cdr: 0.0003, // 100% SDA terminal annual CDR
            curve: Some(DefaultCurve::Sda { speed_multiplier }),
        }
    }

    /// 2% CDR (common baseline).
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// Default model equivalent to [`Self::constant_cdr`] with `cdr = 0.02`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// let spec = DefaultModelSpec::cdr_2pct();
    /// assert_eq!(spec.cdr, 0.02);
    /// ```
    pub fn cdr_2pct() -> Self {
        Self::constant_cdr(0.02)
    }
}
