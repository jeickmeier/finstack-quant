//! The bond yield clock for the carry decomposition.

use crate::instruments::common_impl::pricing::coupon_period::icma_reference_period;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
    df_from_yield, YieldCompounding,
};
use crate::instruments::fixed_income::bond::pricing::time_basis::{
    act365l_year_fraction, bond_coupon_periods,
};
use crate::instruments::Bond;
use crate::metrics::{MetricContext, YieldClock};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::Result;

/// Street-convention accretion on the bond's coupon frequency and day count.
///
/// ACT/ACT (ICMA) resolves the quasi-coupon period from the cached cashflow
/// dates; ACT/365L keeps each enclosing coupon's denominator when the carry
/// horizon crosses a coupon boundary, as the bond yield clock does.
pub(crate) struct BondYieldClock;

impl YieldClock for BondYieldClock {
    fn accretion_factor(
        &self,
        context: &MetricContext,
        ytm: f64,
        rolled_date: Date,
    ) -> Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let frequency = bond.cashflow_spec.frequency();
        let day_count = bond.cashflow_spec.day_count();
        let dc_ctx = DayCountContext {
            frequency: Some(frequency),
            coupon_period: context.cashflows.as_deref().and_then(|flows| {
                icma_reference_period(
                    day_count,
                    frequency,
                    flows.iter().map(|(d, _)| *d),
                    context.as_of,
                )
            }),
            ..DayCountContext::default()
        };
        let tau = if day_count == DayCount::Act365L {
            act365l_year_fraction(
                frequency,
                &bond_coupon_periods(bond)?,
                context.as_of,
                rolled_date,
            )?
        } else {
            day_count.year_fraction(context.as_of, rolled_date, dc_ctx)?
        };
        let df = df_from_yield(ytm, tau, YieldCompounding::Street, frequency)?;
        Ok(1.0 / df)
    }
}
