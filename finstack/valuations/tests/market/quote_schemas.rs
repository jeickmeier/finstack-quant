//! Unit tests for market quote schema helpers and bump logic.
//!
//! These target previously uncovered quote-type modules:
//! - `market/quotes/cds.rs`
//! - `market/quotes/cds_tranche.rs`
//! - `market/quotes/inflation.rs`
//! - `market/quotes/vol.rs`

use crate::common::tolerances;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::types::UnderlyingId;
use finstack_valuations::instruments::OptionType;
use finstack_valuations::market::conventions::ids::{
    BondConventionId, CdsConventionKey, CdsDocClause, FxConventionId, FxOptionConventionId,
    InflationSwapConventionId, OptionConventionId, XccyConventionId,
};
use finstack_valuations::market::quotes::bond::BondQuote;
use finstack_valuations::market::quotes::cds::CdsQuote;
use finstack_valuations::market::quotes::cds_tranche::CDSTrancheQuote;
use finstack_valuations::market::quotes::fx::FxQuote;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::inflation::InflationQuote;
use finstack_valuations::market::quotes::vol::VolQuote;
use finstack_valuations::market::quotes::xccy::XccyQuote;
use std::str::FromStr;

fn d(y: i32, m: time::Month, day: u8) -> Date {
    Date::from_calendar_date(y, m, day).expect("valid date")
}

#[test]
fn cds_quote_id_and_bump_semantics() {
    let q = CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-ACME-5Y"),
        entity: "ACME".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
        pillar: Pillar::Tenor("5Y".parse().unwrap()),
        spread_bp: 100.0,
        recovery_rate: 0.40,
    };
    assert_eq!(q.id().as_str(), "CDS-ACME-5Y");

    // bump_decimal 0.0001 -> +1bp
    let bumped = q.bump_spread_decimal(0.0001);
    match bumped {
        CdsQuote::CdsParSpread { spread_bp, .. } => {
            assert!(
                (spread_bp - 101.0).abs() < tolerances::TIGHT,
                "spread bump mismatch: expected 101.0, got {spread_bp}"
            );
        }
        other @ CdsQuote::CdsUpfront { .. } => panic!("expected CdsParSpread, got {:?}", other),
    }

    let q2 = CdsQuote::CdsUpfront {
        id: QuoteId::new("CDS-ACME-5Y-UF"),
        entity: "ACME".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
        pillar: Pillar::Tenor("5Y".parse().unwrap()),
        running_spread_bp: 500.0,
        upfront_pct: 0.02,
        recovery_rate: 0.40,
    };
    let bumped2 = q2.bump_spread_decimal(0.0002); // +2bp
    match bumped2 {
        CdsQuote::CdsUpfront {
            running_spread_bp,
            upfront_pct,
            ..
        } => {
            assert!(
                (running_spread_bp - 502.0).abs() < tolerances::TIGHT,
                "running spread mismatch: expected 502.0, got {running_spread_bp}"
            );
            assert!(
                (upfront_pct - 0.02).abs() < tolerances::TIGHT,
                "upfront pct should be unchanged: expected 0.02, got {upfront_pct}"
            );
        }
        other @ CdsQuote::CdsParSpread { .. } => panic!("expected CdsUpfront, got {:?}", other),
    }
}

#[test]
fn cds_tranche_quote_id_and_bump_semantics() {
    let q = CDSTrancheQuote::CDSTranche {
        id: QuoteId::new("CDX-IG-3-7"),
        index: "CDX.NA.IG".to_string(),
        attachment: 0.03,
        detachment: 0.07,
        maturity: d(2029, time::Month::June, 20),
        upfront_pct: -2.5,
        running_spread_bp: 500.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    };
    assert_eq!(q.id().as_str(), "CDX-IG-3-7");

    let bumped = q.bump_spread_decimal(0.0001);
    match bumped {
        CDSTrancheQuote::CDSTranche {
            running_spread_bp,
            upfront_pct,
            ..
        } => {
            assert!(
                (running_spread_bp - 501.0).abs() < tolerances::TIGHT,
                "running spread mismatch: expected 501.0, got {running_spread_bp}"
            );
            assert!(
                (upfront_pct + 2.5).abs() < tolerances::TIGHT,
                "upfront pct should be unchanged: expected -2.5, got {upfront_pct}"
            );
        }
    }
}

