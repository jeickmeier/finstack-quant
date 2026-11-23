//! Deterministic schedule-backed pool flow source.

use super::super::exercise::RateView;
use super::super::pool_flow_source::period_averaged_monthly_rate;
use super::super::pool_flows::PoolFlows;
use super::super::*;
use super::period::{NamePeriod, PeriodModel, hazard_period_default, run_period};
use super::schedule::PreparedInstrumentSchedules;
use super::types::{NameState, PeriodDefaultSource};
use crate::structured_credit::types::StructuredCredit;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;

/// [`PoolFlowSource`] driven by instrument schedules and the deal's
/// deterministic behavioural assumptions.
pub(crate) struct InstrumentScheduleFlowSource {
    prepared: PreparedInstrumentSchedules,
    names: Vec<NameState>,
    inputs: Vec<NamePeriod>,
    next_period: usize,
}

impl InstrumentScheduleFlowSource {
    /// Bucket every instrument's contractual schedule onto the prepared legal
    /// periods and verify the contractual draw calendar can be funded.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Resolved deal whose pool carries instrument collateral.
    /// * `context` - Market context used to project each instrument's schedule.
    /// * `prepared` - Prepared simulation supplying the valuation date and periods.
    pub(crate) fn prepare(
        instrument: &StructuredCredit,
        context: &MarketContext,
        prepared: &PreparedDealSimulation,
    ) -> Result<Self> {
        let schedules = PreparedInstrumentSchedules::prepare(instrument, context, prepared)?;
        let period_dates: Vec<Date> = prepared.periods.iter().map(|p| p.payment_date).collect();
        schedules.check_deterministic_funding(
            instrument.pool.reserve_account.amount(),
            instrument.pool.reserve_target.map(|m| m.amount()),
            &period_dates,
        )?;
        let names = schedules.initial_names();
        let inputs = Vec::with_capacity(names.len());
        Ok(Self {
            prepared: schedules,
            names,
            inputs,
            next_period: 0,
        })
    }
}

impl PoolFlowSource for InstrumentScheduleFlowSource {
    fn calculate_pool_flows(&mut self, request: PoolFlowRequest<'_, '_>) -> Result<PoolFlows> {
        let k = self.next_period;
        self.next_period += 1;
        let deal = request.instrument;

        // Deal-level behavioural rates for instruments without a hazard curve.
        let deal_mdr = period_averaged_monthly_rate(
            request.seasoning_months,
            request.months_per_period,
            |seasoning| deal.credit_model.default_spec.mdr(seasoning),
        )?;
        let deal_smm = period_averaged_monthly_rate(
            request.seasoning_months,
            request.months_per_period,
            |seasoning| deal.credit_model.prepayment_spec.smm(seasoning),
        )?;
        let period_pd = 1.0 - (1.0 - deal_mdr.clamp(0.0, 1.0)).powf(request.months_per_period);
        let period_smm = 1.0 - (1.0 - deal_smm.clamp(0.0, 1.0)).powf(request.months_per_period);
        let deal_recovery = deal.credit_model.recovery_spec.rate;

        self.inputs.clear();
        for (i, schedule) in self.prepared.schedules.iter().enumerate() {
            let alive = !self.names[i].retired && !request.state.pool_state.defaulted[i];
            let (pd, recovery_rate) = if !alive {
                (0.0, 0.0)
            } else {
                match &schedule.default_source {
                    PeriodDefaultSource::HazardCurve(id) => hazard_period_default(
                        request.context,
                        id,
                        request.prev_date,
                        request.pay_date,
                        schedule.recovery_rate,
                    )?,
                    PeriodDefaultSource::DealModel => {
                        (period_pd, schedule.recovery_rate.unwrap_or(deal_recovery))
                    }
                }
            };
            self.inputs.push(NamePeriod {
                pd,
                recovery_rate,
                utilization: None,
                interest_addon: 0.0,
            });
        }

        let discount_curve = self.prepared.discount_curve(deal, request.context)?;
        let rate_view = RateView {
            curve: discount_curve.as_deref().map(|c| c as &dyn Discounting),
            shift: request.state.floating_rate_shift,
        };
        run_period(
            &self.prepared,
            &mut self.names,
            request.state,
            PeriodModel {
                k,
                prev_date: request.prev_date,
                pay_date: request.pay_date,
                period_smm,
                rate_view,
                names: &self.inputs,
            },
        )
    }
}
