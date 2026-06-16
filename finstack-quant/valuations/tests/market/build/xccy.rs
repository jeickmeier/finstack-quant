use crate::instruments::rates::xccy_swap::XccySwap;
use crate::market::build::xccy::build_xccy_instrument;
use crate::market::conventions::ids::XccyConventionId;
use crate::market::quotes::ids::{Pillar, QuoteId};
use crate::market::quotes::xccy::XccyQuote;
use crate::market::BuildCtx;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;

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
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let ctx = xccy_build_ctx(as_of);

    let quote = XccyQuote::BasisSwap {
        id: QuoteId::new("EURUSD-XCCY-5Y"),
        convention: XccyConventionId::new("EUR/USD-XCCY"),
        far_pillar: Pillar::Tenor("5Y".parse().unwrap()),
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
}

#[cfg(test)]
mod mtm_reset_builder_tests {
    use super::*;
    use crate::instruments::rates::xccy_swap::{NotionalExchange, ResettingSide};

    #[test]
    fn building_g10_pair_produces_mtm_resetting_swap() {
        let base_date =
            Date::from_calendar_date(2025, time::Month::January, 2).expect("valid base date");
        let ctx = xccy_build_ctx(base_date);

        let quote = XccyQuote::BasisSwap {
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
                resetting_side: ResettingSide::Leg1,
            },
            "G10 pair should default to MtM-resetting on leg1 (non-USD)"
        );
    }
}