#[test]
fn spread_bump_bp_decimal_parity_for_cds_and_tranche() {
    let cds = CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-ACME-3Y"),
        entity: "ACME".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
        pillar: Pillar::Tenor("3Y".parse().unwrap()),
        spread_bp: 80.0,
        recovery_rate: 0.4,
    };
    let bumped_decimal = cds.bump_spread_decimal(0.0001);
    let bumped_bp = cds.bump_spread_bp(1.0);
    match (bumped_decimal, bumped_bp) {
        (
            CdsQuote::CdsParSpread { spread_bp: dec, .. },
            CdsQuote::CdsParSpread { spread_bp: bp, .. },
        ) => {
            assert!(
                (dec - bp).abs() < tolerances::TIGHT,
                "cds decimal/bp bump mismatch: decimal {dec}, bp {bp}"
            );
        }
        other => panic!("expected CdsParSpread bumps, got {:?}", other),
    }

    let tranche = CDSTrancheQuote::CDSTranche {
        id: QuoteId::new("CDX-IG-7-10"),
        index: "CDX.NA.IG".to_string(),
        attachment: 0.07,
        detachment: 0.10,
        maturity: d(2030, time::Month::June, 20),
        upfront_pct: -1.25,
        running_spread_bp: 400.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    };
    let tranche_decimal = tranche.bump_spread_decimal(0.0001);
    let tranche_bp = tranche.bump_spread_bp(1.0);
    let (
        CDSTrancheQuote::CDSTranche {
            running_spread_bp: dec,
            ..
        },
        CDSTrancheQuote::CDSTranche {
            running_spread_bp: bp,
            ..
        },
    ) = (tranche_decimal, tranche_bp);
    assert!(
        (dec - bp).abs() < tolerances::TIGHT,
        "tranche decimal/bp bump mismatch: decimal {dec}, bp {bp}"
    );
}

#[test]
fn inflation_quote_maturity_and_bump() {
    let zcis = InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-5Y"),
        maturity: d(2029, time::Month::June, 20),
        rate: 0.025,
        index: "US-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD-CPI"),
    };
    assert_eq!(zcis.maturity_date(), Some(d(2029, time::Month::June, 20)));

    let bumped = zcis.bump_rate_decimal(0.0001);
    match bumped {
        InflationQuote::InflationSwap { rate, .. } => {
            assert!(
                (rate - 0.0251).abs() < tolerances::TIGHT,
                "inflation rate bump mismatch: expected 0.0251, got {rate}"
            );
        }
        other @ InflationQuote::YoYInflationSwap { .. } => {
            panic!("expected InflationSwap, got {:?}", other)
        }
    }

    let yoy = InflationQuote::YoYInflationSwap {
        id: QuoteId::new("USA-CPI-U-YOY-5Y"),
        maturity: d(2029, time::Month::June, 20),
        rate: 0.03,
        index: "US-CPI-U".to_string(),
        frequency: Tenor::new(1, finstack_core::dates::TenorUnit::Years),
        convention: InflationSwapConventionId::new("USD-CPI"),
    };
    assert_eq!(yoy.maturity_date(), Some(d(2029, time::Month::June, 20)));
    let bumped2 = yoy.bump_rate_decimal(-0.0005);
    match bumped2 {
        InflationQuote::YoYInflationSwap {
            rate, frequency, ..
        } => {
            assert!(
                (rate - 0.0295).abs() < tolerances::TIGHT,
                "yoy rate bump mismatch: expected 0.0295, got {rate}"
            );
            assert_eq!(
                frequency,
                Tenor::new(1, finstack_core::dates::TenorUnit::Years)
            );
        }
        other @ InflationQuote::InflationSwap { .. } => {
            panic!("expected YoYInflationSwap, got {:?}", other)
        }
    }
}

