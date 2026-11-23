//! One selected note with its own identity and its originating deal.

use super::StructuredCredit;
use super::TrancheCashflows;
use finstack_quant_cashflows::{ScheduleBuildOpts, schedule_from_classified_flows};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, InstrumentId};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::credit::pool::CorrelationStructure;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricSeed};
use serde::{Deserialize, Serialize};

/// An independently held note priced through its originating deal's waterfall.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StructuredCreditTranche {
    id: InstrumentId,
    deal: StructuredCredit,
    tranche_id: String,
}

impl StructuredCreditTranche {
    /// Create a position instrument for an explicit note drawn from the deal.
    ///
    /// # Arguments
    /// * `id` - Stable standalone instrument identifier for portfolio positions and metrics.
    /// * `deal` - Originating deal with the note, collateral assumptions and waterfall.
    /// * `tranche_id` - Exact identifier of a note issued by this deal.
    ///
    /// # Errors
    /// Rejects a blank position instrument ID, an invalid deal, or a missing note.
    pub fn new(
        id: InstrumentId,
        deal: StructuredCredit,
        tranche_id: impl Into<String>,
    ) -> Result<Self> {
        let note = Self {
            id,
            deal,
            tranche_id: tranche_id.into(),
        };
        note.validate_invariants()?;
        Ok(note)
    }

    /// Create a note with a stable identifier derived from the deal and note IDs.
    ///
    /// # Arguments
    /// * `deal` - Originating deal with its unmodified waterfall and collateral.
    /// * `tranche_id` - Exact identifier of one note issued by the deal.
    ///
    /// # Errors
    /// Rejects invalid deals and unknown notes.
    pub fn from_deal(deal: StructuredCredit, tranche_id: &str) -> Result<Self> {
        Self::new(
            InstrumentId::new(format!("{}::{tranche_id}", deal.id)),
            deal,
            tranche_id,
        )
    }

    /// Stable standalone instrument identifier.
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Construct the first note of the canonical example structured-credit deal.
    pub fn example() -> Result<Self> {
        let deal = StructuredCredit::example()?;
        let tranche_id = deal
            .tranches
            .tranches
            .first()
            .ok_or_else(|| {
                Error::Validation("Structured-credit example has no issued note".into())
            })?
            .id
            .to_string();
        Self::new(
            InstrumentId::new("STRUCTURED-CREDIT-NOTE-EXAMPLE"),
            deal,
            tranche_id,
        )
    }

    /// Contractual identifier of the selected note within the originating deal.
    pub fn tranche_id(&self) -> &str {
        &self.tranche_id
    }

    /// Originating collateral and waterfall for the selected note.
    pub fn deal(&self) -> &StructuredCredit {
        &self.deal
    }

    /// Clone this note with a replacement originating deal, preserving its identity and tranche.
    ///
    /// # Arguments
    /// * `deal` - Replacement collateral pool and waterfall deal containing `self.tranche_id()`.
    pub fn with_deal(&self, deal: StructuredCredit) -> Result<Self> {
        Self::new(self.id.clone(), deal, self.tranche_id.clone())
    }

    /// Apply a collateral correlation change to the note's originating deal.
    /// The note identifier, tranche selection and outstanding balance are preserved.
    ///
    /// # Arguments
    /// * `correlation` - Replacement calibrated collateral correlation assumptions.
    pub fn set_correlation(&mut self, correlation: CorrelationStructure) {
        self.deal.credit_model.correlation_structure = Some(correlation);
    }

    pub(crate) fn pricing_inputs(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(Money, TrancheMetricSeed)> {
        let flows = self
            .deal
            .tranche_cashflows(&self.tranche_id, market, as_of)?;
        let value = self.deal.value_tranche_cashflows_raw(
            &flows,
            market,
            self.deal.resolve_pricing_as_of(market, as_of),
        )?;
        Ok((value, TrancheMetricSeed(flows)))
    }
}

#[derive(Debug)]
pub(crate) struct TrancheMetricSeed(pub(crate) TrancheCashflows);

impl MetricSeed for TrancheMetricSeed {
    fn seed(&self, context: &mut MetricContext) {
        context.cashflows = Some(self.0.cashflows.clone());
        context.tagged_cashflows = Some(self.0.detailed_flows.clone());
        context.set_ext(self.0.clone());
    }
}

impl finstack_quant_cashflows::CashflowScheduleSource for StructuredCreditTranche {
    fn notional(&self) -> finstack_quant_core::Result<Option<Money>> {
        Ok(self
            .deal
            .tranches
            .tranches
            .iter()
            .find(|tranche| tranche.id.as_str() == self.tranche_id)
            .map(|tranche| tranche.current_balance))
    }

    /// The note's projected interest, principal and write-down flows.
    fn raw_cashflow_schedule(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let flows = (self.deal)
            .tranche_cashflows(&self.tranche_id, context, as_of)?
            .detailed_flows;
        Ok(schedule_from_classified_flows(
            flows,
            self.deal.deal_type.schedule_day_count(),
            ScheduleBuildOpts {
                notional_hint: finstack_quant_cashflows::CashflowScheduleSource::notional(self)?,
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        ))
    }
}

impl Instrument for StructuredCreditTranche {
    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn key(&self) -> finstack_quant_valuations_core::pricer::InstrumentType {
        finstack_quant_valuations_core::pricer::InstrumentType::StructuredCreditTranche
    }

    fn family(&self) -> &'static finstack_quant_valuations_core::pricer::Family {
        &crate::STRUCTURED_CREDIT
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        self.deal.attributes()
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        self.deal.attributes_mut()
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        self.deal.market_dependencies()
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        if self.id.as_str().trim().is_empty() {
            return Err(Error::Validation(
                "Structured-credit note ID must be nonempty".into(),
            ));
        }
        self.deal.tranches.require(&self.tranche_id)?;
        self.deal.validate_invariants()
    }

    /// The note's PV: its projected waterfall cashflows discounted from
    /// `as_of` on the deal's discount curve.
    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.pricing_inputs(market, as_of).map(|(value, _)| value)
    }

    fn seed_metric_context(
        &self,
        context: &mut MetricContext,
        _market: &MarketContext,
        _as_of: Date,
    ) {
        context.discount_curve_id = Some(self.deal.discount_curve_id.clone());
        context.notional = self
            .deal
            .tranches
            .require(&self.tranche_id)
            .ok()
            .map(|note| note.current_balance);
    }

    fn get_instrument_pricing_overrides(
        &self,
    ) -> Option<
        &finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    > {
        Some(&self.deal.instrument_pricing_overrides)
    }

    fn get_instrument_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides>{
        Some(&mut self.deal.instrument_pricing_overrides)
    }

    fn get_metric_pricing_overrides(
        &self,
    ) -> Option<
        &finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    > {
        Some(&self.deal.metric_pricing_overrides)
    }

    fn get_metric_pricing_overrides_mut(
        &mut self,
    ) -> Option<
        &mut finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    > {
        Some(&mut self.deal.metric_pricing_overrides)
    }

    fn get_scenario_pricing_overrides(
        &self,
    ) -> Option<
        &finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    > {
        Some(&self.deal.scenario_pricing_overrides)
    }

    fn get_scenario_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides>{
        Some(&mut self.deal.scenario_pricing_overrides)
    }
}
