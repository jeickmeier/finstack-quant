//! Jump-to-Default metric for single-name CDS.
//!
//! Calculates the instantaneous P&L if the reference entity defaults immediately.
//! This is a key risk metric that measures the immediate impact of a credit event.
//!
//! ## Full JTD Formula (with accrued premium)
//! ```text
//! JTD = signed(LGD × Notional) ∓ signed(Accrued Premium)
//! ```
//!
//! Where:
//! - **LGD** = 1 - Recovery Rate (Loss Given Default)
//! - **Accrued Premium** = Premium accrued from last coupon date to default
//!
//! ## Interpretation
//! - For protection **buyer** (Pay):
//!   - Receives: LGD × Notional (protection payout)
//!   - Pays: Accrued premium (payable on default per ISDA)
//!   - Net JTD = LGD × Notional - Accrued Premium (positive = gain)
//!
//! - For protection **seller** (Receive):
//!   - Pays: LGD × Notional (protection payout)
//!   - Receives: Accrued premium
//!   - Net JTD = Accrued Premium - LGD × Notional (negative = loss)
//!
//! ## Note on Accrued Premium
//!
//! Under ISDA standard documentation, accrued premium is payable upon default
//! (unlike bond coupons which may be forgiven). This calculator includes the
//! accrued premium in the JTD to give a more accurate P&L impact.

use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::instruments::credit_derivatives::cds::pricer::{AccrualDayCountPolicy, CDSPricer};
use crate::instruments::credit_derivatives::cds::{CreditDefaultSwap, PayReceive};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;

/// Jump-to-default calculator for single-name CDS (includes accrued premium).
pub(crate) struct JumpToDefaultCalculator;

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Loss given default (protection payout)
        let lgd = 1.0 - cds.protection.recovery_rate;
        let protection_payout = cds.notional.amount() * lgd;

        // Calculate accrued premium from last coupon date to as_of
        let accrued_premium = calculate_accrued_premium(cds, as_of)?;

        // Apply sign based on position:
        // - Protection buyer: receives protection, pays accrued → JTD = protection - accrued
        // - Protection seller: pays protection, receives accrued → JTD = accrued - protection
        let signed_jtd = match cds.side {
            PayReceive::Pay => protection_payout - accrued_premium,
            PayReceive::Receive => accrued_premium - protection_payout,
        };

        Ok(signed_jtd)
    }
}

/// Jump-to-default calculator (LGD only, excludes accrued premium).
///
/// This simplified version only considers the protection leg payout.
/// Use `JumpToDefaultCalculator` for a more complete P&L impact.
pub(crate) struct JumpToDefaultLgdOnlyCalculator;

impl MetricCalculator for JumpToDefaultLgdOnlyCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;

        Ok(signed_lgd_payout(cds))
    }
}

/// Clean default exposure calculator.
///
/// Computes signed LGD payout less the current mark. Unlike
/// `jump_to_default`, this excludes accrued premium on default, matching
/// Bloomberg-style clean "Def Exposure" screens.
pub(crate) struct DefaultExposureCalculator;

impl MetricCalculator for DefaultExposureCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        Ok(signed_lgd_payout(cds) - context.base_value.amount())
    }
}

fn signed_lgd_payout(cds: &CreditDefaultSwap) -> f64 {
    let lgd = 1.0 - cds.protection.recovery_rate;
    let payout = cds.notional.amount() * lgd;
    match cds.side {
        PayReceive::Pay => payout,
        PayReceive::Receive => -payout,
    }
}

/// Calculate accrued premium from the last coupon date to the given date.
///
/// Uses the CDS pricer's canonical accrued-fraction helper with the ISDA
/// jump-to-default convention (plain `year_fraction` for every day-count;
/// no `Act/360` +1-day inclusivity). Schedule generation matches the pricing
/// engine's coupon dates (IMM dates: 20th of Mar/Jun/Sep/Dec).
fn calculate_accrued_premium(
    cds: &CreditDefaultSwap,
    as_of: finstack_core::dates::Date,
) -> Result<f64> {
    let accrual_fraction = CDSPricer::new().coupon_accrued_fraction(
        cds,
        as_of,
        AccrualDayCountPolicy::IsdaStandard,
    )?;
    let spread = cds.premium.spread_bp.to_f64().ok_or_else(|| {
        finstack_core::Error::Validation(
            "premium spread_bp cannot be represented as f64".to_string(),
        )
    })? / BASIS_POINTS_PER_UNIT;
    Ok(cds.notional.amount() * spread * accrual_fraction)
}
