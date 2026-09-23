use super::{DealType, StructuredCredit, TrancheCoupon, TrancheSeniority};
use crate::cashflow::traits::{
    schedule_from_classified_flows, CashflowProvider, ScheduleBuildOpts,
};
use crate::impl_instrument_base;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error;

impl finstack_quant_cashflows::CashflowScheduleSource for StructuredCredit {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(self.pool.total_balance().ok())
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let detailed_flows =
            crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                self, context, as_of,
            )?
            .into_values()
            .flat_map(|result| result.detailed_flows.into_iter())
            .collect();
        // Use deal-type-appropriate day count convention:
        // CLO/CBO: ACT/360 (standard for leveraged loan market)
        // RMBS/CMBS: 30/360 (standard for mortgage market)
        // ABS/Auto/Card: ACT/360
        let day_count = match self.deal_type {
            DealType::Rmbs | DealType::Cmbs => finstack_quant_core::dates::DayCount::Thirty360,
            _ => finstack_quant_core::dates::DayCount::Act360,
        };
        Ok(schedule_from_classified_flows(
            detailed_flows,
            day_count,
            ScheduleBuildOpts {
                notional_hint: self.notional()?,
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        ))
    }
}

impl Instrument for StructuredCredit {
    impl_instrument_base!(crate::pricer::InstrumentType::StructuredCredit);

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());

        let pool = self.pool.normalized(self.closing_date)?;
        for index_id in pool
            .assets
            .iter()
            .filter_map(|asset| asset.index_id.as_deref())
        {
            deps.add_forward_curve(index_id);
            deps.add_series_id(fixing_series_id(index_id));
        }
        for tranche in &self.tranches.tranches {
            if let TrancheCoupon::Floating(spec) = &tranche.coupon {
                deps.add_forward_curve(spec.index_id.clone());
                deps.add_series_id(fixing_series_id(spec.index_id.as_str()));
            }
        }
        Ok(deps)
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate_resolvable()?;
        if let Some(threshold) = self.cleanup_call_pct {
            if !threshold.is_finite() || threshold <= 0.0 || threshold >= 1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "cleanup_call_pct must be finite and in (0, 1), got {threshold}"
                )));
            }
        }
        self.validate_custom_waterfall()?;
        self.pool.validate_original_balance()?;
        let base_currency = self.pool.get_base_currency();
        for hedge in &self.hedge_swaps {
            hedge.validate(base_currency, &self.tranches)?;
        }
        if let Some(rules) = &self.coverage_rules {
            rules.validate()?;
        }
        self.validate_delinquency()?;
        self.validate_cmbs_terms()?;
        self.validate_liquidation_terms()?;
        if let Some(call) = &self.call_assumption {
            call.validate(
                self.tranches
                    .tranches
                    .iter()
                    .filter(|tranche| tranche.seniority != TrancheSeniority::Equity)
                    .map(|tranche| tranche.id.as_str()),
            )?;
            if call.date < self.closing_date || call.date > self.maturity {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "call_assumption.date {} must lie between closing {} and maturity {}",
                    call.date, self.closing_date, self.maturity
                )));
            }
        }
        if let Some(price) = self.liquidation_price_pct {
            if !price.is_finite() || price <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "liquidation_price_pct ({price}) must be a finite positive percent of par"
                )));
            }
        }
        self.validate_tranche_draws()?;
        if let Some(card) = &self.credit_model.card {
            card.validate()?;
            if self.pool.instruments.is_some() {
                return Err(finstack_quant_core::Error::Validation(
                    "credit_model.card applies to asset and rep-line pools, not instrument collateral"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    fn base_value(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let disc = context.get_discount(self.discount_curve_id.as_str())?;
        let flows = self.dated_cashflows(context, as_of)?;

        flows.npv(disc.as_ref(), as_of)
    }

    fn seed_metric_context(
        &self,
        context: &mut crate::metrics::MetricContext,
        market: &MarketContext,
        as_of: Date,
    ) {
        context.discount_curve_id = Some(self.discount_curve_id.to_owned());
        // Deal-level prices are per CURRENT face: the sum of the current
        // tranche balances (debt and equity), the factor-adjusted quote basis.
        let current_face = self.tranches.tranches.iter().try_fold(
            Money::from((0_i64, self.tranches.total_size.currency())),
            |acc, tranche| acc.checked_add(tranche.current_balance),
        );
        context.notional = current_face.ok().or(Some(self.tranches.total_size));
        if let Ok(results) =
            crate::instruments::fixed_income::structured_credit::pricing::run_simulation(
                self, market, as_of,
            )
        {
            let mut flows = Vec::new();
            let mut accruals = Vec::new();
            for result in results.into_values() {
                flows.extend(result.detailed_flows);
                accruals.extend(result.accrual_periods);
            }
            flows.sort_by_key(|flow| flow.date);
            context.cashflows = Some(
                flows
                    .iter()
                    .filter(|flow| crate::cashflow::primitives::is_cash_settlement_kind(flow.kind))
                    .map(|flow| (flow.date, flow.amount))
                    .collect(),
            );
            context.tagged_cashflows = Some(flows);
            context.structured_credit_accruals = Some(accruals);
        }
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        let effective = self.effective_credit_model();
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: effective.prepayment_spec,
            default_spec: effective.default_spec,
            recovery_spec: effective.recovery_spec,
        }
    }

    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_quant_core::Result<Box<dyn Instrument>> {
        match params {
            ModelParamsSnapshot::StructuredCredit {
                prepayment_spec,
                default_spec,
                recovery_spec,
            } => {
                let mut modified = self.resolved_for_pricing()?;
                modified.credit_model.prepayment_spec = prepayment_spec.clone();
                modified.credit_model.default_spec = default_spec.clone();
                modified.credit_model.recovery_spec = recovery_spec.clone();
                Ok(Box::new(modified))
            }
            ModelParamsSnapshot::None => Ok(self.clone_box()),
            ModelParamsSnapshot::Convertible { .. } => Err(Error::Validation(
                "Instrument type mismatch: expected StructuredCredit model parameters".to_string(),
            )),
        }
    }

    crate::impl_focused_pricing_overrides!();
}

impl StructuredCredit {
    /// Commercial-mortgage terms on every asset must be well formed and the
    /// property NOI must be a finite amount in the asset's currency.
    fn validate_cmbs_terms(&self) -> finstack_quant_core::Result<()> {
        for asset in &self.pool.assets {
            let origination = asset.acquisition_date.unwrap_or(self.closing_date);
            let months_to_maturity = if origination < asset.maturity {
                origination.months_until(asset.maturity)
            } else {
                0
            };
            if let Some(term) = asset.amortization_term_months {
                if term == 0 || term < months_to_maturity {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "asset {} amortization_term_months ({term}) must be positive and at least the \
                         {months_to_maturity} months from origination to maturity",
                        asset.id
                    )));
                }
            }
            if let Some(io) = asset.io_months {
                if io > months_to_maturity
                    || asset
                        .amortization_term_months
                        .is_some_and(|term| io >= term)
                {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "asset {} io_months ({io}) must not exceed the {months_to_maturity} months to \
                         maturity or reach the amortization term",
                        asset.id
                    )));
                }
            }
            if let Some(balloon) = asset.balloon {
                balloon.validate()?;
            }
            if let Some(penalty) = &asset.prepayment_penalty {
                penalty.validate()?;
            }
            if let Some(special) = asset.special_servicing {
                special.validate()?;
            }
            if let Some(noi) = asset.noi {
                if noi.currency() != asset.balance.currency() || !noi.amount().is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "asset {} noi must be a finite amount in {}",
                        asset.id,
                        asset.balance.currency()
                    )));
                }
            }
        }
        Ok(())
    }

    /// Non-performing loan terms: a valid timeline on an asset row that is
    /// carried as performing (the timeline replaces the default flag and the
    /// recovery inputs), resolving inside the deal's life.
    fn validate_liquidation_terms(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        for asset in &self.pool.assets {
            let Some(spec) = asset.liquidation else {
                continue;
            };
            spec.validate()?;
            if self.pool.instruments.is_some() {
                return Err(invalid(format!(
                    "asset {}: liquidation terms apply to asset rows, not instrument collateral",
                    asset.id
                )));
            }
            if asset.is_defaulted || asset.recovery_amount.is_some() || asset.default_date.is_some()
            {
                return Err(invalid(format!(
                    "asset {} carries liquidation terms: leave is_defaulted false and \
                     recovery_amount/default_date unset (the resolution timeline books the default)",
                    asset.id
                )));
            }
            let months = i32::try_from(spec.months_to_resolution).map_err(|_| {
                invalid(format!(
                    "asset {}: months_to_resolution is too large",
                    asset.id
                ))
            })?;
            // The timeline runs from the loan's origination (acquisition
            // date, else closing).
            let anchor = asset.acquisition_date.unwrap_or(self.closing_date);
            if anchor.add_months(months) > self.maturity {
                return Err(invalid(format!(
                    "asset {} resolves {} months after its origination {anchor}, past the deal \
                     maturity {}",
                    asset.id, spec.months_to_resolution, self.maturity
                )));
            }
        }
        Ok(())
    }

    /// Scheduled draws and re-advances name debt notes, carry positive
    /// deal-currency amounts, fall inside the deal's life and are ascending
    /// by date.
    fn validate_tranche_draws(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let currency = self.pool.get_base_currency();
        let debt_note = |id: &str| {
            self.tranches
                .tranches
                .iter()
                .find(|t| t.id.as_str() == id)
                .filter(|t| t.seniority != TrancheSeniority::Equity)
        };
        let mut previous: Option<Date> = None;
        for draw in &self.tranche_draws {
            if debt_note(&draw.tranche_id).is_none() {
                return Err(invalid(format!(
                    "tranche_draws names unknown or equity tranche '{}'",
                    draw.tranche_id
                )));
            }
            if draw.amount.currency() != currency || draw.amount.amount() <= 0.0 {
                return Err(invalid(format!(
                    "tranche_draws amount for '{}' must be a positive {currency} amount, got {}",
                    draw.tranche_id, draw.amount
                )));
            }
            if draw.date <= self.closing_date || draw.date > self.maturity {
                return Err(invalid(format!(
                    "tranche_draws date {} must lie inside (closing, maturity]",
                    draw.date
                )));
            }
            if previous.is_some_and(|prev| draw.date < prev) {
                return Err(invalid(
                    "tranche_draws must be ascending by date".to_string(),
                ));
            }
            previous = Some(draw.date);
        }
        if let Some(readvance) = &self.tranche_readvance {
            let Some(note) = debt_note(&readvance.tranche_id) else {
                return Err(invalid(format!(
                    "tranche_readvance names unknown or equity tranche '{}'",
                    readvance.tranche_id
                )));
            };
            if readvance.commitment.currency() != currency
                || readvance.commitment.amount() < note.current_balance.amount()
            {
                return Err(invalid(format!(
                    "tranche_readvance commitment {} must be a {currency} amount at least the \
                     note's balance {}",
                    readvance.commitment, note.current_balance
                )));
            }
            if self
                .coverage_rules
                .as_ref()
                .and_then(|r| r.borrowing_base.as_ref())
                .is_none()
            {
                return Err(invalid(
                    "tranche_readvance requires coverage_rules.borrowing_base".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// The delinquency model and every asset's seeded buckets must agree.
    fn validate_delinquency(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let model = self.credit_model.delinquency.as_ref();
        if let Some(model) = model {
            model.validate()?;
            if self.pool.instruments.is_some() {
                return Err(invalid(
                    "credit_model.delinquency applies to asset and rep-line pools, not instrument collateral"
                        .to_string(),
                ));
            }
            // A charge-off curve is inverted through the roll rates to size
            // the bucket entries; a bucket that never rolls cannot be.
            let charge_off_curve = matches!(
                self.credit_model.default_spec.curve,
                Some(crate::cashflow::builder::DefaultCurve::CumulativeLoss { .. })
                    | Some(crate::cashflow::builder::DefaultCurve::Timing { .. })
            );
            if charge_off_curve && model.roll_rates.iter().any(|roll| *roll <= 0.0) {
                return Err(invalid(
                    "credit_model.delinquency: every roll_rate must be positive when the default \
                     model is a cumulative-loss or timing curve (the curve's charge-offs are \
                     inverted through the roll rates to size the delinquency entries)"
                        .to_string(),
                ));
            }
        }
        for asset in &self.pool.assets {
            let Some(buckets) = &asset.delinquency_buckets else {
                continue;
            };
            let Some(model) = model else {
                return Err(invalid(format!(
                    "asset {} carries delinquency_buckets but the deal has no credit_model.delinquency",
                    asset.id
                )));
            };
            if buckets.len() != model.buckets() {
                return Err(invalid(format!(
                    "asset {} has {} delinquency buckets but the model defines {}",
                    asset.id,
                    buckets.len(),
                    model.buckets()
                )));
            }
            let mut total = 0.0;
            for bucket in buckets {
                if bucket.currency() != asset.balance.currency() || bucket.amount() < 0.0 {
                    return Err(invalid(format!(
                        "asset {} delinquency buckets must be non-negative amounts in {}",
                        asset.id,
                        asset.balance.currency()
                    )));
                }
                total += bucket.amount();
            }
            if total > asset.balance.amount() * (1.0 + 1e-9) {
                return Err(invalid(format!(
                    "asset {} delinquent balance {total} exceeds its balance {}",
                    asset.id,
                    asset.balance.amount()
                )));
            }
        }
        Ok(())
    }
}
