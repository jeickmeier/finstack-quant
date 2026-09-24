//! Constructing waterfalls: [`Waterfall::builder`], the standard sequential
//! template and the placement of coverage tests and hedge payments.

use super::*;

impl Waterfall {
    /// Start building a waterfall in `base_currency`; tiers are added with
    /// [`WaterfallBuilder::add_tier`] and [`WaterfallBuilder::build`]
    /// validates the result.
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Currency every tier allocates in.
    #[must_use]
    pub fn builder(base_currency: Currency) -> WaterfallBuilder {
        WaterfallBuilder {
            engine: Self {
                tiers: Vec::new(),
                base_currency,
                coverage_rules: None,
            },
            next_priority: 1,
        }
    }

    /// Every coverage test in the waterfall, in tier priority order.
    pub fn coverage_tests(&self) -> impl Iterator<Item = &CoverageTestSpec> {
        let mut tiers: Vec<&WaterfallTier> = self.tiers.iter().collect();
        tiers.sort_by_key(|tier| tier.priority);
        tiers
            .into_iter()
            .filter(|tier| tier.payment_type == PaymentType::CoverageTest)
            .flat_map(|tier| tier.tests.iter())
    }

    /// Insert `test` as a coverage-test position immediately after the tier
    /// that pays `spec.placement_tranche()`'s interest (or after the last
    /// interest tier when that tranche has no interest recipient), joining an
    /// existing test tier at that position whose tests share the test's
    /// `action` and `divert_pct`, else adding a new test tier after those
    /// already there.
    /// Priorities are renumbered `1..=n` in order.
    ///
    /// # Arguments
    ///
    /// * `test` - Coverage test to place.
    pub fn insert_coverage_test(&mut self, test: CoverageTestSpec) {
        self.tiers.sort_by_key(|tier| tier.priority);
        let (placement, anchor) = match test.placement_tranche() {
            Some(tranche_id) => (
                tranche_id.to_string(),
                self.tiers
                    .iter()
                    .position(|tier| tier.interest_tranche_ids().any(|id| id == tranche_id)),
            ),
            // After the junior fees: the last fee tier that follows the
            // coupons (the template's `junior_fees`), else the last coupon.
            None => (
                "junior_fees".to_string(),
                self.tiers
                    .iter()
                    .rposition(|tier| tier.payment_type == PaymentType::Interest)
                    .map(|last_interest| {
                        self.tiers
                            .iter()
                            .enumerate()
                            .skip(last_interest + 1)
                            .take_while(|(_, tier)| {
                                matches!(
                                    tier.payment_type,
                                    PaymentType::Fee | PaymentType::CoverageTest
                                )
                            })
                            .filter(|(_, tier)| tier.payment_type == PaymentType::Fee)
                            .map(|(index, _)| index)
                            .last()
                            .unwrap_or(last_interest)
                    }),
            ),
        };
        let anchor = anchor.or_else(|| {
            self.tiers
                .iter()
                .rposition(|tier| tier.payment_type == PaymentType::Interest)
        });
        // The run of test tiers already at this position: join the one whose
        // tests share this test's action and diversion cap (a tier diverts
        // once, under one action and one cap), else append a new tier after
        // the run so tests keep their insertion order.
        let run_start = anchor.map_or(0, |i| i + 1);
        let run_end = run_start
            + self.tiers[run_start..]
                .iter()
                .take_while(|tier| tier.payment_type == PaymentType::CoverageTest)
                .count();
        let joinable = self.tiers[run_start..run_end].iter_mut().find(|tier| {
            tier.tests.first().is_none_or(|first| {
                first.action == test.action && first.divert_pct == test.divert_pct
            })
        });
        match joinable {
            Some(existing) => existing.tests.push(test),
            None => {
                let id = if run_end == run_start {
                    format!("{placement}_coverage")
                } else {
                    format!("{placement}_coverage_{}", run_end - run_start + 1)
                };
                let tier = WaterfallTier::coverage_tests(id, 0, vec![test]);
                self.tiers.insert(run_end, tier);
            }
        }
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            tier.priority = index + 1;
        }
    }

    /// Create a standard sequential waterfall for a given tranche structure.
    ///
    /// Shared skeleton:
    /// 1. Fees tier (sequential)
    /// 2. One interest tier per note class in payment-priority order, each
    ///    followed by the coverage tests placed on that class
    /// 3. Principal (sequential, by priority)
    /// 4. Equity residual
    ///
    /// Paying interest class by class (INTEX/Bloomberg ordering) lets a
    /// coverage test sit after any class, so a failing Class D test can only
    /// trap the interest ranked below Class D, and lets
    /// [`Self::fund_senior_interest_from_principal`] single out the senior
    /// coupons. Every tier draws on its default [`FundingSource`].
    ///
    /// # Arguments
    ///
    /// * `base_currency` - Deal currency of the waterfall; tranche balances and fees must
    ///   match this currency.
    /// * `tranches` - Capital structure whose notes become sequential interest and
    ///   principal recipients.
    /// * `fees` - Senior fee recipients (a tier ahead of every note), junior
    ///   fee recipients (a tier after every note coupon, ahead of principal)
    ///   the incentive-fee recipient (ahead of `equity_principal` in the
    ///   principal tier and a tier ahead of the residual) and the net-WAC
    ///   carryover recipients (a tier after principal); empty lists omit
    ///   their tiers.
    /// * `coverage_tests` - Deal-level coverage tests, each placed after the
    ///   interest tier of its [`CoverageTestSpec::placement_tranche`].
    pub fn standard_sequential(
        base_currency: Currency,
        tranches: &super::super::TrancheStructure,
        fees: TemplateFees,
        coverage_tests: &[CoverageTestSpec],
    ) -> Self {
        let mut engine = Self {
            tiers: Vec::new(),
            base_currency,
            coverage_rules: None,
        };
        let mut priority = 1;

        if !fees.senior.is_empty() {
            let fees_tier = WaterfallTier::new("fees", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential);
            let fees_tier = fees
                .senior
                .into_iter()
                .fold(fees_tier, |tier, recipient| tier.add_recipient(recipient));
            engine.tiers.push(fees_tier);
            priority += 1;
        }

        let mut sorted_tranches = tranches.tranches.clone();
        sorted_tranches.sort_by_key(|t| t.payment_priority);

        // One interest tier per class in payment-priority order, so a
        // coverage-test position can sit after any class and a funding source
        // can single out the senior coupons. Sequential allocation makes the
        // split an identity when nothing sits between the tiers.
        for tranche in &sorted_tranches {
            if tranche.seniority == super::super::TrancheSeniority::Equity {
                continue;
            }
            let tier = WaterfallTier::new(
                format!("{}_interest", tranche.id.as_str()),
                priority,
                PaymentType::Interest,
            )
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::tranche_interest(
                format!("{}_interest", tranche.id.as_str()),
                tranche.id.as_str(),
            ));
            engine.tiers.push(tier);
            priority += 1;
        }

        // Junior fees (the subordinated management fee) rank after every note
        // coupon and ahead of principal.
        if !fees.junior.is_empty() {
            let junior_tier = WaterfallTier::new("junior_fees", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential);
            let junior_tier = fees
                .junior
                .into_iter()
                .fold(junior_tier, |tier, recipient| tier.add_recipient(recipient));
            engine.tiers.push(junior_tier);
            priority += 1;
        }

        let mut principal_recipients = Vec::new();
        for tranche in &sorted_tranches {
            if tranche.seniority != super::super::TrancheSeniority::Equity {
                principal_recipients.push(Recipient::tranche_principal(
                    format!("{}_principal", tranche.id.as_str()),
                    tranche.id.as_str(),
                    None,
                ));
            }
        }

        // The manager's incentive fee shares in principal proceeds reaching
        // equity above the hurdle, ahead of the equity principal recipient.
        if let Some(incentive) = fees.incentive.as_ref() {
            let mut principal_incentive = incentive.clone();
            principal_incentive.id = "incentive_fee_principal".to_string();
            principal_recipients.push(principal_incentive);
        }
        principal_recipients.push(Recipient::new(
            "equity_principal",
            RecipientType::Equity,
            PaymentCalculation::ResidualCash,
        ));
        let principal_tier = WaterfallTier::new("principal", priority, PaymentType::Principal)
            .allocation_mode(AllocationMode::Sequential);
        let principal_tier = principal_recipients
            .into_iter()
            .fold(principal_tier, |tier, recipient| {
                tier.add_recipient(recipient)
            });
        engine.tiers.push(principal_tier);
        priority += 1;

        // Net-WAC carryover repayments come out of excess interest ahead of
        // the incentive fee and the residual.
        if !fees.carryover.is_empty() {
            let carryover_tier =
                WaterfallTier::new("net_wac_carryover", priority, PaymentType::Fee)
                    .allocation_mode(AllocationMode::Sequential);
            let carryover_tier = fees
                .carryover
                .into_iter()
                .fold(carryover_tier, |tier, recipient| {
                    tier.add_recipient(recipient)
                });
            engine.tiers.push(carryover_tier);
            priority += 1;
        }

        // The manager's incentive fee takes its share of the residual ahead
        // of equity once the equity IRR hurdle is met.
        if let Some(incentive) = fees.incentive {
            let incentive_tier = WaterfallTier::new("incentive_fee", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(incentive);
            engine.tiers.push(incentive_tier);
            priority += 1;
        }

        // Residual interest reaches equity only after every coverage-test
        // position above has been satisfied.
        let equity_tier = WaterfallTier::new("equity", priority, PaymentType::Residual)
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::new(
                "equity_distribution",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            ));
        engine.tiers.push(equity_tier);

        for test in coverage_tests {
            engine.insert_coverage_test(test.clone());
        }

        engine
    }

    /// Add a hedge-swap counterparty payment at the fee position `priority`.
    ///
    /// `SeniorFee` joins the leading fee tier, or opens one ahead of every
    /// other tier; `JuniorFee` joins the fee tier after the last note coupon
    /// (the junior fee tier), or opens one there ahead of principal.
    /// Priorities are renumbered `1..=n` in order.
    ///
    /// # Arguments
    ///
    /// * `recipient` - Fixed-amount payment to the swap counterparty for the
    ///   period.
    /// * `priority` - Fee position the payment ranks in.
    pub fn insert_hedge_payment(
        &mut self,
        recipient: Recipient,
        priority: super::super::SwapPriority,
    ) {
        self.tiers.sort_by_key(|tier| tier.priority);
        match priority {
            super::super::SwapPriority::SeniorFee => match self.tiers.first_mut() {
                Some(first) if first.payment_type == PaymentType::Fee => {
                    first.recipients.push(recipient);
                }
                _ => {
                    let tier = WaterfallTier::new("hedge_fees", 0, PaymentType::Fee)
                        .add_recipient(recipient);
                    self.tiers.insert(0, tier);
                }
            },
            super::super::SwapPriority::JuniorFee => {
                // After the last note coupon (and the coverage tests placed on
                // it), ahead of principal and the residual.
                let last_interest = self
                    .tiers
                    .iter()
                    .rposition(|tier| tier.payment_type == PaymentType::Interest);
                let anchor = self
                    .tiers
                    .iter()
                    .enumerate()
                    .position(|(index, tier)| {
                        last_interest.is_none_or(|last| index > last)
                            && matches!(
                                tier.payment_type,
                                PaymentType::Principal | PaymentType::Residual
                            )
                    })
                    .unwrap_or(self.tiers.len());
                let joins_junior_fee_tier = anchor > 0
                    && self.tiers[anchor - 1].payment_type == PaymentType::Fee
                    && last_interest.is_some_and(|last| anchor - 1 > last);
                if joins_junior_fee_tier {
                    self.tiers[anchor - 1].recipients.push(recipient);
                } else {
                    let tier = WaterfallTier::new("junior_hedge_fees", 0, PaymentType::Fee)
                        .add_recipient(recipient);
                    self.tiers.insert(anchor, tier);
                }
            }
        }
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            tier.priority = index + 1;
        }
    }

    /// Let senior fees and senior note interest top up from principal
    /// proceeds (the CLO principal-waterfall convention).
    ///
    /// Sets [`FundingSource::InterestThenPrincipal`] on every `Fee` tier ranked
    /// ahead of the first interest tier and on every `Interest` tier whose
    /// interest recipients are all non-deferrable claims of `tranches`
    /// ([`Tranche::is_non_deferrable`](super::super::Tranche::is_non_deferrable):
    /// senior notes unless a class says otherwise). Tiers paying any
    /// deferrable coupon are left on interest proceeds.
    ///
    /// # Arguments
    ///
    /// * `tranches` - Capital structure used to classify each interest
    ///   recipient's seniority.
    pub fn fund_senior_interest_from_principal(
        &mut self,
        tranches: &super::super::TrancheStructure,
    ) {
        let is_senior = |id: &str| {
            tranches
                .tranches
                .iter()
                .any(|t| t.id.as_str() == id && t.is_non_deferrable())
        };
        let first_interest = self
            .tiers
            .iter()
            .position(|tier| tier.payment_type == PaymentType::Interest)
            .unwrap_or(self.tiers.len());
        for (index, tier) in self.tiers.iter_mut().enumerate() {
            let senior_only = match tier.payment_type {
                // Only fees ranked ahead of the notes; junior and incentive
                // fee tiers stay on interest proceeds.
                PaymentType::Fee => index < first_interest,
                PaymentType::Interest => {
                    let mut ids = tier.interest_tranche_ids().peekable();
                    ids.peek().is_some() && ids.all(is_senior)
                }
                _ => false,
            };
            if senior_only {
                tier.funding = Some(FundingSource::InterestThenPrincipal);
            }
        }
    }
}

