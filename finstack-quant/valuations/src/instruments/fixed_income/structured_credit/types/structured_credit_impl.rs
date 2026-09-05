use super::waterfall;
use super::{
    CreditModelConfig, DealFees, DealType, DefaultModelSpec, MarketConditions, PaymentCalculation,
    PrepaymentModelSpec, Recipient, RecipientType, RecoveryModelSpec, StructuredCredit,
    TrancheSeniority, Waterfall,
};
use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;

impl Default for MarketConditions {
    fn default() -> Self {
        let (refi_rate, seasonal_factor) = embedded_registry_or_panic().market_conditions();
        Self {
            refi_rate,
            original_rate: None,
            hpa: None,
            unemployment: None,
            seasonal_factor,
            custom_factors: IndexMap::new(),
        }
    }
}

impl Default for CreditModelConfig {
    fn default() -> Self {
        Self {
            prepayment_spec: Self::default_prepayment_spec(),
            default_spec: Self::default_default_spec(),
            recovery_spec: Self::default_recovery_spec(),
            stochastic_prepay_spec: None,
            stochastic_default_spec: None,
            correlation_structure: None,
        }
    }
}

impl CreditModelConfig {
    pub(super) fn default_prepayment_spec() -> PrepaymentModelSpec {
        embedded_registry_or_panic().default_prepayment_spec()
    }

    pub(super) fn default_default_spec() -> DefaultModelSpec {
        embedded_registry_or_panic().default_default_spec()
    }

    pub(super) fn default_recovery_spec() -> RecoveryModelSpec {
        embedded_registry_or_panic().default_recovery_spec()
    }
}

impl StructuredCredit {
    /// Set the payment calendar ID for business day adjustments.
    ///
    /// This is required for accurate schedule generation. Structured credit deals
    /// are calendar-specific (e.g., NY, TARGET2), and using the wrong calendar
    /// shifts payment dates around holidays, breaking WAC/WAL and OC tests.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::dates::BusinessDayConvention;
    /// use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    ///
    /// let clo = StructuredCredit::example()
    ///     .with_payment_calendar("nyse")
    ///     .with_payment_business_day_convention(BusinessDayConvention::ModifiedFollowing);
    /// # let _ = clo;
    /// ```
    #[must_use]
    pub fn with_payment_calendar(mut self, calendar_id: impl Into<String>) -> Self {
        self.payment_calendar_id = Some(calendar_id.into());
        self
    }

    /// Set the business day convention for payment date adjustments.
    ///
    /// If not specified, defaults to `BusinessDayConvention::Following`.
    #[must_use]
    pub fn with_payment_business_day_convention(
        mut self,
        convention: BusinessDayConvention,
    ) -> Self {
        self.payment_business_day_convention = Some(convention);
        self
    }

