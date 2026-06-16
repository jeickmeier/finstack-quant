//! Schedule diagnostics for IRS golden convention coverage.

use crate::instruments::rates::irs::InterestRateSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::days_since_epoch;
use finstack_quant_core::Result;

macro_rules! fixed_metric {
    ($name:ident, $body:expr) => {
        pub(crate) struct $name;
        impl MetricCalculator for $name {
            fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
                let swap: &InterestRateSwap = context.instrument_as()?;
                let schedule = crate::instruments::rates::irs::cashflow::fixed_leg_schedule(swap)?;
                $body(&schedule)
            }
        }
    };
}

macro_rules! floating_metric {
    ($name:ident, $body:expr) => {
        pub(crate) struct $name;
        impl MetricCalculator for $name {
            fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
                let swap: &InterestRateSwap = context.instrument_as()?;
                let schedule =
                    crate::instruments::rates::irs::cashflow::float_leg_schedule_with_curves_as_of(
                        swap,
                        Some(&context.curves),
                        Some(context.as_of),
                    )?;
                $body(&schedule)
            }
        }
    };
}

fixed_metric!(
    FixedLegPaymentCountCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| { Ok(schedule.flows.len() as f64) }
);

floating_metric!(
    FloatingLegPaymentCountCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| { Ok(schedule.flows.len() as f64) }
);

fixed_metric!(
    FixedFirstPaymentDateCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .first()
            .map(|flow| days_since_epoch(flow.date) as f64)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("fixed leg schedule is empty".to_string())
            })
    }
);

fixed_metric!(
    FixedLastPaymentDateCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .last()
            .map(|flow| days_since_epoch(flow.date) as f64)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("fixed leg schedule is empty".to_string())
            })
    }
);

floating_metric!(
    FloatingFirstPaymentDateCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .first()
            .map(|flow| days_since_epoch(flow.date) as f64)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("floating leg schedule is empty".to_string())
            })
    }
);

floating_metric!(
    FloatingLastPaymentDateCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .last()
            .map(|flow| days_since_epoch(flow.date) as f64)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("floating leg schedule is empty".to_string())
            })
    }
);

fixed_metric!(
    FixedFirstAccrualFactorCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .first()
            .map(|flow| flow.accrual_factor)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("fixed leg schedule is empty".to_string())
            })
    }
);

floating_metric!(
    FloatingFirstAccrualFactorCalculator,
    |schedule: &crate::cashflow::builder::CashFlowSchedule| {
        schedule
            .flows
            .first()
            .map(|flow| flow.accrual_factor)
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation("floating leg schedule is empty".to_string())
            })
    }
);
