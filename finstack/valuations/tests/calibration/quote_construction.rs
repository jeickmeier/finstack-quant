use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::money::Money;
use finstack_core::types::IndexId;
use finstack_valuations::instruments::rates::InflationSwap;
use finstack_valuations::instruments::PayReceive;
use finstack_valuations::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId,
};
use finstack_valuations::market::quotes::cds::CdsQuote;
use finstack_valuations::market::quotes::cds_tranche::CDSTrancheQuote;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::inflation::InflationQuote;
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use finstack_valuations::market::{build_cds_tranche_instrument, CDSTrancheBuildOverrides};
use time::Month;

// Comprehensive quote-to-instrument construction test for V2 calibration engine.
// Verifies that all quote types can be successfully converted to calibration instruments.

use finstack_core::currency::Currency;
use finstack_core::dates::DateExt; // Imports DateExt
use finstack_core::HashMap;
use rust_decimal::Decimal;

// Helper to create date validly
fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).unwrap()
}

#[test]
fn test_all_quote_types_instrument_construction() {
    let base_date = date(2025, Month::February, 5);
    let currency = Currency::USD;

    // 1. Discount Quotes (OIS)
    let discount_quotes = vec![
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: "USD-Deposit".into(),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.05,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: "USD-Deposit".into(),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.051,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-1Y"),
            index: "USD-Deposit".into(),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.052,
        }),
    ];

    // 2. Forward Quotes (FRAs)
    let forward_quotes = vec![MarketQuote::Rates(RateQuote::Fra {
        id: QuoteId::new(format!(
            "FRA-{:?}-{:?}",
            base_date.add_months(3),
            base_date.add_months(6)
        )),
        index: IndexId::new("USD-LIBOR-3M"),
        start: Pillar::Date(base_date.add_months(3)),
        end: Pillar::Date(base_date.add_months(6)),
        rate: 0.052,
    })];

    // 3. Hazard Quotes (CDS) -> Use "NA-HY-Curve" ID to avoid conflict with "NA-HY" Index during bootstrap
    let hazard_quotes = vec![
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!("CDS-{:?}", base_date.add_months(12))),
            entity: "NA-HY-Curve".to_string(), // Matches Hazard Curve ID
            pillar: Pillar::Date(base_date.add_months(12)),
            spread_bp: 100.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!("CDS-{:?}", base_date.add_months(36))),
            entity: "NA-HY-Curve".to_string(),
            pillar: Pillar::Date(base_date.add_months(36)),
            spread_bp: 120.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    // 4. Inflation Quotes (ZCIS)
    let inflation_quotes = vec![
        MarketQuote::Inflation(InflationQuote::InflationSwap {
            id: QuoteId::new("USA-CPI-U-ZCIS-1Y"),
            maturity: base_date.add_months(12),
            rate: 0.02,
            index: "USA-CPI-U".to_string(),
            convention: InflationSwapConventionId::new("USD"),
        }),
        MarketQuote::Inflation(InflationQuote::InflationSwap {
            id: QuoteId::new("USA-CPI-U-ZCIS-5Y"),
            maturity: base_date.add_months(60),
            rate: 0.025,
            index: "USA-CPI-U".to_string(),
            convention: InflationSwapConventionId::new("USD"),
        }),
    ];

    // 5. Base Correlation Quotes (Tranches)
    let correlation_quotes = vec![
        MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new(format!("TR-0-3-{:?}", base_date.add_months(60))),
            index: "NA-HY".to_string(),
            attachment: 0.0,
            detachment: 0.03, // 0-3% Equity
            maturity: base_date.add_months(60),
            upfront_pct: 0.10,
            running_spread_bp: 500.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new(format!("TR-3-7-{:?}", base_date.add_months(60))),
            index: "NA-HY".to_string(),
            attachment: 0.03,
            detachment: 0.07, // 3-7% Mezz
            maturity: base_date.add_months(60),
            upfront_pct: 0.0,
            running_spread_bp: 300.0,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    // Build instruments using current builders to ensure quote compatibility.
    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("forward".to_string(), "USD-LIBOR-3M".to_string());
    curve_ids.insert("credit".to_string(), "NA-HY-Curve".to_string());
    let build_ctx = finstack_valuations::market::BuildCtx::new(base_date, 1_000_000.0, curve_ids);

    for q in &discount_quotes {
        if let MarketQuote::Rates(rq) = q {
            finstack_valuations::market::build_rate_instrument(rq, &build_ctx)
                .expect("discount instrument build");
        }
    }

    for q in &forward_quotes {
        if let MarketQuote::Rates(rq) = q {
            finstack_valuations::market::build_rate_instrument(rq, &build_ctx)
                .expect("forward instrument build");
        }
    }

    for q in &hazard_quotes {
        if let MarketQuote::Cds(cds_q) = q {
            finstack_valuations::market::build_cds_instrument(cds_q, &build_ctx)
                .expect("cds instrument build");
        }
    }

    for q in &inflation_quotes {
        if let MarketQuote::Inflation(_) = q {
            if let MarketQuote::Inflation(InflationQuote::InflationSwap {
                maturity,
                rate,
                index,
                ..
            }) = q
            {
                let swap = InflationSwap::builder()
                    .id(format!("INF-{}", maturity).into())
                    .notional(Money::new(1_000_000.0, currency))
                    .start_date(base_date)
                    .maturity(*maturity)
                    .fixed_rate(Decimal::try_from(*rate).expect("valid decimal"))
                    .inflation_index_id(index.clone().into())
                    .discount_curve_id("USD-OIS".into())
                    .day_count(DayCount::Act365F)
                    .side(PayReceive::Pay)
                    .build()
                    .expect("inflation swap build");
                swap.validate().expect("inflation swap validate");
            }
        }
    }

    for q in &correlation_quotes {
        if let MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            attachment,
            detachment,
            ..
        }) = q
        {
            assert!(detachment > attachment, "detachment must exceed attachment");
            assert!(*detachment <= 1.0, "detachment should be capped at 100%");
        }
        if let MarketQuote::CDSTranche(tranche) = q {
            let overrides = CDSTrancheBuildOverrides::new(40);
            build_cds_tranche_instrument(tranche, &build_ctx, &overrides)
                .expect("cds tranche instrument build");
        }
    }
}
