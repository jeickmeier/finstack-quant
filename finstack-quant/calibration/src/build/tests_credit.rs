//! Quote-to-CDS-instrument construction tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::build::cds::build_cds_instrument;
use crate::build::cds_tranche::{build_cds_tranche_instrument, CdsTrancheBuildOverrides};
use crate::build::BuildCtx;
use crate::quotes::cds::CdsQuote;
use crate::quotes::cds_tranche::CdsTrancheQuote;
use crate::quotes::ids::Pillar;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, Tenor};
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CdsConvention, CdsValuationConvention,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche;
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

#[test]
fn credit_coupon_anchors_follow_trade_date_across_all_quarterly_rolls() {
    use time::Month::{December, June, March, September};

    // June, September, and December 2026 have weekend roll dates. Exercise
    // both sides of every roll, including trades whose T+3 cash crosses it.
    for ((year, month), (prior_year, prior_month)) in [
        ((2026, March), (2025, December)),
        ((2026, June), (2026, March)),
        ((2026, September), (2026, June)),
        ((2026, December), (2026, September)),
    ] {
        let roll = Date::from_calendar_date(year, month, 20).unwrap();
        let prior = Date::from_calendar_date(prior_year, prior_month, 20).unwrap();
        for offset in -5..=5 {
            let trade = roll.add_days(offset).unwrap();
            let expected_start = if offset < 0 { prior } else { roll };
            let mut curve_ids = HashMap::default();
            curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
            curve_ids.insert("credit".to_string(), "CDX.NA.IG".to_string());
            let ctx = BuildCtx::new(trade, 1_000_000.0, curve_ids);
            let convention = CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            };
            let cds_quote = CdsQuote::CdsUpfront {
                id: "CDS-ROLL".into(),
                entity: "CDX.NA.IG".to_string(),
                convention: convention.clone(),
                pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
                coupon_bp: 100.0,
                upfront_pct: 0.02,
                recovery_rate: 0.40,
            };
            let cds_instrument = build_cds_instrument(&cds_quote, &ctx).unwrap();
            let cds = cds_instrument
                .as_any()
                .downcast_ref::<CreditDefaultSwap>()
                .unwrap();
            assert_eq!(cds.premium_leg.start, expected_start, "CDS trade {trade}");
            assert!(cds.protection_start() <= trade, "CDS trade {trade}");
            assert!(cds.upfront.unwrap().0 > trade, "CDS trade {trade}");

            let tranche_quote = CdsTrancheQuote {
                id: "TRANCHE-ROLL".into(),
                index: "CDX.NA.IG".to_string(),
                series: 46,
                attachment: 0.03,
                detachment: 0.07,
                maturity: Date::from_calendar_date(2031, December, 20).unwrap(),
                upfront_pct: 0.02,
                coupon_bp: 100.0,
                convention,
            };
            let tranche_instrument = build_cds_tranche_instrument(
                &tranche_quote,
                &ctx,
                &CdsTrancheBuildOverrides::default(),
            )
            .unwrap();
            let tranche = tranche_instrument
                .as_any()
                .downcast_ref::<CdsTranche>()
                .unwrap();
            assert_eq!(
                tranche.start_date,
                Some(expected_start),
                "tranche trade {trade}"
            );
            assert_eq!(
                tranche.upfront.unwrap().0,
                cds.upfront.unwrap().0,
                "shared cash settlement for trade {trade}"
            );
        }
    }
}

#[test]
fn cds_coupon_anchor_cash_settlement_and_protection_step_in_remain_distinct() {
    use time::Month::{December, March};

    let trade = Date::from_calendar_date(2026, March, 18).unwrap();
    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "XYZ-CREDIT".to_string());
    let ctx = BuildCtx::new(trade, 1_000_000.0, curve_ids);
    let quote = CdsQuote::CdsUpfront {
        id: "CDS-CROSS-ROLL".into(),
        entity: "XYZ-CREDIT".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        },
        pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
        coupon_bp: 100.0,
        upfront_pct: 0.02,
        recovery_rate: 0.40,
    };
    for (convention, expected_step_in) in [
        (CdsValuationConvention::BloombergCdswClean, trade),
        (
            CdsValuationConvention::IsdaDirty,
            Date::from_calendar_date(2026, March, 19).unwrap(),
        ),
    ] {
        let ctx = ctx.clone().with_cds_valuation_convention(Some(convention));
        let instrument = build_cds_instrument(&quote, &ctx).unwrap();
        let cds = instrument
            .as_any()
            .downcast_ref::<CreditDefaultSwap>()
            .unwrap();
        assert_eq!(
            cds.premium_leg.start,
            Date::from_calendar_date(2025, December, 20).unwrap()
        );
        assert_eq!(
            cds.upfront.unwrap().0,
            Date::from_calendar_date(2026, March, 23).unwrap()
        );
        assert_eq!(cds.valuation_convention, convention);
        let step_in = trade
            .add_days(cds.valuation_convention.protection_step_in_days())
            .unwrap()
            .max(cds.protection_start());
        assert_eq!(step_in, expected_step_in);
    }
}
