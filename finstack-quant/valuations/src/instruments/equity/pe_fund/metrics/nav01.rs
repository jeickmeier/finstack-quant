//! NAV01 calculator for PrivateMarketsFund.
//!
//! Computes NAV01 (residual NAV sensitivity) using finite differences:
//! the LP position's PV change for a 1% proportional move in fund value.
//!
//! # Formula
//! ```text
//! NAV01 = (PV(value scaled * (1 + h)) - PV(value scaled * (1 - h))) / 2
//! ```
//! Where the FD bump h is 1% (0.01). The central difference is divided by 2
//! (not 2h), so the result is the PV change *per 1% move*, in currency
//! units, consistent with the per-1bp `Dv01` convention.
//!
//! # Note
//! Both value inputs are scaled by ±1%: every distribution/proceeds event
//! (which drives the waterfall and future LP cashflows) and the fund's
//! stated `unrealized_nav` (the residual mark that dominates holder-view PV
//! for an unrealized fund). Contributions are unchanged.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::pe_fund::waterfall::FundEventKind;
use crate::instruments::equity::pe_fund::PrivateMarketsFund;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Standard NAV bump: 1% (0.01)
const NAV_BUMP_PCT: f64 = 0.01;

/// Return a copy of the fund with all distribution/proceeds events and the
/// stated `unrealized_nav` scaled by `1 + bump`.
fn scaled_fund(
    fund: &PrivateMarketsFund,
    bump: f64,
) -> finstack_quant_core::Result<PrivateMarketsFund> {
    let mut scaled = fund.clone();
    for event in &mut scaled.events {
        if matches!(
            event.kind,
            FundEventKind::Distribution | FundEventKind::Proceeds
        ) {
            event.amount = Money::new(
                event.amount.amount() * (1.0 + bump),
                event.amount.currency(),
            )?;
        }
    }
    if let Some(nav) = scaled.unrealized_nav {
        scaled.unrealized_nav = Some(Money::new(nav.amount() * (1.0 + bump), nav.currency())?);
    }
    Ok(scaled)
}

/// NAV01 calculator for PrivateMarketsFund.
pub(crate) struct Nav01Calculator;

impl MetricCalculator for Nav01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fund: &PrivateMarketsFund = context.instrument_as()?;
        let as_of = context.as_of;

        let pv_up = scaled_fund(fund, NAV_BUMP_PCT)?
            .value(context.curves.as_ref(), as_of)?
            .amount();
        let pv_down = scaled_fund(fund, -NAV_BUMP_PCT)?
            .value(context.curves.as_ref(), as_of)?
            .amount();

        // Scenarios are ±1%; return the PV change for a 1% NAV move.
        Ok((pv_up - pv_down) / 2.0)
    }
}
