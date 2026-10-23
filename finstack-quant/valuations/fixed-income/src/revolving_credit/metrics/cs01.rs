//! Z-spread CS01 inputs for revolving credit facilities.
//!
//! Revolving credit is valued by [`RevolvingCreditPricer`], which discounts the
//! facility's cashflows on a single discount curve and survival-weights them
//! **only when a hazard curve is present** (plus a recovery leg). Consequently:
//!
//! - **With** a replayable credit curve, a par-spread bump followed by hazard
//!   rebootstrap moves the PV, so CS01 is reported via the canonical
//!   [`GenericParallelCs01`] calculator. An analyst-built curve without a
//!   calibration recipe gets a direct hazard-rate bump instead
//!   (`metrics::sensitivities::cs01_direct_hazard`).
//! - **Without** a credit curve, survival is identically `1.0` and the recovery
//!   leg vanishes, so the canonical CS01 is zero. Credit-spread risk is then
//!   reported via the market-standard z-spread bump in
//!   [`ZSpreadParallelCs01`] / [`ZSpreadBucketedCs01`], using this module's
//!   deterministic holder-view flows.
//!
//! Only **deterministic** facilities expose a single holder-view schedule to
//! z-bump. A stochastic facility with no credit curve has no deterministic
//! schedule (its credit dynamics belong on a hazard curve), so requesting
//! CS01 is a validation error — a silent `0.0` would mask unmonitored credit
//! exposure. Supply a `credit_curve_id` to obtain hazard-based CS01.
//!
//! [`RevolvingCreditPricer`]: crate::revolving_credit::RevolvingCreditPricer
//! [`GenericParallelCs01`]: finstack_quant_valuations_core::metrics::sensitivities::cs01::GenericParallelCs01
//! [`ZSpreadParallelCs01`]: finstack_quant_valuations_core::metrics::ZSpreadParallelCs01
//! [`ZSpreadBucketedCs01`]: finstack_quant_valuations_core::metrics::ZSpreadBucketedCs01

use crate::revolving_credit::cashflow_engine::CashflowEngine;
use crate::revolving_credit::types::DrawRepaySpec;
use crate::revolving_credit::RevolvingCredit;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::metrics::{ZSpreadCs01, ZSpreadCs01Inputs};

impl ZSpreadCs01 for RevolvingCredit {
    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<ZSpreadCs01Inputs> {
        // Compounding frequency for the z-spread shift = coupon/fee payments per
        // year, mirroring the term-loan and bond z-spread convention.
        let compounds_per_year = self.frequency.compounding_frequency();

        let flows = match &self.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => {
                // Reuse the pricer's deterministic schedule (same fixings path)
                // so PV_z(0) reproduces the no-credit-curve base PV.
                let schedule = CashflowEngine::deterministic(self, curves, as_of)?;

                // `price_single_path` discounts every scheduled flow on/after
                // `as_of`; flows exactly on `as_of` carry t = 0 (zero spread
                // sensitivity), so excluding them in the bump cache is
                // immaterial to CS01.
                let mut flows: Vec<(Date, Money)> = schedule
                    .schedule
                    .get_flows()
                    .iter()
                    .filter(|cf| cf.date >= as_of)
                    .map(|cf| (cf.date, cf.amount))
                    .collect();

                // Upfront fee is a separate PV term in the pricer, paid at the
                // commitment date and only when that date is in the future.
                let upfront = self.upfront_fee_amount()?;
                if upfront.amount() > 0.0 && self.issue_date > as_of {
                    flows.push((self.issue_date, upfront));
                }
                flows
            }
            // Stochastic facilities with no credit curve: there is no
            // deterministic schedule to z-bump, and reporting a silent 0.0
            // would hide unmonitored credit exposure (see module docs).
            DrawRepaySpec::Stochastic(_) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "RevolvingCredit {}: CS01 for a stochastic facility requires a \
                     credit_curve_id (hazard-based CS01); the z-spread fallback only \
                     applies to deterministic draw/repay schedules",
                    self.id
                )));
            }
        };

        Ok(ZSpreadCs01Inputs {
            settlement: as_of,
            discount_curve_id: self.discount_curve_id.clone(),
            compounds_per_year,
            flows,
        })
    }
}