    /// Set the clean-up call pool factor threshold.
    ///
    /// When the pool factor (current balance / original balance) drops below
    /// this threshold, the deal may be optionally redeemed. Tranches are paid
    /// in seniority order (senior first), bounded by remaining pool value.
    ///
    /// Industry standard: typically 0.10 (10%).
    ///
    /// # Errors
    /// Returns a validation error if `threshold` is not finite or is outside
    /// `(0.0, 1.0)`.
    pub fn with_cleanup_call(mut self, threshold: f64) -> finstack_quant_core::Result<Self> {
        if !threshold.is_finite() || threshold <= 0.0 || threshold >= 1.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "cleanup_call_pct must be finite and in (0, 1), got {threshold}"
            )));
        }
        self.cleanup_call_pct = Some(threshold);
        Ok(self)
    }

    /// Build the senior fee recipients for the waterfall's fee tier.
    ///
    /// Returns an empty vector when no [`DealFees`] are attached (fee tier
    /// skipped). Basis-point fees use [`PaymentCalculation::PercentageOfCollateral`]
    /// with `annualized: true`. The trustee fee is annual and is divided by
    /// payment periods per year.
    fn fee_recipients(&self) -> finstack_quant_core::Result<Vec<Recipient>> {
        Ok({
            let Some(fees) = self.fees.as_ref() else {
                return Ok(Vec::new());
            };
            let ccy = self.pool.get_base_currency();
            let mut recipients = Vec::new();

            fn bp_recipient(id: &str, name: &str, bp: f64) -> Option<Recipient> {
                (bp > 0.0 && bp.is_finite()).then(|| {
                    Recipient::new(
                        id,
                        RecipientType::ServiceProvider(name.to_string()),
                        PaymentCalculation::PercentageOfCollateral {
                            rate: bp / 10_000.0,
                            annualized: true,
                            day_count: None,
                            rounding: None,
                        },
                    )
                })
            }

            // Trustee fee first: a flat administrative charge senior to everything.
            let months_per_period = f64::from(self.frequency.months().unwrap_or(12).max(1));
            let periods_per_year = (12.0 / months_per_period).max(1.0);
            let trustee_period = fees.trustee_fee_annual.amount() / periods_per_year;
            if trustee_period > 0.0 && trustee_period.is_finite() {
                recipients.push(Recipient::new(
                    "trustee_fee",
                    RecipientType::ServiceProvider("Trustee".to_string()),
                    PaymentCalculation::FixedAmount {
                        amount: Money::new(trustee_period, ccy)?,
                        rounding: None,
                    },
                ));
            }

            recipients.extend(bp_recipient(
                "senior_mgmt_fee",
                "Manager",
                fees.senior_mgmt_fee_bp,
            ));
            recipients.extend(bp_recipient(
                "servicing_fee",
                "Servicer",
                fees.servicing_fee_bp,
            ));
            if let Some(bp) = fees.master_servicer_fee_bp {
                recipients.extend(bp_recipient("master_servicer_fee", "MasterServicer", bp));
            }
            if let Some(bp) = fees.special_servicer_fee_bp {
                recipients.extend(bp_recipient("special_servicer_fee", "SpecialServicer", bp));
            }
            // Subordinated management fees require a junior fee tier; including
            // them here would incorrectly make them senior to the notes.

            recipients
        })
    }

    /// Attach the deal-type standard fee calibration.
    ///
    /// Pulls the registry-backed [`DealFees`] for this deal's type — the same
    /// constants exposed by `types::constants` (CLO senior management, ABS
    /// servicing, CMBS master/special servicing, RMBS servicing).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    /// let deal = StructuredCredit::example().with_standard_fees();
    /// assert!(deal.fees.is_some());
    /// ```
    #[must_use]
    pub fn with_standard_fees(mut self) -> Self {
        let ccy = self.pool.get_base_currency();
        self.fees = Some(match self.deal_type {
            DealType::Clo | DealType::Cbo => DealFees::clo_standard(ccy),
            DealType::Cmbs => DealFees::cmbs_standard(ccy),
            DealType::Rmbs => DealFees::rmbs_standard(ccy),
            _ => DealFees::abs_standard(ccy),
        });
        self
    }

    /// Attach explicit transaction fees.
    #[must_use]
    pub fn with_fees(mut self, fees: DealFees) -> Self {
        self.fees = Some(fees);
        self
    }

    /// Attach overcollateralization / interest-coverage triggers to the deal.
    ///
    /// Each trigger names a tranche and the OC and/or IC ratio that must be
    /// maintained for it. When a test fails during simulation, the cure amount
    /// is diverted from divertible tiers to redeem senior notes. CLO/CBO
    /// templates also trap junior coupon; ABS/RMBS/CMBS keep coupons payable
    /// and turbo only residual cash.
    ///
    /// # Arguments
    ///
    /// * `triggers` - Coverage triggers to evaluate each payment period.
    ///
    /// # Errors
    ///
    /// Returns `Err` when a trigger level is not finite or not strictly
    /// positive, or when it names a tranche that is not part of this deal —
    /// a silently-ignored trigger would look like protection that is not there.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    /// #     StructuredCredit, waterfall::CoverageTrigger,
    /// # };
    /// # fn example(deal: StructuredCredit) -> finstack_quant_core::Result<()> {
    /// let deal = deal.with_coverage_triggers(vec![CoverageTrigger {
    ///     tranche_id: "CLASS_A".to_string(),
    ///     oc_trigger: Some(1.20),
    ///     ic_trigger: Some(1.15),
    /// }])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_coverage_triggers(
        mut self,
        triggers: Vec<waterfall::CoverageTrigger>,
    ) -> finstack_quant_core::Result<Self> {
        for trigger in &triggers {
            if !self
                .tranches
                .tranches
                .iter()
                .any(|t| t.id.as_str() == trigger.tranche_id)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "coverage trigger references tranche '{}', which is not part \
                     of deal '{}'",
                    trigger.tranche_id,
                    self.id.as_str()
                )));
            }
            for (label, level) in [("oc", trigger.oc_trigger), ("ic", trigger.ic_trigger)] {
                if let Some(level) = level {
                    if !level.is_finite() || level <= 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "{label}_trigger for tranche '{}' must be finite and \
                             positive, got {level}",
                            trigger.tranche_id
                        )));
                    }
                }
            }
        }
        self.coverage_triggers = triggers;
        Ok(self)
    }

    /// Calculate current loss percentage of the pool.
    ///
    /// Reconstructs the original pool balance as the denominator:
    /// `current_balance + cumulative_defaults + cumulative_prepayments + cumulative_scheduled_amortization`
    ///
    /// This avoids inflating the loss rate as the pool amortizes and aligns with
    /// Moody's/S&P convention of reporting cumulative losses against original pool balance.
    pub fn current_loss_percentage(&self) -> finstack_quant_core::Result<f64> {
        let current_balance = self.pool.total_balance()?.amount();
        let scheduled_amort = self.pool.cumulative_scheduled_amortization.amount();
        // Reconstruct original balance from all tracked reductions
        let original_balance = current_balance
            + self.pool.cumulative_defaults.amount()
            + self.pool.cumulative_prepayments.amount()
            + scheduled_amort;

        if original_balance <= 0.0 {
            return Ok(0.0);
        }

        Ok(
            (self.pool.cumulative_defaults.amount() - self.pool.cumulative_recoveries.amount())
                / original_balance
                * DECIMAL_TO_PERCENT,
        )
    }

    /// Create waterfall from instrument configuration.
    ///
    /// Returns the deal's custom [`Self::waterfall`] when one is attached,
    /// otherwise synthesizes the canonical sequential template. In both cases
    /// deal-level [`Self::coverage_triggers`] are appended for the coverage-test
    /// loop.
    pub fn create_waterfall(&self) -> finstack_quant_core::Result<Waterfall> {
        Ok({
            let mut waterfall = match self.waterfall.as_ref() {
                Some(custom) => custom.clone(),
                // Senior transaction fees, paid ahead of every note.
                None => Waterfall::standard_sequential(
                    self.deal_type,
                    self.pool.get_base_currency(),
                    &self.tranches,
                    self.fee_recipients()?,
                ),
            };

            // Attach deal OC/IC triggers for the waterfall coverage-test loop.
            for trigger in &self.coverage_triggers {
                waterfall = waterfall.add_coverage_trigger(trigger.clone());
            }
            waterfall
        })
    }

    /// Attach a fully custom payment waterfall, replacing the template.
    ///
    /// The waterfall is used verbatim by pricing (see [`Self::waterfall`] for
    /// the composition rules with `coverage_triggers` and `waterfall_rules`).
    /// Validation runs immediately so a malformed structure fails at
    /// construction rather than silently mispricing.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the waterfall is structurally invalid
    /// (duplicate tier/recipient ids, bad weights, non-finite parameters),
    /// references a tranche not in this deal, pays an equity tranche by id
    /// instead of [`RecipientType::Equity`], mismatches the pool currency,
    /// duplicates a coverage trigger, or when deal-level [`Self::fees`] are
    /// set (encode fees as leading `Fee` tiers of the waterfall instead).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    /// # fn example(deal: StructuredCredit) -> finstack_quant_core::Result<()> {
    /// // Start from the template and customize, or build from scratch.
    /// let waterfall = deal.create_waterfall().expect("valid create_waterfall fixture");
    /// let deal = deal.with_waterfall(waterfall)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `waterfall` - Waterfall used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn with_waterfall(mut self, waterfall: Waterfall) -> finstack_quant_core::Result<Self> {
        self.waterfall = Some(waterfall);
        self.validate_custom_waterfall()?;
        Ok(self)
    }

    /// Validate an attached custom waterfall against the deal.
    ///
    /// No-op when the deal carries no custom waterfall. Called by
    /// [`Self::with_waterfall`] for fail-fast construction and again by the
    /// simulation engine (and `validate_invariants`), so deals arriving via
    /// JSON — which never pass through `with_waterfall` — get identical
    /// errors instead of silently mispricing.
    pub(crate) fn validate_custom_waterfall(&self) -> finstack_quant_core::Result<()> {
        let Some(waterfall) = self.waterfall.as_ref() else {
            return Ok(());
        };
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);

        // Structural tier validation (duplicate ids, empty tiers, weights,
        // non-finite payment parameters) — shared with the executor.
        let tier_errors =
            crate::instruments::fixed_income::structured_credit::utils::get_validation_errors(
                &waterfall.tiers,
            );
        if let Some(first) = tier_errors.first() {
            return Err(invalid(format!(
                "custom waterfall is structurally invalid ({} error(s); first: {first})",
                tier_errors.len(),
            )));
        }
        if waterfall.tiers.is_empty() {
            return Err(invalid(
                "custom waterfall must define at least one tier".to_string(),
            ));
        }

        let pool_currency = self.pool.get_base_currency();
        if waterfall.base_currency != pool_currency {
            return Err(invalid(format!(
                "custom waterfall base_currency {} does not match pool currency {}",
                waterfall.base_currency, pool_currency
            )));
        }

        if self.fees.is_some() {
            return Err(invalid(
                "deal-level `fees` conflict with a custom waterfall; encode senior fees as \
                 leading Fee tiers of the waterfall instead (leading fee tiers feed the IC \
                 numerator and excess-spread sizing exactly like template fees)"
                    .to_string(),
            ));
        }

        let tranche = |id: &str| self.tranches.tranches.iter().find(|t| t.id.as_str() == id);

        // Every tranche-keyed recipient must resolve to a real, non-equity
        // tranche. The engine records equity flows under RecipientType::Equity
        // (see `tranche_recipient_keys` in the simulation engine), so paying an
        // equity tranche by id would distribute cash that is never recorded.
        //
        // The waterfall also DEFINES each tranche's interest claim (F3): at
        // most one interest-type recipient may name a tranche, otherwise the
        // claim is ambiguous (split-coupon tiers are not supported).
        let mut interest_claim_owner: finstack_quant_core::HashMap<&str, &str> =
            finstack_quant_core::HashMap::default();
        for tier in &waterfall.tiers {
            for recipient in &tier.recipients {
                let mut referenced: Vec<&str> = Vec::new();
                if let RecipientType::Tranche(id) = &recipient.recipient_type {
                    referenced.push(id.as_str());
                }
                match &recipient.calculation {
                    PaymentCalculation::TrancheInterest { tranche_id, .. }
                    | PaymentCalculation::CappedTrancheInterest { tranche_id, .. } => {
                        referenced.push(tranche_id.as_str());
                        if let Some(prev_tier) =
                            interest_claim_owner.insert(tranche_id.as_str(), tier.id.as_str())
                        {
                            return Err(invalid(format!(
                                "custom waterfall defines tranche '{tranche_id}'s interest \
                                 claim in both tier '{prev_tier}' and tier '{}'; a tranche's \
                                 interest claim must come from exactly one recipient",
                                tier.id
                            )));
                        }
                    }
                    PaymentCalculation::TranchePrincipal { tranche_id, .. } => {
                        referenced.push(tranche_id.as_str());
                    }
                    _ => {}
                }
                for id in referenced {
                    let Some(t) = tranche(id) else {
                        return Err(invalid(format!(
                            "custom waterfall tier '{}' recipient '{}' references unknown \
                             tranche '{id}'",
                            tier.id, recipient.id
                        )));
                    };
                    if t.seniority == TrancheSeniority::Equity {
                        return Err(invalid(format!(
                            "custom waterfall tier '{}' recipient '{}' pays equity tranche \
                             '{id}' by id; equity distributions must use RecipientType::Equity \
                             with ResidualCash (the engine records equity flows under that key)",
                            tier.id, recipient.id
                        )));
                    }
                }
            }
        }

        // Coverage triggers from the waterfall itself plus deal-level triggers
        // (appended by `create_waterfall`) must resolve, carry sane levels, and
        // not double-test one tranche — a duplicated trigger would evaluate and
        // cure the same test twice per period.
        let mut seen: finstack_quant_core::HashSet<&str> = finstack_quant_core::HashSet::default();
        for trigger in waterfall
            .coverage_triggers
            .iter()
            .chain(self.coverage_triggers.iter())
        {
            if tranche(trigger.tranche_id.as_str()).is_none() {
                return Err(invalid(format!(
                    "custom waterfall coverage trigger references unknown tranche '{}'",
                    trigger.tranche_id
                )));
            }
            if !seen.insert(trigger.tranche_id.as_str()) {
                return Err(invalid(format!(
                    "duplicate coverage trigger for tranche '{}' across the custom waterfall \
                     and deal-level coverage_triggers",
                    trigger.tranche_id
                )));
            }
            for (label, level) in [("oc", trigger.oc_trigger), ("ic", trigger.ic_trigger)] {
                if let Some(level) = level {
                    if !level.is_finite() || level <= 0.0 {
                        return Err(invalid(format!(
                            "{label}_trigger for tranche '{}' must be finite and positive, \
                             got {level}",
                            trigger.tranche_id
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

impl core::fmt::Debug for StructuredCredit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StructuredCredit")
            .field("id", &self.id)
            .field("deal_type", &self.deal_type)
            .field("closing_date", &self.closing_date)
            .field("first_payment_date", &self.first_payment_date)
            .field("maturity", &self.maturity)
            .field("frequency", &self.frequency)
            .field("discount_curve_id", &self.discount_curve_id)
            .finish()
    }
}

impl core::fmt::Display for StructuredCredit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let pool_balance = self
            .pool
            .total_balance()
            .unwrap_or(Money::from((0_i64, self.pool.get_base_currency())));
        let tranche_count = self.tranches.tranches.len();

        write!(
            f,
            "{} {:?} | AssetPool: {} {} | {} tranches | {} -> {}",
            self.id.as_str(),
            self.deal_type,
            pool_balance.amount(),
            pool_balance.currency(),
            tranche_count,
            self.closing_date,
            self.maturity,
        )
    }
}
