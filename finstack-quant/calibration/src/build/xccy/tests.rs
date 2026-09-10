//! Tests for the surrounding crate component and its documented behavior.
//!
use crate::build::xccy::build_xccy_instrument;
use crate::build::BuildCtx;
use crate::quotes::ids::{Pillar, QuoteId};
use crate::quotes::xccy::XccyQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations::instruments::rates::xccy_swap::XccySwap;
use finstack_quant_valuations::market::conventions::ids::XccyConventionId;

fn xccy_build_ctx(as_of: Date) -> BuildCtx {
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("domestic_discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("foreign_discount".to_string(), "EUR-OIS".to_string());
    curve_ids.insert("domestic_forward".to_string(), "USD-SOFR-OIS".to_string());
    curve_ids.insert("foreign_forward".to_string(), "EUR-ESTR-OIS".to_string());
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

    let swap = instrument
        .as_any()
        .downcast_ref::<XccySwap>()
        .expect("Expected XccySwap");
    assert_eq!(swap.leg1.currency, Currency::EUR);
    assert_eq!(swap.leg2.currency, Currency::USD);
    assert_eq!(swap.reporting_currency, Currency::USD);
    assert!((swap.leg2.notional.amount() - 10_000_000.0).abs() < 1e-8);
    assert!((swap.leg1.notional.amount() - (10_000_000.0 / 1.10)).abs() < 1e-8);
    assert_eq!(swap.leg1.payment_lag_days, 2);
    assert_eq!(swap.leg2.payment_lag_days, 2);
    assert_eq!(swap.leg1.reset_lag_days, Some(0));
    assert_eq!(swap.leg2.reset_lag_days, Some(0));
    assert!(swap.leg2.end > swap.leg2.start);

    assert_eq!(
        swap.leg1.spread_bp,
        rust_decimal::Decimal::try_from(-15.0).expect("valid decimal"),
        "basis spread must sit on the base-currency (EUR) leg"
    );
    assert_eq!(
        swap.leg2.spread_bp,
        rust_decimal::Decimal::ZERO,
        "USD quote-currency leg must pay its index flat"
    );
    assert!(
        matches!(
            swap.leg1.compounding,
            finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding::CompoundedInArrears { .. }
        ),
        "EUR contractual ESTR-OIS must compound in arrears, got {:?}",
        swap.leg1.compounding
    );
    assert!(
        matches!(
            swap.leg2.compounding,
            finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding::CompoundedInArrears { .. }
        ),
        "USD contractual SOFR-OIS must compound in arrears, got {:?}",
        swap.leg2.compounding
    );
}

#[test]
fn unregistered_forward_override_keeps_contractual_overnight_compounding() {
    let as_of =
        Date::from_calendar_date(2025, time::Month::January, 10).expect("valid XCCY build date");
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("domestic_discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("foreign_discount".to_string(), "EUR-OIS".to_string());
    curve_ids.insert(
        "domestic_forward".to_string(),
        "USD-SOFR-OIS-ALIAS".to_string(),
    );
    curve_ids.insert("foreign_forward".to_string(), "EUR-ESTR-OIS".to_string());
    let ctx = BuildCtx::new(as_of, 10_000_000.0, curve_ids);

    let quote = XccyQuote {
        id: QuoteId::new("EURUSD-XCCY-5Y-ALIAS"),
        convention: XccyConventionId::new("EUR/USD-XCCY"),
        far_pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y XCCY tenor")),
        basis_spread_bp: -15.0,
        spot_fx: Some(1.10),
    };

    let instrument = build_xccy_instrument(&quote, &ctx).expect("build xccy swap");
    let swap = instrument
        .as_any()
        .downcast_ref::<XccySwap>()
        .expect("Expected XccySwap");
    assert_eq!(swap.leg2.forward_curve_id.as_str(), "USD-SOFR-OIS-ALIAS");
    assert!(
        matches!(
            swap.leg2.compounding,
            finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding::CompoundedInArrears { .. }
        ),
        "unregistered forward alias must keep contractual overnight compounding, got {:?}",
        swap.leg2.compounding
    );
}

#[test]
fn registered_term_forward_override_on_ois_convention_is_rejected() {
    let as_of =
        Date::from_calendar_date(2025, time::Month::January, 10).expect("valid XCCY build date");
    let mut curve_ids = finstack_quant_core::HashMap::default();
    curve_ids.insert("domestic_discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("foreign_discount".to_string(), "EUR-OIS".to_string());
    curve_ids.insert("domestic_forward".to_string(), "USD-SOFR-3M".to_string());
    curve_ids.insert("foreign_forward".to_string(), "EUR-ESTR-OIS".to_string());
    let ctx = BuildCtx::new(as_of, 10_000_000.0, curve_ids);

    let quote = XccyQuote {
        id: QuoteId::new("EURUSD-XCCY-5Y-TERM"),
        convention: XccyConventionId::new("EUR/USD-XCCY"),
        far_pillar: Pillar::Tenor("5Y".parse().expect("valid 5Y XCCY tenor")),
        basis_spread_bp: -15.0,
        spot_fx: Some(1.10),
    };

    let Err(err) = build_xccy_instrument(&quote, &ctx) else {
        unreachable!("term override on OIS must fail");
    };
    assert!(
        err.to_string().contains("USD-SOFR-3M"),
        "error should name the mismatched override, got {err}"
    );
}

#[cfg(test)]
mod mtm_reset_builder_tests {
    use super::*;
    use finstack_quant_valuations::instruments::rates::xccy_swap::{
        NotionalExchange, ResettingSide,
    };

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
        let swap = instrument
            .as_any()
            .downcast_ref::<XccySwap>()
            .expect("instrument is an XccySwap");

        assert_eq!(
            swap.notional_exchange,
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg2,
            },
            "EUR/USD should default to MtM-resetting on leg2 (USD)"
        );
    }
}
