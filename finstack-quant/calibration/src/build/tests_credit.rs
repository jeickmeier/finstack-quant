//! Quote-to-CDS-instrument construction tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::build::cds::build_cds_instrument;
use crate::build::BuildCtx;
use crate::quotes::cds::CdsQuote;
use crate::quotes::ids::Pillar;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsConvention;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use rust_decimal::Decimal;

#[test]
fn test_build_cds_par_spread() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "XYZ-CORP-SNR".to_string());
    let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

    // Use USD:IsdaNa
    let key = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::IsdaNa,
    };

    let quote = CdsQuote::CdsParSpread {
        id: "CDS-TEST-1".into(),
        entity: "XYZ-CORP-SNR".to_string(),
        convention: key,
        pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
        spread_bp: 120.0,
        recovery_rate: 0.40,
    };

    // Note: This relies on USD:IsdaNa being in the embedded registry.
    // If not, we might fail like in rates. But we added IsdaNa to enum.
    let instrument = build_cds_instrument(&quote, &ctx).expect("build cds par");

    assert_eq!(instrument.id(), "CDS-TEST-1");

    if let Some(cds) = instrument.as_any().downcast_ref::<CreditDefaultSwap>() {
        assert_eq!(cds.notional.currency(), Currency::USD);
        assert_eq!(cds.premium_leg.coupon_bp, Decimal::from(120));
        assert_eq!(cds.protection_leg.recovery_rate, 0.40);
        assert_eq!(cds.convention, CdsConvention::IsdaNa);
        assert_eq!(cds.doc_clause, Some(CdsDocClause::IsdaNa));
        // Verify discount/credit curve ids come from BuildCtx role mappings
        assert_eq!(cds.premium_leg.discount_curve_id.as_str(), "USD-OIS");
        assert_eq!(cds.protection_leg.credit_curve_id.as_str(), "XYZ-CORP-SNR");
    } else {
        panic!("Expected CreditDefaultSwap");
    }
}

#[test]
fn test_build_cds_upfront() {
    let as_of = Date::from_calendar_date(2025, time::Month::January, 10).unwrap();
    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "XYZ-CREDIT".to_string());

    let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);

    let key = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::IsdaNa,
    };

    let quote = CdsQuote::CdsUpfront {
        id: "CDS-TEST-UP".into(),
        entity: "XYZ-CORP-SNR".to_string(),
        convention: key,
        pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
        coupon_bp: 100.0,
        upfront_pct: 0.02, // 2% upfront
        recovery_rate: 0.40,
    };

    let instrument = build_cds_instrument(&quote, &ctx).expect("build cds upfront");

    if let Some(cds) = instrument.as_any().downcast_ref::<CreditDefaultSwap>() {
        assert_eq!(cds.premium_leg.coupon_bp, Decimal::from(100)); // Running
        assert!(cds.upfront.is_some());
        assert_eq!(cds.convention, CdsConvention::IsdaNa);
        assert_eq!(cds.doc_clause, Some(CdsDocClause::IsdaNa));
        if let Some((_dt, amount)) = cds.upfront {
            assert_eq!(amount.amount(), 20_000.0); // 2% of 1M
        }
        assert_eq!(cds.premium_leg.discount_curve_id.as_str(), "USD-OIS");
        assert_eq!(cds.protection_leg.credit_curve_id.as_str(), "XYZ-CREDIT");
    } else {
        panic!("Expected CreditDefaultSwap");
    }
}