/// Builder for a [`Waterfall`], started by [`Waterfall::builder`].
pub struct WaterfallBuilder {
    engine: Waterfall,
    next_priority: usize,
}

impl WaterfallBuilder {
    /// Add a tier; a tier with priority `0` takes the next free priority.
    /// Tiers are kept in priority order.
    ///
    /// # Arguments
    ///
    /// * `tier` - Tier to append.
    #[must_use]
    pub fn add_tier(mut self, mut tier: WaterfallTier) -> Self {
        if tier.priority == 0 {
            tier.priority = self.next_priority;
            self.next_priority += 1;
        }
        self.engine.tiers.push(tier);
        self.engine.tiers.sort_by_key(|t| t.priority);
        self
    }

    /// Attach collateral valuation rules for the OC tests (rating haircuts,
    /// defaulted-asset valuation, CCC bucket and discount-obligation rules).
    ///
    /// # Arguments
    ///
    /// * `rules` - Coverage rules applied by every OC test.
    #[must_use]
    pub fn coverage_rules(mut self, rules: CoverageRules) -> Self {
        self.engine.coverage_rules = Some(rules);
        self
    }

    /// Validate and return the waterfall.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when two tiers share a priority or
    /// [`crate::instruments::fixed_income::structured_credit::validate_tiers`]
    /// reports a structural error (duplicate ids, empty tiers, invalid
    /// weights or non-finite payment parameters).
    pub fn build(self) -> finstack_quant_core::Result<Waterfall> {
        let tiers = &self.engine.tiers;
        if let Some(pair) = tiers
            .windows(2)
            .find(|pair| pair[0].priority == pair[1].priority)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "waterfall tiers '{}' and '{}' have duplicate priority {}",
                pair[0].id, pair[1].id, pair[0].priority
            )));
        }
        let errors =
            crate::instruments::fixed_income::structured_credit::utils::validate_tiers(tiers);
        if let Some(first) = errors.first() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "waterfall is structurally invalid ({} error(s); first: {first})",
                errors.len()
            )));
        }
        Ok(self.engine)
    }
}
