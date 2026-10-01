//! Jump-to-Default metric for single-name CDS.
//!
//! Calculates the signed contractual settlement if the reference entity defaults
//! immediately. The current mark is excluded; `default_exposure` subtracts it.
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
//!
//! ## Protection window
//!
//! An immediate default is covered from `protection_start()` (inclusive) until
//! `premium_leg.end` (exclusive), matching the pricer's represented protection
//! interval. This tests the existing contract's protection, without applying a
//! fresh trade's quote step-in delay. Before a forward protection start, no LGD
//! payout is due. Premium accrual follows its separate contractual window: when
//! premium starts earlier, accrued premium remains payable on default before
//! protection begins. At or after protection end both payout metrics are zero,
//! even if a rolled coupon remains unpaid.
//! `default_exposure` retains its clean definition of eligible signed LGD less
//! the current mark in all these states; it is not a pending-coupon cashflow model.

use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::instruments::credit_derivatives::cds::pricing::{AccrualDayCountPolicy, CdsPricer};
use crate::instruments::credit_derivatives::cds::{CreditDefaultSwap, PayReceive};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::Date;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;

/// Jump-to-default calculator for single-name CDS (includes accrued premium).
pub(crate) struct JumpToDefaultCalculator;

impl MetricCalculator for JumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Protection and premium have separate contractual start dates.
        let signed_protection = signed_lgd_payout(cds, as_of);

        // Calculate accrued premium from last coupon date to as_of
        let accrued_premium = calculate_accrued_premium(cds, as_of)?;

        // Apply sign based on position:
        // - Protection buyer: receives protection, pays accrued → JTD = protection - accrued
        // - Protection seller: pays protection, receives accrued → JTD = accrued - protection
        let signed_jtd = match cds.side {
            PayReceive::Pay => signed_protection - accrued_premium,
            PayReceive::Receive => signed_protection + accrued_premium,
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

        Ok(signed_lgd_payout(cds, context.as_of))
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
        Ok(signed_lgd_payout(cds, context.as_of) - context.base_value.amount())
    }
}

fn has_default_protection(cds: &CreditDefaultSwap, as_of: Date) -> bool {
    cds.protection_start() <= as_of && as_of < cds.premium_leg.end
}

fn signed_lgd_payout(cds: &CreditDefaultSwap, as_of: Date) -> f64 {
    if !has_default_protection(cds, as_of) {
        return 0.0;
    }
    let lgd = 1.0 - cds.protection_leg.recovery_rate;
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
    as_of: finstack_quant_core::dates::Date,
) -> Result<f64> {
    let accrual_fraction = CdsPricer::new().coupon_accrued_fraction(
        cds,
        as_of,
        AccrualDayCountPolicy::IsdaStandard,
    )?;
    let spread = cds.premium_leg.coupon_bp.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "premium.coupon_bp cannot be represented as f64".to_string(),
        )
    })? / BASIS_POINTS_PER_UNIT;
    Ok(cds.notional.amount() * spread * accrual_fraction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use rust_decimal::Decimal;
    use std::sync::Arc;
    use time::macros::date;

    fn contract(premium_start: Date, protection_start: Date) -> CreditDefaultSwap {
        let mut cds = CreditDefaultSwap::example().expect("valid example");
        cds.notional = Money::new(10_000_000.0, Currency::USD).expect("finite notional");
        cds.premium_leg.start = premium_start;
        cds.premium_leg.end = date!(2026 - 09 - 20);
        cds.premium_leg.coupon_bp = Decimal::from(100);
        cds.premium_leg.day_count = DayCount::Act360;
        cds.protection_leg.recovery_rate = 0.4;
        cds.protection_effective_date = Some(protection_start);
        cds
    }

    fn context(cds: CreditDefaultSwap, as_of: Date, mark: f64) -> MetricContext {
        MetricContext::new(
            Arc::new(cds),
            Arc::new(MarketContext::new()),
            as_of,
            Money::new(mark, Currency::USD).expect("finite mark"),
            MetricContext::default_config(),
        )
    }

    #[test]
    fn forward_default_before_protection_preserves_separate_premium_window() {
        let protection_start = date!(2025 - 02 - 01);
        // A wholly forward contract owes nothing on default before start, but
        // an earlier premium start still creates a default-accrual obligation.
        for premium_start in [date!(2025 - 01 - 01), protection_start] {
            for (side, mark) in [
                (PayReceive::Pay, 250_000.0),
                (PayReceive::Receive, -250_000.0),
            ] {
                let mut cds = contract(premium_start, protection_start);
                cds.side = side;
                let mut ctx = context(cds, date!(2025 - 01 - 31), mark);
                let accrued = if premium_start < protection_start {
                    10_000_000.0 * 0.01 * 30.0 / 360.0
                } else {
                    0.0
                };
                let expected_jtd = if mark > 0.0 { -accrued } else { accrued };
                assert!(
                    (JumpToDefaultCalculator.calculate(&mut ctx).expect("JTD") - expected_jtd)
                        .abs()
                        < 1e-8
                );
                assert_eq!(
                    JumpToDefaultLgdOnlyCalculator
                        .calculate(&mut ctx)
                        .expect("LGD"),
                    0.0
                );
                assert_eq!(
                    DefaultExposureCalculator
                        .calculate(&mut ctx)
                        .expect("exposure"),
                    -mark
                );
            }
        }
    }

    #[test]
    fn default_payout_uses_inclusive_start_and_exclusive_end() {
        let start = date!(2025 - 02 - 01);
        for (as_of, eligible) in [
            (date!(2025 - 01 - 31), false),
            (start, true),
            (date!(2025 - 02 - 02), true),
            (date!(2026 - 09 - 19), true),
            (date!(2026 - 09 - 20), false),
            (date!(2026 - 09 - 21), false),
        ] {
            for (side, sign) in [(PayReceive::Pay, 1.0), (PayReceive::Receive, -1.0)] {
                let mut cds = contract(start, start);
                cds.side = side;
                let mut ctx = context(cds, as_of, sign * 250_000.0);
                let payout = if eligible { sign * 6_000_000.0 } else { 0.0 };
                assert_eq!(
                    JumpToDefaultLgdOnlyCalculator
                        .calculate(&mut ctx)
                        .expect("LGD"),
                    payout
                );
                assert_eq!(
                    DefaultExposureCalculator
                        .calculate(&mut ctx)
                        .expect("exposure"),
                    payout - sign * 250_000.0
                );
                if !eligible {
                    // September 20 is Sunday: a pending rolled coupon does not
                    // extend the period over which a credit event is covered.
                    assert_eq!(
                        JumpToDefaultCalculator.calculate(&mut ctx).expect("JTD"),
                        0.0
                    );
                }
            }
        }
    }

    #[test]
    fn current_protection_retains_accrued_premium_and_position_sign() {
        let start = date!(2025 - 01 - 01);
        let expected = 6_000_000.0 - 10_000_000.0 * 0.01 * 31.0 / 360.0;
        for (side, sign) in [(PayReceive::Pay, 1.0), (PayReceive::Receive, -1.0)] {
            let mut cds = contract(start, start);
            cds.side = side;
            let mut ctx = context(cds, date!(2025 - 02 - 01), 0.0);
            assert!(
                (JumpToDefaultCalculator.calculate(&mut ctx).expect("JTD") - sign * expected).abs()
                    < 1e-8
            );
        }
    }
}
