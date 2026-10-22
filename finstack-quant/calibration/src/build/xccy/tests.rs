//! Tests for the surrounding crate component and its documented behavior.
//!
use crate::build::xccy::build_xccy_instrument;
use crate::build::BuildCtx;
use crate::quotes::XccyQuote;
use crate::quotes::{Pillar, QuoteId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::market::conventions::ids::XccyConventionId;

fn xccy_build_ctx(as_of: Date) -> BuildCtx {
    let mut curve_ids = finstack_quant_core::FxHashMap::default();
    curve_ids.insert("domestic_discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("foreign_discount".to_string(), "EUR-OIS".to_string());
    BuildCtx::new(as_of, 10_000_000.0, curve_ids)
}

#[test]
fn test_build_xccy_basis_swap() {
    let as_of =
        Date::from_calendar_date(2025, time::Month::January, 10).expect("valid XCCY build date");
    let ctx = xccy_build_ctx(as_of);

    let quote = XccyQuote {
        id: QuoteId::new("EURUSD-XCCY-5Y"),
        convention: XccyConventionId::new("EUR/USD-XCCY"),
        far_pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y XCCY tenor")),
        basis_spread_bp: -15.0,
        spot_fx: Some(1.10),
    };

    let instrument = build_xccy_instrument(&quote, &ctx).expect("build xccy swap");
    assert_eq!(instrument.id(), "EURUSD-XCCY-5Y");

    let swap = instrument;
    assert_eq!(swap.leg1.notional.currency(), Currency::EUR);
    assert_eq!(swap.leg2.notional.currency(), Currency::USD);
    assert_eq!(swap.reporting_currency, Currency::USD);
    assert!((swap.leg2.notional.amount() - 10_000_000.0).abs() < 1e-8);
    assert!((swap.leg1.notional.amount() - (10_000_000.0 / 1.10)).abs() < 1e-8);
    assert_eq!(swap.leg1.leg.payment_lag_days, 2);
    assert_eq!(swap.leg2.leg.payment_lag_days, 2);
    assert_eq!(swap.leg1.leg.reset_lag_days, 0);
    assert_eq!(swap.leg2.leg.reset_lag_days, 0);
    assert!(swap.leg2.leg.end > swap.leg2.leg.start);

    assert_eq!(
        swap.leg1.leg.spread_bp,
        rust_decimal::Decimal::try_from(-15.0).expect("valid decimal"),
        "basis spread must sit on the base-currency (EUR) leg"
    );
    assert_eq!(
        swap.leg2.leg.spread_bp,
        rust_decimal::Decimal::ZERO,
        "USD quote-currency leg must pay its index flat"
    );
    assert!(
        matches!(
            swap.leg1.leg.compounding,
            finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears { .. }
        ),
        "EUR contractual ESTR-OIS must compound in arrears, got {:?}",
        swap.leg1.leg.compounding
    );
    assert!(
        matches!(
            swap.leg2.leg.compounding,
            finstack_quant_cashflows::builder::FloatingLegCompounding::CompoundedInArrears { .. }
        ),
        "USD contractual SOFR-OIS must compound in arrears, got {:?}",
        swap.leg2.leg.compounding
    );
}

#[cfg(test)]
mod mtm_reset_builder_tests {
    use super::*;
    use finstack_quant_valuations_core::instruments::{NotionalExchange, ResettingSide};

    #[test]
    fn building_g10_pair_produces_mtm_resetting_swap() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let ctx = xccy_build_ctx(base_date);

        let quote = XccyQuote {
            id: QuoteId::new("EUR/USD-5Y"),
            convention: XccyConventionId::new("EUR/USD-XCCY"),
            far_pillar: Pillar::Tenor("5Y".parse().expect("valid tenor")),
            basis_spread_bp: -25.0,
            spot_fx: Some(1.10),
        };

        let instrument = build_xccy_instrument(&quote, &ctx).expect("build succeeds");
        let swap = instrument;

        assert_eq!(
            swap.notional_exchange,
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg2,
            },
            "EUR/USD should default to MtM-resetting on leg2 (USD)"
        );
    }
}
