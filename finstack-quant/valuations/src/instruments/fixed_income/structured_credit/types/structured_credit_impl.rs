use super::{
    CoverageTestSpec, CreditModelConfig, DealFees, DealType, DefaultModelSpec, HedgeSwap,
    LossAllocationPolicy, MarketConditions, PaymentCalculation, PaymentType, PrepaymentModelSpec,
    Recipient, RecipientType, RecoveryModelSpec, StructuredCredit, TemplateFees, TrancheSeniority,
    Waterfall,
};
use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::money::Money;

impl Default for MarketConditions {
    fn default() -> Self {
        Self {
            refi_rate: embedded_registry_or_panic().market_conditions(),
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
            delinquency: None,
            card: None,
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

    /// Build the template's fee recipients from the deal's [`DealFees`]:
    /// senior fees (trustee, senior management, servicing) ahead of every
    /// note, the subordinated management fee after the note coupons, and the
    /// incentive fee ahead of the residual.
    ///
    /// Returns empty recipients when no fees are attached (fee tiers skipped).
    /// Basis-point fees use [`PaymentCalculation::PercentageOfCollateral`]
    /// with `annualized: true`. The trustee fee is annual and is divided by
    /// payment periods per year.
    fn template_fees(&self) -> finstack_quant_core::Result<TemplateFees> {
        let Some(fees) = self.fees.as_ref() else {
            return Ok(TemplateFees::default());
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
            // Special servicing is paid on the specially serviced loans only.
            if bp > 0.0 && bp.is_finite() {
                recipients.push(Recipient::new(
                    "special_servicer_fee",
                    RecipientType::ServiceProvider("SpecialServicer".to_string()),
                    PaymentCalculation::PercentageOfSpecialServiced {
                        rate: bp / 10_000.0,
                        annualized: true,
                        day_count: None,
                        rounding: None,
                    },
                ));
            }
        }

        // The subordinated management fee ranks after every note coupon.
        let junior = (fees.subordinated_mgmt_fee_bp > 0.0
            && fees.subordinated_mgmt_fee_bp.is_finite())
        .then(|| {
            Recipient::new(
                "subordinated_mgmt_fee",
                RecipientType::ManagerFee(super::ManagementFeeType::Subordinated),
                PaymentCalculation::PercentageOfCollateral {
                    rate: fees.subordinated_mgmt_fee_bp / 10_000.0,
                    annualized: true,
                    day_count: None,
                    rounding: None,
                },
            )
        })
        .into_iter()
        .collect();
        let incentive = fees.incentive_fee.map(|spec| {
            Recipient::new(
                "incentive_fee",
                RecipientType::ManagerFee(super::ManagementFeeType::Incentive),
                PaymentCalculation::IncentiveFee {
                    hurdle_irr: spec.hurdle_irr,
                    share_pct: spec.share_pct,
                },
            )
        });

        Ok(TemplateFees {
            senior: recipients,
            junior,
            incentive,
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

    /// Attach a hedge swap that settles through the waterfall (chainable).
    ///
    /// # Arguments
    ///
    /// * `hedge` - Swap, tracked notional and fee-tier priority; validated
    ///   against the deal currency and tranches when the deal is priced.
    #[must_use]
    pub fn with_hedge_swap(mut self, hedge: HedgeSwap) -> Self {
        self.hedge_swaps.push(hedge);
        self
    }

    /// Attach explicit transaction fees.
    #[must_use]
    pub fn with_fees(mut self, fees: DealFees) -> Self {
        self.fees = Some(fees);
        self
    }

    /// Attach overcollateralization / interest-coverage tests to the deal.
    ///
    /// Each test names the tested class, its kind and level and what a
    /// failure does with the diverted interest; the waterfall places it right
    /// after the interest tier of its placement tranche, so a failure can only
    /// divert cash ranked below that position (see
    /// [`Self::coverage_triggers`]).
    ///
    /// # Arguments
    ///
    /// * `tests` - Coverage tests to evaluate each payment period.
    ///
    /// # Errors
    ///
    /// Returns `Err` when a level is not finite or not strictly positive, when
    /// a test names a tranche that is not part of this deal or an equity
    /// tranche, when two tests share a tranche and kind, or when an id is
    /// duplicated — a silently-ignored test would look like protection that
    /// is not there.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    /// #     CoverageTestSpec, StructuredCredit,
    /// # };
    /// # fn example(deal: StructuredCredit) -> finstack_quant_core::Result<()> {
    /// let deal = deal.with_coverage_triggers(vec![
    ///     CoverageTestSpec::oc("CLASS_B", 1.20),
    ///     CoverageTestSpec::ic("CLASS_B", 1.15),
    /// ])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_coverage_triggers(
        mut self,
        tests: Vec<CoverageTestSpec>,
    ) -> finstack_quant_core::Result<Self> {
        self.validate_coverage_tests(tests.iter())?;
        self.coverage_triggers = tests;
        Ok(self)
    }

    /// Validate a set of coverage tests against this deal's tranches: known,
    /// non-equity tranche ids (tested and placement), finite positive levels,
    /// unique ids and at most one test per `(tranche, kind)`.
    fn validate_coverage_tests<'a>(
        &self,
        tests: impl Iterator<Item = &'a CoverageTestSpec>,
    ) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let tranche = |id: &str| self.tranches.tranches.iter().find(|t| t.id.as_str() == id);
        let mut seen_ids: finstack_quant_core::HashSet<&str> =
            finstack_quant_core::HashSet::default();
        let mut seen_keys: finstack_quant_core::HashSet<(&str, super::CoverageTestType)> =
            finstack_quant_core::HashSet::default();
        for test in tests {
            for id in [test.tranche_id.as_str(), test.placement_tranche()] {
                match tranche(id) {
                    None => {
                        return Err(invalid(format!(
                            "coverage test '{}' references tranche '{id}', which is not \
                             part of deal '{}'",
                            test.id,
                            self.id.as_str()
                        )))
                    }
                    Some(t) if t.seniority == TrancheSeniority::Equity => {
                        return Err(invalid(format!(
                            "coverage test '{}' references equity tranche '{id}'; coverage \
                             tests apply to debt classes",
                            test.id
                        )))
                    }
                    Some(_) => {}
                }
            }
            if !test.trigger_level.is_finite() || test.trigger_level <= 0.0 {
                return Err(invalid(format!(
                    "coverage test '{}' level must be finite and positive, got {}",
                    test.id, test.trigger_level
                )));
            }
            if !seen_ids.insert(test.id.as_str()) {
                return Err(invalid(format!("duplicate coverage test id '{}'", test.id)));
            }
            if !seen_keys.insert((test.tranche_id.as_str(), test.kind)) {
                return Err(invalid(format!(
                    "duplicate {} coverage test for tranche '{}'",
                    test.kind.label(),
                    test.tranche_id
                )));
            }
        }
        Ok(())
    }

    /// Loss-allocation policy in force for pricing: the explicit
    /// [`Self::loss_allocation`] when set, else the deal-type market
    /// convention from [`LossAllocationPolicy::default_for`].
    pub fn effective_loss_allocation(&self) -> LossAllocationPolicy {
        self.loss_allocation
            .unwrap_or_else(|| LossAllocationPolicy::default_for(self.deal_type))
    }

    /// Whether the template waterfall pays senior fees and senior note
    /// interest from principal proceeds when interest proceeds fall short:
    /// the explicit [`Self::principal_covers_senior_interest`] when set, else
    /// `true` for CLO/CBO deals and `false` for every other deal type.
    pub fn effective_principal_covers_senior_interest(&self) -> bool {
        self.principal_covers_senior_interest
            .unwrap_or(matches!(self.deal_type, DealType::Clo | DealType::Cbo))
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
    /// deal-level [`Self::coverage_triggers`] are placed as coverage-test
    /// tiers after the interest tier of their placement tranche.
    pub fn create_waterfall(&self) -> finstack_quant_core::Result<Waterfall> {
        let mut waterfall = match self.waterfall.as_ref() {
            Some(custom) => custom.clone(),
            None => {
                // Senior transaction fees, paid ahead of every note.
                let mut template = Waterfall::standard_sequential(
                    self.pool.get_base_currency(),
                    &self.tranches,
                    self.template_fees()?,
                    &[],
                );
                if self.effective_principal_covers_senior_interest() {
                    template.fund_senior_interest_from_principal(&self.tranches);
                }
                template
            }
        };

        for test in &self.coverage_triggers {
            waterfall.insert_coverage_test(test.clone());
        }
        if waterfall.coverage_rules.is_none() {
            waterfall.coverage_rules = self.coverage_rules.clone();
        }
        Ok(waterfall)
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
    /// * `waterfall` - Replacement payment waterfall used verbatim by pricing; must
    ///   reference this deal's tranches and currency.
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

        // Coverage tests carried by the waterfall's test tiers plus the
        // deal-level tests (placed by `create_waterfall`) must resolve, carry
        // sane levels, and not double-test one (tranche, kind) — a duplicated
        // test would evaluate and cure the same ratio twice per period.
        self.validate_coverage_tests(
            waterfall
                .coverage_tests()
                .chain(self.coverage_triggers.iter()),
        )?;

        // A test tier pays nobody and must sit after the tier that pays its
        // tested tranche's interest: a test evaluated ahead of that coupon
        // would divert the coupon it is meant to protect. Every test in one
        // tier shares an action, since the tier diverts once.
        let mut ordered: Vec<&super::WaterfallTier> = waterfall.tiers.iter().collect();
        ordered.sort_by_key(|tier| tier.priority);
        for (position, tier) in ordered.iter().enumerate() {
            let is_test_tier = tier.payment_type == PaymentType::CoverageTest;
            if is_test_tier && !tier.recipients.is_empty() {
                return Err(invalid(format!(
                    "coverage-test tier '{}' must not carry recipients",
                    tier.id
                )));
            }
            if !is_test_tier && !tier.tests.is_empty() {
                return Err(invalid(format!(
                    "tier '{}' carries coverage tests but is not a coverage-test tier",
                    tier.id
                )));
            }
            if is_test_tier && tier.tests.is_empty() {
                return Err(invalid(format!(
                    "coverage-test tier '{}' must carry at least one test",
                    tier.id
                )));
            }
            if is_test_tier
                && tier
                    .tests
                    .iter()
                    .any(|test| test.action != tier.tests[0].action)
            {
                return Err(invalid(format!(
                    "coverage-test tier '{}' mixes actions; every test in one tier must \
                     share an action",
                    tier.id
                )));
            }
            for test in &tier.tests {
                let interest_position = ordered
                    .iter()
                    .position(|t| t.interest_tranche_ids().any(|id| id == test.tranche_id));
                if let Some(interest_position) = interest_position {
                    if interest_position > position {
                        return Err(invalid(format!(
                            "coverage test '{}' sits before the tier that pays tranche '{}' \
                             interest; place it after that tier",
                            test.id, test.tranche_id
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
