//! Z-spread CS01 inputs for revolving credit facilities.
//!
//! Revolving credit is valued by [`RevolvingCreditPricer`], which discounts the
//! facility's cashflows on a single discount curve and survival-weights them
//! **only when a hazard curve is present** (plus a recovery leg). Consequently:
//!
//! - **With** a credit curve, a par-spread / hazard bump moves the PV, so CS01
//!   is reported via the canonical hazard CS01 (the calculator delegates to
//!   [`GenericParallelCs01`]).
//! - **Without** a credit curve, survival is identically `1.0` and the recovery
//!   leg vanishes, so the canonical CS01 is zero. Credit-spread risk is then
//!   reported via the market-standard z-spread bump in
//!   [`ZSpreadParallelCs01`] / [`ZSpreadBucketedCs01`], using this module's
//!   deterministic holder-view flows.
//!
//! Only **deterministic** facilities expose a single holder-view schedule to
//! z-bump. A stochastic facility with no credit curve has no deterministic
//! schedule (its credit dynamics belong on a hazard curve), so it reports an
//! empty flow set — CS01 reads `0.0` rather than erroring, and a credit curve
//! should be supplied to obtain hazard-based CS01.
//!
//! [`RevolvingCreditPricer`]: crate::instruments::fixed_income::revolving_credit::pricer::RevolvingCreditPricer
//! [`GenericParallelCs01`]: crate::metrics::GenericParallelCs01
//! [`ZSpreadParallelCs01`]: crate::metrics::ZSpreadParallelCs01
//! [`ZSpreadBucketedCs01`]: crate::metrics::ZSpreadBucketedCs01

use crate::instruments::fixed_income::revolving_credit::cashflow_engine::CashflowEngine;
use crate::instruments::fixed_income::revolving_credit::types::{BaseRateSpec, DrawRepaySpec};
use crate::instruments::RevolvingCredit;
use crate::metrics::{ZSpreadCs01, ZSpreadCs01Inputs};
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

impl ZSpreadCs01 for RevolvingCredit {
    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<ZSpreadCs01Inputs> {
        // Compounding frequency for the z-spread shift = coupon/fee payments per
        // year, mirroring the term-loan and bond z-spread convention.
        let years = self.frequency.to_years_simple();
        let compounds_per_year = if years > 0.0 && years.is_finite() {
            (1.0 / years).round().max(1.0)
        } else {
            1.0
        };

        let flows = match &self.draw_repay_spec {
            DrawRepaySpec::Deterministic(_) => {
                // Reuse the pricer's deterministic schedule (same fixings path)
                // so PV_z(0) reproduces the no-credit-curve base PV.
                let fixings = match &self.base_rate_spec {
                    BaseRateSpec::Floating(spec) => {
                        finstack_core::market_data::fixings::get_fixing_series(
                            curves,
                            spec.index_id.as_ref(),
                        )
                        .ok()
                    }
                    _ => None,
                };
                let engine = CashflowEngine::new(self, Some(curves), as_of, fixings)?;
                let schedule = engine.generate_deterministic()?;

                // `price_single_path` discounts every scheduled flow on/after
                // `as_of`; flows exactly on `as_of` carry t = 0 (zero spread
                // sensitivity), so excluding them in the bump cache is
                // immaterial to CS01.
                let mut flows: Vec<(Date, Money)> = schedule
                    .schedule
                    .flows
                    .iter()
                    .filter(|cf| cf.date >= as_of)
                    .map(|cf| (cf.date, cf.amount))
                    .collect();

                // Upfront fee is a separate PV term in the pricer, paid at the
                // commitment date and only when that date is in the future.
                if let Some(upfront) = self.fees.upfront_fee {
                    if self.commitment_date > as_of {
                        flows.push((self.commitment_date, upfront));
                    }
                }
                flows
            }
            // Stochastic facilities with no credit curve: no deterministic
            // schedule to z-bump (see module docs).
            DrawRepaySpec::Stochastic(_) => Vec::new(),
        };

        Ok(ZSpreadCs01Inputs {
            settlement: as_of,
            discount_curve_id: self.discount_curve_id.clone(),
            compounds_per_year,
            flows,
        })
    }
}
