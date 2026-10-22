use super::{
    DealAccruals, StructuredCredit, TrancheAccrualPeriod, TrancheCashflows, TrancheSeniority,
};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_cashflows::{
    schedule_from_classified_flows, CashflowProvider, ScheduleBuildOpts,
};
use finstack_quant_core::cashflow::CashFlow;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::money::Money;
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricSeed};

impl CashflowScheduleSource for StructuredCredit {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(self.pool.total_balance().ok())
    }

    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let detailed_flows = self
            .run_simulation(context, as_of)?
            .tranches
            .into_values()
            .flat_map(|result| result.detailed_flows.into_iter())
            .collect();
        self.projected_schedule(detailed_flows)
    }
}

impl StructuredCredit {
    /// Projected deal schedule from the detailed flows of one deterministic
    /// simulation run, in tranche iteration order.
    fn projected_schedule(
        &self,
        detailed_flows: Vec<CashFlow>,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        schedule_from_classified_flows(
            detailed_flows,
            self.deal_type.schedule_day_count(),
            ScheduleBuildOpts {
                notional_hint: CashflowScheduleSource::notional(self)?,
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        )
    }

    /// Deal NPV from one deterministic simulation run, plus that run's flows
    /// for metric evaluation.
    ///
    /// The value is the one [`Instrument::base_value`] computes (the same
    /// discounting of the same public schedule), so a registry valuation that
    /// prices through here and seeds its metric context with the returned
    /// [`SimulationMetricSeed`] runs the waterfall once.
    pub(crate) fn price_with_seed(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, SimulationMetricSeed)> {
        let disc = market.get_discount(self.discount_curve_id.as_str())?;
        let run = self.run_simulation(market, as_of)?.tranches;
        let flows = RunFlows {
            deal: self,
            run: &run,
        }
        .dated_cashflows(market, as_of)?;
        let value = finstack_quant_core::cashflow::npv(disc.as_ref(), as_of, &flows)?;
        Ok((value, SimulationMetricSeed::from_run(run)))
    }
}

/// One simulation run as a schedule source, so its flows go through the same
/// public normalization as [`StructuredCredit`]'s own schedule.
struct RunFlows<'a> {
    deal: &'a StructuredCredit,
    run: &'a FxHashMap<String, TrancheCashflows>,
}

impl CashflowScheduleSource for RunFlows<'_> {
    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let detailed_flows = self
            .run
            .values()
            .flat_map(|result| result.detailed_flows.iter().cloned())
            .collect();
        self.deal.projected_schedule(detailed_flows)
    }
}

/// Deal flows and accrual records of one deterministic simulation run, as the
/// metric context reads them.
#[derive(Clone)]
pub(crate) struct SimulationMetricSeed {
    cashflows: Vec<(Date, Money)>,
    tagged_cashflows: Vec<CashFlow>,
    accruals: Vec<TrancheAccrualPeriod>,
}

impl SimulationMetricSeed {
    fn from_run(run: FxHashMap<String, TrancheCashflows>) -> Self {
        let mut flows = Vec::new();
        let mut accruals = Vec::new();
        for result in run.into_values() {
            flows.extend(result.detailed_flows);
            accruals.extend(result.accrual_periods);
        }
        flows.sort_by_key(|flow| flow.date);
        Self {
            cashflows: flows
                .iter()
                .filter(|flow| flow.kind.is_cash_settlement())
                .map(|flow| (flow.date, flow.amount))
                .collect(),
            tagged_cashflows: flows,
            accruals,
        }
    }

    fn install(self, context: &mut MetricContext) {
        context.cashflows = Some(self.cashflows);
        context.tagged_cashflows = Some(self.tagged_cashflows);
        context.set_ext(DealAccruals(self.accruals));
    }
}

impl std::fmt::Debug for SimulationMetricSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimulationMetricSeed")
            .field("cashflows", &self.cashflows.len())
            .field("tagged_cashflows", &self.tagged_cashflows.len())
            .field("accruals", &self.accruals.len())
            .finish()
    }
}

impl MetricSeed for SimulationMetricSeed {
    fn seed(&self, context: &mut MetricContext) {
        self.clone().install(context);
    }
}

