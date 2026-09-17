//! Default model specifications for credit instruments.

use super::vector_at;

/// Default curve shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "curve", rename_all = "snake_case")]
pub enum DefaultCurve {
    /// Constant CDR (no seasoning effect)
    Constant,
    /// SDA standard curve: ramps to peak then declines
    Sda {
        /// Speed multiplier (1.0 = 100% SDA)
        speed_multiplier: f64,
    },
    /// Explicit annual CDR for each month of seasoning; the last value is
    /// held for later months. The `cdr` field is ignored.
    Vector {
        /// Annual CDR per month of seasoning as decimals, month 1 first.
        monthly_cdr: Vec<f64>,
    },
    /// Cumulative net loss curve (rating-agency ABS convention): losses as a
    /// percent of the original pool balance by month of seasoning, with a
    /// constant loss severity. Defaults in month `t` are
    /// `Δloss_t / severity` of the original balance; see
    /// [`DefaultModelSpec::mdr_with_survival`] for the conversion to a
    /// monthly rate on the surviving balance. The `cdr` field is ignored.
    CumulativeLoss {
        /// Cumulative net loss in percent of the original balance per month
        /// of seasoning (`1.5` = 1.5%), non-decreasing, month 1 first; the
        /// last value is held.
        cumulative_net_loss_pct: Vec<f64>,
        /// Loss severity as a decimal fraction of defaulted par in `(0, 1]`.
        severity: f64,
    },
    /// Rating-agency default timing: a lifetime cumulative default rate
    /// spread over the years of the pool's life. Defaults within a year
    /// accrue linearly by month. The `cdr` field is ignored.
    Timing {
        /// Lifetime defaults as a decimal fraction of the original balance.
        cumulative_default_rate: f64,
        /// Share of lifetime defaults occurring in each year of seasoning, in
        /// percent (e.g. `[15, 30, 30, 15, 10]`); must sum to 100.
        annual_pct: Vec<f64>,
    },
}