#[test]
fn vol_quote_bump_and_swaption_maturity_alias() {
    let opt = VolQuote::OptionVol {
        id: QuoteId::new("SPX-VOL-20241220-4500"),
        underlying: UnderlyingId::new("SPX"),
        expiry: d(2024, time::Month::December, 20),
        strike: 4500.0,
        vol: 0.20,
        option_type: OptionType::Call,
        convention: OptionConventionId::new("USD-EQUITY"),
    };
    let bumped = opt.bump_vol_absolute(0.01);
    match bumped {
        VolQuote::OptionVol { vol, .. } => {
            assert!(
                (vol - 0.21).abs() < tolerances::TIGHT,
                "vol bump mismatch: expected 0.21, got {vol}"
            );
        }
        other => panic!("expected OptionVol, got {:?}", other),
    }

    // Swaption maturity must use "maturity" (legacy "tenor" is rejected).
    let json_with_tenor = r#"
    {
      "swaption_vol": {
        "id": "USD-SWPTN-VOL-1Yx5Y-0.045",
        "expiry": "2025-06-20",
        "tenor": "2030-06-20",
        "strike": 0.045,
        "vol": 0.15,
        "quote_type": "Normal",
        "convention": "USD"
      }
    }"#;

    let parsed: Result<VolQuote, _> = serde_json::from_str(json_with_tenor);
    assert!(parsed.is_err(), "legacy 'tenor' field should be rejected");

    let json_with_maturity = r#"
    {
      "swaption_vol": {
        "id": "USD-SWPTN-VOL-1Yx5Y-0.045",
        "expiry": "2025-06-20",
        "maturity": "2030-06-20",
        "strike": 0.045,
        "vol": 0.15,
        "quote_type": "Normal",
        "convention": "USD"
      }
    }"#;

    let parsed: VolQuote = serde_json::from_str(json_with_maturity).expect("should parse");
    match parsed {
        VolQuote::SwaptionVol { maturity, .. } => {
            assert_eq!(maturity, d(2030, time::Month::June, 20));
        }
        other => panic!("expected SwaptionVol, got {:?}", other),
    }
}