impl Instrument for StructuredCredit {
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::StructuredCredit,
        crate::STRUCTURED_CREDIT
    );

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());

        let pool = self.pool.normalized(self.closing_date)?;
        for forward_curve_id in pool
            .assets
            .iter()
            .filter_map(|asset| asset.forward_curve_id.as_deref())
        {
            deps.add_forward_curve(forward_curve_id);
            deps.add_series_id(fixing_series_id(forward_curve_id));
        }
        for tranche in &self.tranches.tranches {
            if let RateSpec::Floating(spec) = &tranche.coupon {
                deps.add_forward_curve(spec.forward_curve_id.clone());
                deps.add_series_id(fixing_series_id(spec.forward_curve_id.as_str()));
            }
        }
        Ok(deps)
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate_resolvable()?;
        if let Some(threshold) = self.cleanup_call_decimal {
            if !threshold.is_finite() || threshold <= 0.0 || threshold >= 1.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "cleanup_call_decimal must be finite and in (0, 1), got {threshold}"
                )));
            }
        }
        self.validate_custom_waterfall()?;
        // A custom waterfall validates the deal-level triggers together with
        // its own test tiers; the template path validates them here.
        if self.waterfall.is_none() {
            self.validate_coverage_tests(self.coverage_triggers.iter())?;
        }
        self.pool.validate_original_balance()?;
        let currency = self.pool.get_currency();
        for hedge in &self.hedge_swaps {
            hedge.validate(currency, &self.tranches)?;
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

        finstack_quant_core::cashflow::npv(disc.as_ref(), as_of, &flows)
    }

    fn seed_metric_context(
        &self,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
        market: &MarketContext,
        as_of: Date,
    ) {
        context.discount_curve_id = Some(self.discount_curve_id.to_owned());
        // Deal-level prices are per CURRENT face: the sum of the current
        // tranche balances (debt and equity), the factor-adjusted quote basis.
        let current_face = self.tranches.tranches.iter().try_fold(
            Money::from((0_i64, self.tranches.currency())),
            |acc, tranche| acc.checked_add(tranche.current_balance),
        );
        context.notional = current_face.ok().or(self.tranches.total_size().ok());
        // The discounting pricer seeds the flows of the run it priced; any
        // other path runs the waterfall here.
        if context.tagged_cashflows.is_some() {
            return;
        }
        if let Ok(results) = self.run_simulation(market, as_of).map(|run| run.tranches) {
            SimulationMetricSeed::from_run(results).install(context);
        }
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        ModelParamsSnapshot::StructuredCredit {
            prepayment_spec: self.credit_model.prepayment_spec.clone(),
            default_spec: self.credit_model.default_spec.clone(),
            recovery_spec: self.credit_model.recovery_spec.clone(),
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

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
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
            if asset.defaulted || asset.recovery_amount.is_some() || asset.default_date.is_some() {
                return Err(invalid(format!(
                    "asset {} carries liquidation terms: leave defaulted false and \
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
            if anchor.add_months(months)? > self.maturity {
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
        let currency = self.pool.get_currency();
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
                self.credit_model.default_spec,
                finstack_quant_cashflows::builder::DefaultModelSpec::CumulativeLoss { .. }
                    | finstack_quant_cashflows::builder::DefaultModelSpec::Timing { .. }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structured_credit::pricing::SIMULATION_RUNS;
    use finstack_quant_valuations_core::instruments::common_impl::helpers::compute_metrics_dyn;
    use finstack_quant_valuations_core::instruments::PricingOptions;
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_test_support::discount_forward_curves::{
        flat_discount_with_tenor, flat_forward_with_tenor,
    };
    use std::sync::Arc;

    fn simulation_runs() -> usize {
        SIMULATION_RUNS.with(std::cell::Cell::get)
    }

    #[test]
    fn price_with_metrics_runs_the_waterfall_once() {
        let deal = StructuredCredit::example().expect("example deal");
        let as_of = deal.closing_date;
        let dependencies = deal.market_dependencies().expect("dependencies");
        let mut market = MarketContext::new();
        for id in &dependencies.curves.discount_curves {
            market = market.insert(flat_discount_with_tenor(id.as_str(), as_of, 0.04, 30.0));
        }
        for id in &dependencies.curves.forward_curves {
            market = market.insert(flat_forward_with_tenor(id.as_str(), as_of, 0.04, 0.25));
        }
        let metrics = [MetricId::WAL];

        let before = simulation_runs();
        let priced = deal
            .price_with_metrics(&market, as_of, &metrics, PricingOptions::default())
            .expect("price with metrics");
        assert_eq!(
            simulation_runs() - before,
            1,
            "one deterministic waterfall run per price_with_metrics"
        );

        // The seeded value and metrics match the unseeded paths bit for bit.
        assert_eq!(
            priced.value.amount().to_bits(),
            deal.value(&market, as_of)
                .expect("value")
                .amount()
                .to_bits()
        );
        let unseeded = compute_metrics_dyn(
            Arc::new(deal),
            Arc::new(market),
            as_of,
            priced.value,
            &metrics,
            Default::default(),
        )
        .expect("unseeded metrics");
        assert_eq!(
            priced.metric(MetricId::WAL).map(f64::to_bits),
            unseeded.get(&MetricId::WAL).copied().map(f64::to_bits)
        );
    }
}
