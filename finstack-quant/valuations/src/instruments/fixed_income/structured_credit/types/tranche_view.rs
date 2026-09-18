//! One note of a structured-credit deal priced as an instrument.
//!
//! The metric registry reprices "the instrument" for every bump (Default01,
//! Prepayment01, Recovery01, Severity01, DV01, theta, ...) through
//! [`Instrument::base_value`]. When a caller asks for one tranche's metrics,
//! placing the whole deal in the metric context makes every sensitivity a
//! deal-level number that the senior and the equity share. This view makes
//! the tranche the priced instrument: `base_value` is the tranche's PV and
//! nothing else changes.
//!
//! The deal's own metric calculators downcast the context instrument to
//! [`StructuredCredit`]; the view keeps that working by exposing the deal
//! through `as_any`, so calculators reading deal fields (prices, accrued,
//! yield, WAL, pool metrics) see the deal while every reprice sees the
//! tranche.

use super::{DealType, StructuredCredit};
use crate::cashflow::traits::{schedule_from_classified_flows, ScheduleBuildOpts};
use crate::instruments::common_impl::traits::{Attributes, Instrument};
use crate::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// A deal plus the identifier of the note the metrics are computed for.
///
/// Built by [`StructuredCredit::value_tranche_with_metrics`]; direct callers
/// can place it in a `MetricContext` to obtain per-tranche registry metrics.
#[derive(Clone, Debug)]
pub struct StructuredCreditTranche {
    /// The deal that owns the note.
    pub deal: StructuredCredit,
    /// Identifier of the priced note.
    pub tranche_id: String,
}

impl StructuredCreditTranche {
    /// Pair a deal with the note whose metrics are wanted.
    ///
    /// # Arguments
    ///
    /// * `deal` - The structured-credit deal the note belongs to.
    /// * `tranche_id` - Exact identifier of one of `deal.tranches`.
    pub fn new(deal: StructuredCredit, tranche_id: impl Into<String>) -> Self {
        Self {
            deal,
            tranche_id: tranche_id.into(),
        }
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
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let flows = self
            .deal
            .get_tranche_cashflows(&self.tranche_id, context, as_of)?
            .detailed_flows;
        let day_count = match self.deal.deal_type {
            DealType::Rmbs | DealType::Cmbs => finstack_quant_core::dates::DayCount::Thirty360,
            _ => finstack_quant_core::dates::DayCount::Act360,
        };
        Ok(schedule_from_classified_flows(
            flows,
            day_count,
            ScheduleBuildOpts {
                notional_hint: finstack_quant_cashflows::CashflowScheduleSource::notional(self)?,
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
            },
        ))
    }
}

impl Instrument for StructuredCreditTranche {
    fn id(&self) -> &str {
        self.deal.id.as_str()
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::StructuredCredit
    }

    /// The deal, not the view: the deal's metric calculators downcast the
    /// context instrument to `StructuredCredit`.
    fn as_any(&self) -> &dyn std::any::Any {
        self.deal.as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self.deal.as_any_mut()
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
    ) -> finstack_quant_core::Result<
        crate::instruments::common_impl::dependencies::MarketDependencies,
    > {
        self.deal.market_dependencies()
    }

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.deal.validate_invariants()
    }

    /// The note's PV: its projected waterfall cashflows discounted from
    /// `as_of` on the deal's discount curve.
    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.deal.value_tranche(&self.tranche_id, market, as_of)
    }

    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        self.deal.model_params_snapshot()
    }

    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_quant_core::Result<Box<dyn Instrument>> {
        let deal = self.deal.with_model_params(params)?;
        let deal = deal
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .cloned()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "structured-credit model parameters must produce a StructuredCredit".into(),
                )
            })?;
        Ok(Box::new(Self {
            deal,
            tranche_id: self.tranche_id.clone(),
        }))
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn get_instrument_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::InstrumentPricingOverrides> {
        Some(&self.deal.instrument_pricing_overrides)
    }

    fn get_instrument_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::InstrumentPricingOverrides> {
        Some(&mut self.deal.instrument_pricing_overrides)
    }

    fn get_metric_pricing_overrides(&self) -> Option<&crate::instruments::MetricPricingOverrides> {
        Some(&self.deal.metric_pricing_overrides)
    }

    fn get_metric_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::MetricPricingOverrides> {
        Some(&mut self.deal.metric_pricing_overrides)
    }

    fn get_scenario_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::ScenarioPricingOverrides> {
        Some(&self.deal.scenario_pricing_overrides)
    }

    fn get_scenario_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::ScenarioPricingOverrides> {
        Some(&mut self.deal.scenario_pricing_overrides)
    }
}