#[test]
fn quote_serialization_roundtrip() {
    let cds = CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-ACME-5Y"),
        entity: "ACME".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
        pillar: Pillar::Tenor("5Y".parse().unwrap()),
        spread_bp: 100.0,
        recovery_rate: 0.40,
    };
    let cds_json = serde_json::to_string(&cds).expect("serialize cds");
    let cds_parsed: CdsQuote = serde_json::from_str(&cds_json).expect("deserialize cds");
    match cds_parsed {
        CdsQuote::CdsParSpread { spread_bp, .. } => {
            assert!(
                (spread_bp - 100.0).abs() < tolerances::TIGHT,
                "cds roundtrip spread mismatch: expected 100.0, got {spread_bp}"
            );
        }
        other @ CdsQuote::CdsUpfront { .. } => panic!("expected CdsParSpread, got {:?}", other),
    }

    let tranche = CDSTrancheQuote::CDSTranche {
        id: QuoteId::new("CDX-IG-3-7"),
        index: "CDX.NA.IG".to_string(),
        attachment: 0.03,
        detachment: 0.07,
        maturity: d(2029, time::Month::June, 20),
        upfront_pct: -2.5,
        running_spread_bp: 500.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    };
    let tranche_json = serde_json::to_string(&tranche).expect("serialize tranche");
    let tranche_parsed: CDSTrancheQuote =
        serde_json::from_str(&tranche_json).expect("deserialize tranche");
    let CDSTrancheQuote::CDSTranche {
        running_spread_bp,
        upfront_pct,
        ..
    } = tranche_parsed;
    assert!(
        (running_spread_bp - 500.0).abs() < tolerances::TIGHT,
        "tranche roundtrip spread mismatch: expected 500.0, got {running_spread_bp}"
    );
    assert!(
        (upfront_pct + 2.5).abs() < tolerances::TIGHT,
        "tranche roundtrip upfront mismatch: expected -2.5, got {upfront_pct}"
    );

    let infl = InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-5Y-RT"),
        maturity: d(2029, time::Month::June, 20),
        rate: 0.025,
        index: "US-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD-CPI"),
    };
    let infl_json = serde_json::to_string(&infl).expect("serialize inflation");
    let infl_parsed: InflationQuote =
        serde_json::from_str(&infl_json).expect("deserialize inflation");
    match infl_parsed {
        InflationQuote::InflationSwap { rate, .. } => {
            assert!(
                (rate - 0.025).abs() < tolerances::TIGHT,
                "inflation roundtrip rate mismatch: expected 0.025, got {rate}"
            );
        }
        other @ InflationQuote::YoYInflationSwap { .. } => {
            panic!("expected InflationSwap, got {:?}", other)
        }
    }

    let vol = VolQuote::OptionVol {
        id: QuoteId::new("SPX-VOL-20241220-4500-RT"),
        underlying: UnderlyingId::new("SPX"),
        expiry: d(2024, time::Month::December, 20),
        strike: 4500.0,
        vol: 0.20,
        option_type: OptionType::Call,
        convention: OptionConventionId::new("USD-EQUITY"),
    };
    let vol_json = serde_json::to_string(&vol).expect("serialize vol");
    let vol_parsed: VolQuote = serde_json::from_str(&vol_json).expect("deserialize vol");
    match vol_parsed {
        VolQuote::OptionVol { vol, .. } => {
            assert!(
                (vol - 0.20).abs() < tolerances::TIGHT,
                "vol roundtrip mismatch: expected 0.20, got {vol}"
            );
        }
        other => panic!("expected OptionVol, got {:?}", other),
    }

    let fx = FxQuote::ForwardOutright {
        id: QuoteId::new("EURUSD-FWD-3M"),
        convention: FxConventionId::new("EUR/USD"),
        pillar: Pillar::Tenor("3M".parse().unwrap()),
        forward_rate: 1.1050,
    };
    let fx_json = serde_json::to_string(&fx).expect("serialize fx");
    let fx_parsed: FxQuote = serde_json::from_str(&fx_json).expect("deserialize fx");
    match fx_parsed {
        FxQuote::ForwardOutright { forward_rate, .. } => {
            assert!(
                (forward_rate - 1.1050).abs() < tolerances::TIGHT,
                "fx forward roundtrip mismatch: expected 1.1050, got {forward_rate}"
            );
        }
        other => panic!("expected ForwardOutright, got {:?}", other),
    }

    let fx_swap = FxQuote::SwapOutright {
        id: QuoteId::new("EURUSD-SWAP-3M"),
        convention: FxConventionId::new("EUR/USD"),
        far_pillar: Pillar::Tenor("3M".parse().unwrap()),
        near_rate: 1.1000,
        far_rate: 1.1055,
    };
    let fx_swap_json = serde_json::to_string(&fx_swap).expect("serialize fx swap");
    let fx_swap_parsed: FxQuote = serde_json::from_str(&fx_swap_json).expect("deserialize fx swap");
    match fx_swap_parsed {
        FxQuote::SwapOutright {
            near_rate,
            far_rate,
            ..
        } => {
            assert!(
                (near_rate - 1.1000).abs() < tolerances::TIGHT,
                "fx swap near rate mismatch: expected 1.1000, got {near_rate}"
            );
            assert!(
                (far_rate - 1.1055).abs() < tolerances::TIGHT,
                "fx swap far rate mismatch: expected 1.1055, got {far_rate}"
            );
        }
        other => panic!("expected SwapOutright, got {:?}", other),
    }

    let bond = BondQuote::FixedRateBulletCleanPrice {
        id: QuoteId::new("BOND-UST-5Y"),
        currency: Currency::USD,
        issue_date: d(2025, time::Month::January, 15),
        maturity: d(2030, time::Month::January, 15),
        coupon_rate: 0.045,
        convention: BondConventionId::new("USD-UST"),
        clean_price_pct: 99.25,
    };
    let bond_json = serde_json::to_string(&bond).expect("serialize bond");
    let bond_parsed: BondQuote = serde_json::from_str(&bond_json).expect("deserialize bond");
    match bond_parsed {
        BondQuote::FixedRateBulletCleanPrice {
            clean_price_pct,
            coupon_rate,
            ..
        } => {
            assert!(
                (clean_price_pct - 99.25).abs() < tolerances::TIGHT,
                "bond clean price mismatch: expected 99.25, got {clean_price_pct}"
            );
            assert!(
                (coupon_rate - 0.045).abs() < tolerances::TIGHT,
                "bond coupon mismatch: expected 0.045, got {coupon_rate}"
            );
        }
        other => panic!("expected FixedRateBulletCleanPrice, got {:?}", other),
    }

    let bond_ytm = BondQuote::FixedRateBulletYtm {
        id: QuoteId::new("BOND-CORP-5Y-YTM"),
        currency: Currency::USD,
        issue_date: d(2025, time::Month::January, 15),
        maturity: d(2030, time::Month::January, 15),
        coupon_rate: 0.045,
        convention: BondConventionId::new("USD-CORP"),
        ytm: 0.0475,
    };
    let bond_ytm_json = serde_json::to_string(&bond_ytm).expect("serialize bond ytm");
    let bond_ytm_parsed: BondQuote =
        serde_json::from_str(&bond_ytm_json).expect("deserialize bond ytm");
    match bond_ytm_parsed {
        BondQuote::FixedRateBulletYtm {
            ytm, coupon_rate, ..
        } => {
            assert!(
                (ytm - 0.0475).abs() < tolerances::TIGHT,
                "bond ytm mismatch: expected 0.0475, got {ytm}"
            );
            assert!(
                (coupon_rate - 0.045).abs() < tolerances::TIGHT,
                "bond coupon mismatch: expected 0.045, got {coupon_rate}"
            );
        }
        other => panic!("expected FixedRateBulletYtm, got {:?}", other),
    }

    let bond_zspread = BondQuote::FixedRateBulletZSpread {
        id: QuoteId::new("BOND-CORP-5Y-Z"),
        currency: Currency::USD,
        issue_date: d(2025, time::Month::January, 15),
        maturity: d(2030, time::Month::January, 15),
        coupon_rate: 0.045,
        convention: BondConventionId::new("USD-CORP"),
        z_spread: 0.0120,
    };
    let bond_z_json = serde_json::to_string(&bond_zspread).expect("serialize bond zspread");
    let bond_z_parsed: BondQuote =
        serde_json::from_str(&bond_z_json).expect("deserialize bond zspread");
    match bond_z_parsed {
        BondQuote::FixedRateBulletZSpread { z_spread, .. } => {
            assert!((z_spread - 0.0120).abs() < tolerances::TIGHT);
        }
        other => panic!("expected FixedRateBulletZSpread, got {:?}", other),
    }

    let bond_oas = BondQuote::FixedRateBulletOas {
        id: QuoteId::new("BOND-CORP-5Y-OAS"),
        currency: Currency::USD,
        issue_date: d(2025, time::Month::January, 15),
        maturity: d(2030, time::Month::January, 15),
        coupon_rate: 0.045,
        convention: BondConventionId::new("USD-CORP"),
        oas: 0.0080,
    };
    let bond_oas_json = serde_json::to_string(&bond_oas).expect("serialize bond oas");
    let bond_oas_parsed: BondQuote =
        serde_json::from_str(&bond_oas_json).expect("deserialize bond oas");
    match bond_oas_parsed {
        BondQuote::FixedRateBulletOas { oas, .. } => {
            assert!((oas - 0.0080).abs() < tolerances::TIGHT);
        }
        other => panic!("expected FixedRateBulletOas, got {:?}", other),
    }

    let fx_option = FxQuote::OptionVanilla {
        id: QuoteId::new("EURUSD-CALL-6M"),
        convention: FxOptionConventionId::new("EUR/USD-VANILLA"),
        expiry: d(2025, time::Month::July, 10),
        strike: 1.12,
        option_type: OptionType::Call,
        vol_surface_id: "EURUSD-VOL".into(),
    };
    let fx_option_json = serde_json::to_string(&fx_option).expect("serialize fx option");
    let fx_option_parsed: FxQuote =
        serde_json::from_str(&fx_option_json).expect("deserialize fx option");
    match fx_option_parsed {
        FxQuote::OptionVanilla {
            strike,
            option_type,
            ..
        } => {
            assert!(
                (strike - 1.12).abs() < tolerances::TIGHT,
                "fx option strike mismatch: expected 1.12, got {strike}"
            );
            assert_eq!(option_type, OptionType::Call);
        }
        other => panic!("expected OptionVanilla, got {:?}", other),
    }
}

