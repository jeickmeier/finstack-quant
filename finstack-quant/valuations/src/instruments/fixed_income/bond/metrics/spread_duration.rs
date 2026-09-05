//! Bond spread duration on the quote-reproducing Z-spread basis.
//!
//! Canonical bond CS01 may use a hazard/par-CDS curve. Campisi attribution,
//! however, needs a spread duration paired with an observed bond spread.
//! This calculator therefore always bumps the bond's solved Z-spread and
//! normalizes around the quote-reproducing base PV:
//!
//! ```text
//! Spread Duration = -(PV(z + 1bp) - PV(z)) / (PV(z) * 1bp)
//! ```

use super::cs01::z_spread_bumped_pvs;
use crate::constants::ONE_BASIS_POINT;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

const RELATIVE_BASE_PV_EPSILON: f64 = 1e-12;

/// Bond spread duration calculated from a quote-reproducing Z-spread bump.
pub(crate) struct BondSpreadDurationCalculator;

impl MetricCalculator for BondSpreadDurationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let notional = context.instrument_as::<Bond>()?.notional.amount();
        if !notional.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond spread duration requires finite notional; got {notional}"
            )));
        }

        let (base_pv, bumped_pv) = z_spread_bumped_pvs(context)?;
        let base_pv_floor = RELATIVE_BASE_PV_EPSILON * notional.abs().max(1.0);
        if base_pv.abs() <= base_pv_floor {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond spread duration is undefined for zero or near-zero quote-reproducing \
                 base PV ({base_pv}); minimum magnitude is {base_pv_floor}"
            )));
        }

        let spread_pnl = bumped_pv - base_pv;
        if !spread_pnl.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond spread duration requires a finite Z-spread bump result; \
                 bumped PV - base PV = {spread_pnl}"
            )));
        }

        let spread_duration = -spread_pnl / (base_pv * ONE_BASIS_POINT);
        if !spread_duration.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond spread duration result must be finite; got {spread_duration}"
            )));
        }

        Ok(spread_duration)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    fn context(notional: f64) -> MetricContext {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");
        let bond = Bond::fixed(
            "SPREAD-DURATION-VALIDATION",
            Money::new(notional, Currency::USD).expect("valid money fixture"),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            as_of,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.8)])
                .build()
                .expect("curve"),
        );
        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            Money::new(notional, Currency::USD).expect("valid money fixture"),
            MetricContext::default_config(),
        );
        context.computed.insert(MetricId::ZSpread, 0.0);
        context
    }

    #[test]
    fn rejects_near_zero_quote_reproducing_base_pv() {
        let mut context = context(1e-15);
        let err = BondSpreadDurationCalculator
            .calculate(&mut context)
            .expect_err("near-zero base PV must fail closed");
        assert!(
            err.to_string().contains("near-zero"),
            "error must identify the invalid denominator; got: {err}"
        );
    }

    #[test]
    fn rejects_nonfinite_z_spread_input() {
        let mut context = context(100.0);
        context.computed.insert(MetricId::ZSpread, f64::NAN);
        let err = BondSpreadDurationCalculator
            .calculate(&mut context)
            .expect_err("non-finite Z-spread must fail closed");
        assert!(
            err.to_string().contains("finite") && err.to_string().contains("Z-spread"),
            "error must identify the non-finite Z-spread; got: {err}"
        );
    }
}