/// Default model specification.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DefaultModelSpec {
    /// CDR: Constant Default Rate (annual, e.g., 0.02 for 2%).
    ///
    /// This field is **ignored** when any curve other than
    /// [`DefaultCurve::Constant`] is active: the monthly rate is then derived
    /// entirely from the curve.
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
    /// For the vector curve the annual CDR is the entry for the seasoning
    /// month. For the cumulative-loss and timing curves the month's default
    /// share of the original balance is divided by the balance surviving the
    /// curve's own earlier defaults, which is exact for a pool without
    /// amortization or prepayments; engines that track the pool balance use
    /// [`Self::mdr_with_survival`] instead.
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
    /// - a vector curve is empty or holds an invalid value
    ///
    /// Returns `InputError::NegativeValue`/`InputError::Invalid` if the
    /// constant `cdr` is negative or non-finite.
    ///
    /// # References
    ///
    /// - `docs/REFERENCES.md#isda-cds-standard-model`
    /// - `docs/REFERENCES.md#tuckman-serrat-fixed-income`
    pub fn mdr(&self, seasoning_months: u32) -> finstack_quant_core::Result<f64> {
        self.mdr_impl(seasoning_months, None)
    }

    /// Monthly default rate on the balance actually surviving to the month.
    ///
    /// Identical to [`Self::mdr`] for the constant, SDA and vector curves. For
    /// the cumulative-loss and timing curves, which state defaults as a share
    /// of the *original* balance, the month's default share is divided by
    /// `surviving_balance_fraction` so that applying the rate to the current
    /// balance reproduces the curve's defaults regardless of amortization and
    /// prepayments.
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Number of months since origination or pool start.
    /// * `surviving_balance_fraction` - Current pool balance divided by the
    ///   balance the curve is expressed against, as a positive finite
    ///   decimal (may exceed 1.0 after par build).
    ///
    /// # Returns
    ///
    /// Monthly default rate as a decimal in `[0, 1]`; zero once the surviving
    /// balance is exhausted.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `surviving_balance_fraction` is not a
    /// finite non-negative number, plus every error of [`Self::mdr`].
    pub fn mdr_with_survival(
        &self,
        seasoning_months: u32,
        surviving_balance_fraction: f64,
    ) -> finstack_quant_core::Result<f64> {
        if !surviving_balance_fraction.is_finite() || surviving_balance_fraction < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "surviving_balance_fraction ({surviving_balance_fraction}) must be finite and non-negative"
            )));
        }
        self.mdr_impl(seasoning_months, Some(surviving_balance_fraction))
    }

    fn mdr_impl(
        &self,
        seasoning_months: u32,
        surviving_balance_fraction: Option<f64>,
    ) -> finstack_quant_core::Result<f64> {
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
            Some(DefaultCurve::Vector { monthly_cdr }) => {
                vector_at(monthly_cdr, seasoning_months, "monthly_cdr")?
            }
            Some(DefaultCurve::CumulativeLoss { .. } | DefaultCurve::Timing { .. }) => {
                let month = seasoning_months.max(1);
                let before = self.cumulative_default_fraction_checked(month - 1)?;
                let after = self.cumulative_default_fraction_checked(month)?;
                let defaulted = (after - before).max(0.0);
                let survival = surviving_balance_fraction.unwrap_or(1.0 - before);
                if survival <= f64::EPSILON {
                    return Ok(0.0);
                }
                return Ok((defaulted / survival).clamp(0.0, 1.0));
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

    /// Cumulative defaults through a seasoning month as a decimal fraction of
    /// the original balance, for the cumulative-loss and timing curves.
    ///
    /// # Arguments
    ///
    /// * `seasoning_months` - Number of months since origination; month 0 is
    ///   before any default.
    ///
    /// # Returns
    ///
    /// `Some(fraction)` for [`DefaultCurve::CumulativeLoss`] and
    /// [`DefaultCurve::Timing`], `None` for rate-based curves.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the curve's parameters are invalid
    /// (see [`Self::validate`]).
    pub fn cumulative_default_fraction(
        &self,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<Option<f64>> {
        match &self.curve {
            Some(DefaultCurve::CumulativeLoss { .. } | DefaultCurve::Timing { .. }) => Ok(Some(
                self.cumulative_default_fraction_checked(seasoning_months)?,
            )),
            _ => Ok(None),
        }
    }

    fn cumulative_default_fraction_checked(
        &self,
        seasoning_months: u32,
    ) -> finstack_quant_core::Result<f64> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        match &self.curve {
            Some(DefaultCurve::CumulativeLoss {
                cumulative_net_loss_pct,
                severity,
            }) => {
                if !severity.is_finite() || *severity <= 0.0 || *severity > 1.0 {
                    return Err(invalid(format!(
                        "cumulative-loss severity ({severity}) must be a decimal in (0, 1]"
                    )));
                }
                if seasoning_months == 0 {
                    return Ok(0.0);
                }
                let loss_pct = vector_at(
                    cumulative_net_loss_pct,
                    seasoning_months,
                    "cumulative_net_loss_pct",
                )?;
                Ok((loss_pct / 100.0 / severity).clamp(0.0, 1.0))
            }
            Some(DefaultCurve::Timing {
                cumulative_default_rate,
                annual_pct,
            }) => {
                if !cumulative_default_rate.is_finite()
                    || !(0.0..=1.0).contains(cumulative_default_rate)
                {
                    return Err(invalid(format!(
                        "timing cumulative_default_rate ({cumulative_default_rate}) must be a decimal in [0, 1]"
                    )));
                }
                if annual_pct.is_empty() {
                    return Err(invalid("timing annual_pct must not be empty".to_string()));
                }
                let mut share = 0.0_f64;
                for (year, pct) in annual_pct.iter().enumerate() {
                    if !pct.is_finite() || *pct < 0.0 {
                        return Err(invalid(format!(
                            "timing annual_pct[{year}] ({pct}) must be finite and non-negative"
                        )));
                    }
                    let year_start = 12 * year as u32;
                    let elapsed = seasoning_months.saturating_sub(year_start).min(12);
                    share += pct / 100.0 * f64::from(elapsed) / 12.0;
                }
                Ok((cumulative_default_rate * share).clamp(0.0, 1.0))
            }
            _ => Ok(0.0),
        }
    }

    /// Validate the curve parameters without evaluating a particular month.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` for a non-finite or negative constant CDR,
    /// an invalid SDA multiplier, a vector curve that is empty or holds a
    /// value outside `[0, 1]`, a cumulative-loss curve that is empty,
    /// decreasing, negative or paired with a severity outside `(0, 1]`, or a
    /// timing curve whose `annual_pct` does not sum to 100 or whose
    /// cumulative default rate is outside `[0, 1]`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        match &self.curve {
            Some(DefaultCurve::Vector { monthly_cdr }) => {
                for (index, cdr) in monthly_cdr.iter().enumerate() {
                    if !cdr.is_finite() || !(0.0..=1.0).contains(cdr) {
                        return Err(invalid(format!(
                            "monthly_cdr[{index}] ({cdr}) must be a decimal in [0, 1]"
                        )));
                    }
                }
            }
            Some(DefaultCurve::CumulativeLoss {
                cumulative_net_loss_pct,
                ..
            }) => {
                let mut previous = 0.0_f64;
                for (index, loss) in cumulative_net_loss_pct.iter().enumerate() {
                    if !loss.is_finite() || *loss < previous {
                        return Err(invalid(format!(
                            "cumulative_net_loss_pct[{index}] ({loss}) must be finite and non-decreasing"
                        )));
                    }
                    previous = *loss;
                }
                let last = cumulative_net_loss_pct.len() as u32;
                self.cumulative_default_fraction_checked(last.max(1))?;
            }
            Some(DefaultCurve::Timing { annual_pct, .. }) => {
                let total: f64 = annual_pct.iter().sum();
                if (total - 100.0).abs() > 1e-6 {
                    return Err(invalid(format!(
                        "timing annual_pct must sum to 100, got {total}"
                    )));
                }
                let last = 12 * annual_pct.len() as u32;
                self.cumulative_default_fraction_checked(last.max(1))?;
            }
            _ => {}
        }
        // Month 1 and the SDA peak cover every branch of the piecewise curves.
        self.mdr(1)?;
        self.mdr(30)?;
        Ok(())
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

    /// Explicit annual CDR per month of seasoning (the last value is held).
    ///
    /// The stored `cdr` is the first vector entry and is ignored by
    /// [`Self::mdr`].
    ///
    /// # Arguments
    ///
    /// * `monthly_cdr` - Annual CDR per seasoning month as decimals in
    ///   `[0, 1]`, month 1 first; must be non-empty.
    ///
    /// # Returns
    ///
    /// Default model using the vector curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// let spec = DefaultModelSpec::vector(vec![0.01, 0.03]);
    /// assert_eq!(spec.mdr(2)?, spec.mdr(24)?);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn vector(monthly_cdr: Vec<f64>) -> Self {
        Self {
            cdr: monthly_cdr.first().copied().unwrap_or(0.0),
            curve: Some(DefaultCurve::Vector { monthly_cdr }),
        }
    }

    /// Cumulative net loss curve with a constant severity.
    ///
    /// The stored `cdr` is zero and ignored by [`Self::mdr`].
    ///
    /// # Arguments
    ///
    /// * `cumulative_net_loss_pct` - Cumulative net loss in percent of the
    ///   original balance per seasoning month (`1.5` = 1.5%), non-decreasing,
    ///   month 1 first; the last value is held.
    /// * `severity` - Loss severity as a decimal fraction of defaulted par in
    ///   `(0, 1]`; defaults are `loss / severity`.
    ///
    /// # Returns
    ///
    /// Default model using the cumulative-loss curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// // 2% lifetime net loss at 50% severity is a 4% lifetime default rate.
    /// let spec = DefaultModelSpec::cumulative_loss(vec![0.5, 1.0, 1.5, 2.0], 0.5);
    /// assert_eq!(spec.cumulative_default_fraction(4)?, Some(0.04));
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn cumulative_loss(cumulative_net_loss_pct: Vec<f64>, severity: f64) -> Self {
        Self {
            cdr: 0.0,
            curve: Some(DefaultCurve::CumulativeLoss {
                cumulative_net_loss_pct,
                severity,
            }),
        }
    }

    /// Rating-agency default timing over a lifetime cumulative default rate.
    ///
    /// The stored `cdr` is zero and ignored by [`Self::mdr`].
    ///
    /// # Arguments
    ///
    /// * `cumulative_default_rate` - Lifetime defaults as a decimal fraction
    ///   of the original balance in `[0, 1]`.
    /// * `annual_pct` - Share of lifetime defaults in each year of seasoning,
    ///   in percent (e.g. `[15, 30, 30, 15, 10]`), summing to 100.
    ///
    /// # Returns
    ///
    /// Default model using the timing curve.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_cashflows::builder::DefaultModelSpec;
    ///
    /// let spec = DefaultModelSpec::timing(0.06, vec![15.0, 30.0, 30.0, 15.0, 10.0]);
    /// assert_eq!(spec.cumulative_default_fraction(60)?, Some(0.06));
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn timing(cumulative_default_rate: f64, annual_pct: Vec<f64>) -> Self {
        Self {
            cdr: 0.0,
            curve: Some(DefaultCurve::Timing {
                cumulative_default_rate,
                annual_pct,
            }),
        }
    }
}