#[test]
fn quote_rejects_invalid_dates() {
    let infl_bad_date = r#"
    {
      "inflation_swap": {
        "maturity": "2029-13-20",
        "rate": 0.025,
        "index": "US-CPI-U",
        "convention": "USD-CPI"
      }
    }"#;
    assert!(
        serde_json::from_str::<InflationQuote>(infl_bad_date).is_err(),
        "invalid inflation maturity date should be rejected"
    );

    let vol_bad_date = r#"
    {
      "swaption_vol": {
        "expiry": "2025-06-20",
        "maturity": "2030-14-20",
        "strike": 0.045,
        "vol": 0.15,
        "quote_type": "Normal",
        "convention": "USD"
      }
    }"#;
    assert!(
        serde_json::from_str::<VolQuote>(vol_bad_date).is_err(),
        "invalid swaption maturity date should be rejected"
    );
}

#[test]
fn quote_denies_unknown_fields() {
    // All these schemas use `deny_unknown_fields`; validate it on at least one variant each.

    let cds_bad = r#"
    {
      "type": "cds_par_spread",
      "id": "CDS-ACME-5Y",
      "entity": "ACME",
      "convention": { "currency": "USD", "doc_clause": "cr14" },
      "pillar": { "tenor": "5Y" },
      "spread_bp": 100.0,
      "recovery_rate": 0.4,
      "extra": 1
    }"#;
    assert!(serde_json::from_str::<CdsQuote>(cds_bad).is_err());

    let tranche_bad = r#"
    {
      "type": "cds_tranche",
      "id": "CDX-IG-3-7",
      "index": "CDX.NA.IG",
      "attachment": 0.03,
      "detachment": 0.07,
      "maturity": "2029-06-20",
      "upfront_pct": -2.5,
      "running_spread_bp": 500.0,
      "convention": { "currency": "USD", "doc_clause": "cr14" },
      "extra": "nope"
    }"#;
    assert!(serde_json::from_str::<CDSTrancheQuote>(tranche_bad).is_err());

    let infl_bad = r#"
    {
      "inflation_swap": {
        "maturity": "2029-06-20",
        "rate": 0.025,
        "index": "US-CPI-U",
        "convention": "USD-CPI",
        "extra": true
      }
    }"#;
    assert!(serde_json::from_str::<InflationQuote>(infl_bad).is_err());

    let vol_bad = r#"
    {
      "option_vol": {
        "underlying": "SPX",
        "expiry": "2024-12-20",
        "strike": 4500.0,
        "vol": 0.20,
        "option_type": "call",
        "convention": "USD-EQUITY",
        "extra": 123
      }
    }"#;
    assert!(serde_json::from_str::<VolQuote>(vol_bad).is_err());
}

#[test]
fn bond_fx_and_xccy_quote_helpers_preserve_ids_and_bumps() {
    let bond = BondQuote::FixedRateBulletCleanPrice {
        id: QuoteId::new("BOND-HELPER"),
        currency: Currency::USD,
        issue_date: d(2025, time::Month::January, 15),
        maturity: d(2030, time::Month::January, 15),
        coupon_rate: 0.045,
        convention: BondConventionId::new("USD-UST"),
        clean_price_pct: 99.25,
    };
    assert_eq!(bond.id().as_str(), "BOND-HELPER");
    assert!((bond.value() - 99.25).abs() < tolerances::TIGHT);
    match bond.bump_value_bp(10.0) {
        BondQuote::FixedRateBulletCleanPrice {
            clean_price_pct, ..
        } => assert!((clean_price_pct - 99.251).abs() < tolerances::TIGHT),
        other => panic!("expected clean-price bond quote, got {other:?}"),
    }

    let fx = FxQuote::SwapOutright {
        id: QuoteId::new("EURUSD-SWAP"),
        convention: FxConventionId::new("EUR/USD"),
        far_pillar: Pillar::Tenor("3M".parse().expect("valid tenor")),
        near_rate: 1.10,
        far_rate: 1.105,
    };
    assert_eq!(fx.id().as_str(), "EURUSD-SWAP");
    assert!((fx.value() - 1.105).abs() < tolerances::TIGHT);
    match fx.bump_rate_decimal(0.0025) {
        FxQuote::SwapOutright {
            near_rate,
            far_rate,
            ..
        } => {
            assert!((near_rate - 1.1025).abs() < tolerances::TIGHT);
            assert!((far_rate - 1.1075).abs() < tolerances::TIGHT);
        }
        other => panic!("expected FX swap quote, got {other:?}"),
    }

    let xccy = XccyQuote::BasisSwap {
        id: QuoteId::new("EURUSD-XCCY-5Y"),
        convention: XccyConventionId::new("EUR/USD-XCCY"),
        far_pillar: Pillar::Tenor("5Y".parse().expect("valid tenor")),
        basis_spread_bp: 12.5,
        spot_fx: Some(1.08),
    };
    assert_eq!(xccy.id().as_str(), "EURUSD-XCCY-5Y");
    assert!((xccy.value() - 12.5).abs() < tolerances::TIGHT);
    match xccy.bump_spread_decimal(0.0002) {
        XccyQuote::BasisSwap {
            basis_spread_bp,
            spot_fx,
            ..
        } => {
            assert!((basis_spread_bp - 14.5).abs() < tolerances::TIGHT);
            assert_eq!(spot_fx, Some(1.08));
        }
    }
}

#[test]
fn convention_ids_and_doc_clause_aliases_roundtrip() {
    let bond = BondConventionId::from("USD-CORP");
    let fx = FxConventionId::new("EUR/USD");
    let xccy = XccyConventionId::new("EUR/USD-XCCY");

    assert_eq!(bond.as_str(), "USD-CORP");
    assert_eq!(bond.to_string(), "USD-CORP");
    assert_eq!(fx.as_str(), "EUR/USD");
    assert_eq!(xccy.to_string(), "EUR/USD-XCCY");

    let clause = CdsDocClause::from_str("isda_na").expect("alias should parse");
    assert_eq!(clause, CdsDocClause::IsdaNa);
    let short_alias = CdsDocClause::from_str("xr").expect("short alias should parse");
    assert_eq!(short_alias, CdsDocClause::Xr14);
    let err = CdsDocClause::from_str("bad_clause").expect_err("unknown alias should fail");
    assert!(err.contains("Unknown CDS doc clause"));

    let key = CdsConventionKey {
        currency: Currency::USD,
        doc_clause: CdsDocClause::Cr14,
    };
    assert_eq!(key.to_string(), "USD:cr14");
}
