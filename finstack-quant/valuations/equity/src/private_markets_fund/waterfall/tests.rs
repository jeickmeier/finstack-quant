use super::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::money::Money;
use smallvec::smallvec;
use time::Month;

fn test_currency() -> Currency {
    Currency::USD
}

fn test_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("Valid date"), day)
        .expect("Valid date")
}

#[test]
fn waterfall_spec_builder() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(1.0)
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("Operation succeeded");

    assert_eq!(spec.style, WaterfallStyle::European);
    assert_eq!(spec.tranches.len(), 4);
    assert_eq!(spec.day_count, DayCount::Act365F);
}

#[test]
fn fund_event_creation() {
    let contrib = FundEvent::contribution(
        test_date(2020, 1, 1),
        Money::from((1000000_i64, test_currency())),
    );

    assert_eq!(contrib.kind, FundEventKind::Contribution);

    let distrib = FundEvent::distribution(
        test_date(2025, 1, 1),
        Money::from((1500000_i64, test_currency())),
    );

    assert_eq!(distrib.kind, FundEventKind::Distribution);
}

#[test]
fn simple_waterfall_allocation() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2) // Simple 80/20 split
        .build()
        .expect("Operation succeeded");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::from((1500000_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("Operation succeeded");

    assert!(!ledger.rows.is_empty());

    // Should have return of capital and promote allocation
    let roc_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Return of Capital"))
        .collect();
    assert!(!roc_rows.is_empty());

    // LP should get back their $1M capital first
    let total_lp_roc: f64 = roc_rows.iter().map(|r| r.to_lp.amount()).sum();
    assert!((total_lp_roc - 1000000.0).abs() < 1e-6);
}

#[test]
fn mixed_currency_events_rejected() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, Currency::USD)),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::from((1_500_000_i64, Currency::EUR)),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let err = engine
        .run(&events)
        .expect_err("mixed-currency events must be rejected");
    assert!(matches!(
        err,
        finstack_quant_core::Error::CurrencyMismatch { .. }
    ));
}

#[test]
fn american_waterfall_processes_deals_chronologically() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    // The chronological order of the two deals is the REVERSE of their deal_id
    // order, so this only passes if deals are processed by earliest date rather
    // than by id or hash bucket.
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        )
        .with_deal_id("A_late"),
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        )
        .with_deal_id("Z_early"),
        FundEvent::proceeds(
            test_date(2022, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
            "A_late",
        ),
        FundEvent::proceeds(
            test_date(2021, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
            "Z_early",
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("runs");

    let first_deal = ledger
        .rows
        .iter()
        .find_map(|r| r.deal_id.as_deref())
        .expect("at least one deal-tagged row");
    assert_eq!(first_deal, "Z_early");
}

#[test]
fn american_waterfall_invariant_to_input_order() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let mut events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        )
        .with_deal_id("deal_a"),
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        )
        .with_deal_id("deal_b"),
        FundEvent::proceeds(
            test_date(2022, 1, 1),
            Money::from((1_500_000_i64, test_currency())),
            "deal_a",
        ),
        FundEvent::proceeds(
            test_date(2021, 1, 1),
            Money::from((900_000_i64, test_currency())),
            "deal_b",
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger_a = engine.run(&events).expect("runs");
    events.reverse();
    let ledger_b = engine.run(&events).expect("runs");
    assert_eq!(ledger_a.rows, ledger_b.rows);
}

#[test]
fn validate_waterfall_spec() {
    // Valid spec
    let valid_spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build();
    assert!(valid_spec.is_ok());

    // Invalid spec - promote shares don't sum to 1.0
    let invalid_spec = PeFundWaterfallSpec {
        style: WaterfallStyle::European,
        tranches: smallvec![PeFundWaterfallTranche::PromoteTier {
            hurdle_irr: 0.0,
            lp_share: 0.7,
            gp_share: 0.4, // 0.7 + 0.4 = 1.1 > 1.0
        }],
        clawback: None,
        day_count: DayCount::Act365F,
        catch_up_mode: CatchUpMode::Full,
    };

    assert!(invalid_spec.validate().is_err());
}

/// A waterfall spec that cannot exhaustively allocate a distribution must
/// FAIL, not silently drop the residual.
///
/// Before the fix, `allocate_distribution` discarded whatever
/// `remaining_amount` survived the tranche loop: a plausible
/// ROC + preferred spec (no terminal promote tier) on a distribution
/// exceeding capital + pref silently vanished the super-preferred profit
/// from LP cashflows, PV, IRR, MOIC, DPI and TVPI — with no error.
#[test]
fn incomplete_spec_with_residual_cash_errors_instead_of_dropping_it() {
    // ROC + 8% pref only — no residual-splitting tier.
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .build()
        .expect("spec builds");

    // 1000 in, 1500 out after 4 years: ROC takes 1000, the pref
    // entitlement is ~1000·(1.08⁴ − 1) ≈ 360 — leaving ~140 unallocated.
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2024, 1, 1),
            Money::from((1500_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let err = engine
        .run(&events)
        .expect_err("residual cash the spec cannot allocate must error");
    let msg = err.to_string();
    assert!(
        msg.contains("allocate") || msg.contains("residual") || msg.contains("remain"),
        "error must explain the unallocated residual: {msg}"
    );

    // The same spec WITH a terminal promote tier allocates everything and
    // must keep working.
    let complete = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("complete spec builds");
    let ledger = EquityWaterfallEngine::new(&complete)
        .run(&events)
        .expect("exhaustive spec allocates fully");
    let total: f64 = ledger
        .rows
        .iter()
        .map(|r| r.to_lp.amount() + r.to_gp.amount())
        .sum();
    assert!(
        (total - 1500.0).abs() < 1e-6,
        "exhaustive spec must allocate the full distribution, got {total}"
    );
}

#[test]
fn catchup_precise_reaches_target_split() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(1.0)
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("Operation succeeded");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2024, 1, 1),
            Money::from((280_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("Operation succeeded");

    let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
    let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
    let profit = (total_lp + total_gp) - 100.0;

    // Ensure profit positive and GP share ~20%
    assert!(profit > 0.0);
    let gp_share = if profit.abs() > 1e-9 {
        total_gp / profit
    } else {
        0.0
    };
    assert!((gp_share - 0.20).abs() < 1e-6, "gp_share={}", gp_share);

    // Catch-up row should exist with positive GP allocation (likely ~9)
    let catchup_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Catch-Up") && r.to_gp.amount() > 0.0)
        .collect();
    assert!(!catchup_rows.is_empty());
}

/// a promote tier's hurdle must gate the GP split.
/// Until the LP-net IRR reaches the hurdle, the tier pays the LP at 100%;
/// only the excess is split at lp/gp shares.
#[test]
fn promote_tier_hurdle_gates_gp_split() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.20, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((1400_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("runs");

    let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
    let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();

    // LP must first be brought to the 20% hurdle IRR at 100%:
    // entitlement = 1000 · 1.20^t with t = 366/365 (Act/365F, 2020 leap).
    let t = 366.0 / 365.0;
    let hurdle_entitlement = 1000.0 * 1.20_f64.powf(t);
    let expected_gp = 0.2 * (1400.0 - hurdle_entitlement);

    assert!(
        (total_gp - expected_gp).abs() < 0.1,
        "GP carry must only apply above the hurdle entitlement: \
         expected ~{expected_gp}, got {total_gp}"
    );
    assert!(
        total_gp < 0.2 * 400.0 - 1.0,
        "GP carry {total_gp} must be below the ungated 20% of profit (80)"
    );
    assert!((total_lp + total_gp - 1400.0).abs() < 1e-6, "conservation");
}

/// when the distribution cannot bring the LP to the
/// hurdle IRR, the GP receives nothing from the promote tier.
#[test]
fn promote_tier_below_hurdle_pays_lp_only() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.20, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((1100_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("runs");

    let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
    let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
    assert!(
        total_gp.abs() < 1e-9,
        "GP must receive nothing below the hurdle, got {total_gp}"
    );
    assert!((total_lp - 1100.0).abs() < 1e-6, "LP receives everything");
}

/// multiple promote tiers cascade — each tier's split
/// applies only between its own hurdle and the next tier's hurdle.
#[test]
fn promote_tiers_cascade_at_next_hurdle() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.08, 0.8, 0.2)
        .promote_tier(0.20, 0.7, 0.3)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((2000_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("runs");

    let t = 366.0 / 365.0;
    let entitlement_8 = 1000.0 * 1.08_f64.powf(t);
    let entitlement_20 = 1000.0 * 1.20_f64.powf(t);

    // Tier 1 (80/20) covers only the band where the LP moves from the 8%
    // to the 20% entitlement: distribution band = (E20 − E8)/0.8.
    let tier1_band = (entitlement_20 - entitlement_8) / 0.8;
    let expected_gp_tier1 = 0.2 * tier1_band;
    // Tier 2 (70/30) takes everything above.
    let remaining_after = 2000.0 - 1000.0 - (entitlement_8 - 1000.0) - tier1_band;
    let expected_gp_tier2 = 0.3 * remaining_after;

    let gp_tier1: f64 = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Promote 8.0%"))
        .map(|r| r.to_gp.amount())
        .sum();
    let gp_tier2: f64 = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Promote 20.0%"))
        .map(|r| r.to_gp.amount())
        .sum();

    assert!(
        (gp_tier1 - expected_gp_tier1).abs() < 0.1,
        "tier-1 GP should be ~{expected_gp_tier1}, got {gp_tier1}"
    );
    assert!(
        (gp_tier2 - expected_gp_tier2).abs() < 0.1,
        "tier-2 GP should be ~{expected_gp_tier2}, got {gp_tier2}"
    );

    let total: f64 = ledger
        .rows
        .iter()
        .map(|r| r.to_lp.amount() + r.to_gp.amount())
        .sum();
    assert!((total - 2000.0).abs() < 1e-6, "conservation");
}

/// the ledger's `lp_irr_to_date` must be computed
/// from the LP-net history (contributions + ledger `to_lp`), not from
/// gross fund events that include GP carry.
#[test]
fn lp_irr_to_date_uses_ledger_net_flows() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((1500_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("runs");

    // LP receives 1000 (RoC) + 0.8·500 = 1400 net of GP carry.
    let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
    assert!((total_lp - 1400.0).abs() < 1e-6);

    let t = 366.0 / 365.0;
    let expected_net_irr = 1.4_f64.powf(1.0 / t) - 1.0;
    let gross_irr = 1.5_f64.powf(1.0 / t) - 1.0;

    let last_irr = ledger
        .rows
        .last()
        .and_then(|r| r.lp_irr_to_date)
        .expect("last row has an LP IRR");
    assert!(
        (last_irr - expected_net_irr).abs() < 1e-3,
        "lp_irr_to_date {last_irr} should match the LP-net IRR {expected_net_irr}"
    );
    assert!(
        last_irr < gross_irr - 0.05,
        "lp_irr_to_date {last_irr} must sit well below the gross-event IRR {gross_irr}"
    );
}

#[test]
fn clawback_preserves_early_return_dates_with_every_holdback() {
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((1_200_000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::from((100_000_i64, test_currency())),
        ),
    ];
    for holdback in [0.0, 0.5, 1.0] {
        let spec = PeFundWaterfallSpec::builder()
            .return_of_capital()
            .preferred_irr(0.08)
            .catch_up(1.0)
            .promote_tier(0.08, 0.8, 0.2)
            .clawback(ClawbackSpec {
                holdback_decimal: Some(holdback),
            })
            .build()
            .expect("spec");
        let ledger = EquityWaterfallEngine::new(&spec)
            .run(&events)
            .expect("dated reconciliation");
        let gp: f64 = ledger.rows.iter().map(|row| row.to_gp.amount()).sum();
        let lp: f64 = ledger.rows.iter().map(|row| row.to_lp.amount()).sum();
        assert!((gp - 60_000.0).abs() < 1e-6, "holdback={holdback}, gp={gp}");
        assert!((lp - 1_240_000.0).abs() < 1e-6);
        assert!(
            ledger
                .rows
                .iter()
                .filter(|row| row.tranche.contains("Clawback"))
                .all(|row| row.to_gp.amount() >= -1e-6)
        );
    }
}

#[test]
fn clawback_reconciles_later_capital_and_releases_escrow_to_lp() {
    for later_capital in [40_i64, 90_i64] {
        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::from((100_i64, test_currency())),
            ),
            FundEvent::distribution(
                test_date(2021, 1, 1),
                Money::from((150_i64, test_currency())),
            ),
            FundEvent::contribution(
                test_date(2023, 1, 1),
                Money::from((later_capital, test_currency())),
            ),
        ];
        let expected_gp = (50 - later_capital).max(0) as f64 * 0.2;
        for holdback in [0.0, 0.5, 1.0] {
            let spec = PeFundWaterfallSpec::builder()
                .return_of_capital()
                .promote_tier(0.0, 0.8, 0.2)
                .clawback(ClawbackSpec {
                    holdback_decimal: Some(holdback),
                })
                .build()
                .expect("spec");
            let ledger = EquityWaterfallEngine::new(&spec)
                .run(&events)
                .expect("later-capital reconciliation");
            let gp: f64 = ledger.rows.iter().map(|row| row.to_gp.amount()).sum();
            let lp: f64 = ledger.rows.iter().map(|row| row.to_lp.amount()).sum();
            assert!(
                (gp - expected_gp).abs() < 1e-6,
                "capital={later_capital} holdback={holdback} gp={gp}"
            );
            assert!(
                (gp + lp - 150.0).abs() < 1e-6,
                "escrow must be fully allocated"
            );
        }
    }
}

#[test]
fn dated_hurdle_uses_fixed_rate_npv_with_later_contributions() {
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.08, 0.8, 0.2)
        .build()
        .expect("spec");
    let start = test_date(2020, 1, 1);
    let early = test_date(2021, 1, 1);
    let later = test_date(2024, 1, 1);
    let end = test_date(2025, 1, 1);
    let flows = vec![
        (start, Money::from((-100_i64, test_currency()))),
        (early, Money::from((150_i64, test_currency()))),
        (later, Money::from((-90_i64, test_currency()))),
    ];
    let t = |date: Date| (date - start).whole_days() as f64 / 365.0;
    let expected = (100.0 * 1.08_f64.powf(t(end)) - 150.0 * 1.08_f64.powf(t(end) - t(early))
        + 90.0 * 1.08_f64.powf(t(end) - t(later)))
    .max(0.0);
    let actual = EquityWaterfallEngine::new(&spec)
        .calculate_preferred_amount(0.08, &flows, end)
        .expect("fixed hurdle has a unique additional-payment solution");
    assert!((actual - expected).abs() < 1e-12);
}

#[test]
fn clawback_preserves_dated_partial_capital_and_catchup_modes() {
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((50_i64, test_currency())),
        ),
        FundEvent::contribution(
            test_date(2022, 1, 1),
            Money::from((20_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2023, 1, 1),
            Money::from((150_i64, test_currency())),
        ),
    ];
    for mode in [None, Some(CatchUpMode::Partial), Some(CatchUpMode::Full)] {
        let mut builder = PeFundWaterfallSpec::builder()
            .return_of_capital()
            .preferred_irr(0.08);
        if let Some(mode) = mode {
            builder = builder.catch_up(0.5).catch_up_mode(mode);
        }
        let plain = builder.promote_tier(0.08, 0.8, 0.2).build().expect("spec");
        let baseline = EquityWaterfallEngine::new(&plain)
            .run(&events)
            .expect("dated allocations");
        let expected_gp: f64 = baseline.rows.iter().map(|row| row.to_gp.amount()).sum();
        for holdback in [0.0, 0.5, 1.0] {
            let mut reconciled = plain.clone();
            reconciled.clawback = Some(ClawbackSpec {
                holdback_decimal: Some(holdback),
            });
            let ledger = EquityWaterfallEngine::new(&reconciled)
                .run(&events)
                .expect("reconciled allocations");
            let gp: f64 = ledger.rows.iter().map(|row| row.to_gp.amount()).sum();
            let lp: f64 = ledger.rows.iter().map(|row| row.to_lp.amount()).sum();
            assert!(
                (gp - expected_gp).abs() < 1e-6,
                "mode={mode:?} holdback={holdback}, gp={gp}, expected={expected_gp}"
            );
            assert!((gp + lp - 200.0).abs() < 1e-6);
        }
    }
}

#[test]
fn american_deal_carry_is_reconciled_against_whole_fund_capital() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .clawback(ClawbackSpec {
            holdback_decimal: Some(0.5),
        })
        .build()
        .expect("spec");
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        )
        .with_deal_id("A"),
        FundEvent::distribution(
            test_date(2021, 1, 1),
            Money::from((150_i64, test_currency())),
        )
        .with_deal_id("A"),
        FundEvent::contribution(
            test_date(2023, 1, 1),
            Money::from((90_i64, test_currency())),
        )
        .with_deal_id("B"),
    ];
    let ledger = EquityWaterfallEngine::new(&spec)
        .run(&events)
        .expect("deal allocation and fund clawback");
    assert!(
        ledger
            .rows
            .iter()
            .any(|row| row.deal_id.as_deref() == Some("A") && row.to_gp.amount() > 0.0)
    );
    let gp: f64 = ledger.rows.iter().map(|row| row.to_gp.amount()).sum();
    let lp: f64 = ledger.rows.iter().map(|row| row.to_lp.amount()).sum();
    assert!(gp.abs() < 1e-12);
    assert!((lp - 150.0).abs() < 1e-12);
}

#[test]
fn clawback_fund_end_overdistribution() {
    let claw = ClawbackSpec {
        holdback_decimal: None,
    };

    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .clawback(claw)
        .build()
        .expect("Operation succeeded");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2022, 1, 1),
            Money::from((150_i64, test_currency())),
        ),
        FundEvent::contribution(
            test_date(2023, 1, 1),
            Money::from((90_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("Operation succeeded");

    // Last row should be clawback settlement with negative GP amount
    let last = ledger.rows.last().expect("rows");
    assert!(last.tranche.contains("Clawback Settlement"));
    assert!(last.to_gp.amount() < 0.0);
    // In this scenario, first promote paid GP 10; final profit is 0 so GP should return ~10
    assert!((last.to_gp.amount() + 10.0).abs() < 1e-6);
}

#[test]
fn clawback_no_catchup_does_not_over_release() {
    // No catch-up tranche: GP entitlement on lifetime profit is the
    // promote share of profit *above the preferred return*, not
    // profit × gp_share. The old formula released 10 (= 50 × 0.2); the
    // catch-up-aware replay releases only (50 − pref) × 0.2.
    let claw = ClawbackSpec {
        holdback_decimal: Some(1.0), // all carry held back until settlement
    };

    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .promote_tier(0.0, 0.8, 0.2)
        .clawback(claw)
        .build()
        .expect("spec");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2022, 1, 1),
            Money::from((150_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("run");

    let settlement = ledger
        .rows
        .iter()
        .find(|r| r.tranche.contains("Clawback Settlement"))
        .expect("settlement row");

    // Act365F tenor 2020-01-01 → 2022-01-01 is 731/365 years.
    let t = 731.0 / 365.0;
    let pref = 100.0 * (1.08_f64).powf(t) - 100.0;
    let expected_gp = (50.0 - pref) * 0.2;
    assert!(
        (settlement.to_gp.amount() - expected_gp).abs() < 1e-3,
        "settlement {} should equal promote share above pref {expected_gp}",
        settlement.to_gp.amount()
    );
    assert!(
        settlement.to_gp.amount() < 10.0 - 1e-6,
        "must not over-release to the GP at profit × gp_share (10): {}",
        settlement.to_gp.amount()
    );
}

#[test]
fn partial_catchup_consumes_both_gp_and_lp_shares() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(0.5)
        .promote_tier(0.0, 0.8, 0.2)
        .catch_up_mode(CatchUpMode::Partial)
        .build()
        .expect("spec");
    let events = vec![
        FundEvent::contribution(
            test_date(2021, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2022, 1, 1),
            Money::from((109_i64, test_currency())),
        ),
    ];
    let ledger = EquityWaterfallEngine::new(&spec)
        .run(&events)
        .expect("waterfall");
    let gp: f64 = ledger.rows.iter().map(|row| row.to_gp.amount()).sum();
    let lp: f64 = ledger.rows.iter().map(|row| row.to_lp.amount()).sum();
    assert!((gp - 0.5).abs() < 1e-10, "GP {gp}");
    assert!((gp + lp - 109.0).abs() < 1e-10);
}

#[test]
fn clawback_partial_catchup_does_not_over_release() {
    // Partial catch-up caps the GP's catch-up take at gp_share of each
    // dollar, so lifetime GP entitlement is below the full-catch-up
    // profit × promote-share figure the old settlement formula used.
    let claw = ClawbackSpec {
        holdback_decimal: Some(1.0),
    };

    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catch_up(0.5)
        .promote_tier(0.0, 0.8, 0.2)
        .catch_up_mode(CatchUpMode::Partial)
        .clawback(claw)
        .build()
        .expect("spec");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2022, 1, 1),
            Money::from((120_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("run");

    let settlement = ledger
        .rows
        .iter()
        .find(|r| r.tranche.contains("Clawback Settlement"))
        .expect("settlement row");

    // Replay economics: RoC 100 → pref P = 100·1.08^t − 100 → catch-up
    // takes min(0.5 × remaining, needed) → promote splits the rest 80/20.
    let t = 731.0 / 365.0;
    let pref = 100.0 * (1.08_f64).powf(t) - 100.0;
    let remaining = 20.0 - pref;
    let catchup_amount = (0.2 * pref / (0.5 - 0.2)).min(remaining);
    let catchup_gp = 0.5 * catchup_amount;
    let promote_gp = (remaining - catchup_amount) * 0.2;
    let expected_gp = catchup_gp + promote_gp;
    assert!(
        (settlement.to_gp.amount() - expected_gp).abs() < 1e-3,
        "settlement {} should equal partial-catch-up entitlement {expected_gp}",
        settlement.to_gp.amount()
    );
    // Old formula: profit × first promote gp_share = 20 × 0.2 = 4.0.
    assert!(
        settlement.to_gp.amount() < 4.0 - 1e-6,
        "must not over-release at full-catch-up economics (4.0): {}",
        settlement.to_gp.amount()
    );
}

#[test]
fn clawback_settles_on_the_last_event_date() {
    let claw = ClawbackSpec {
        holdback_decimal: Some(0.1),
    };

    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .clawback(claw)
        .build()
        .expect("Operation succeeded");

    let events = vec![
        FundEvent::contribution(
            test_date(2025, 1, 1),
            Money::from((100_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2025, 6, 15), // 2025Q2
            Money::from((150_i64, test_currency())),
        ),
    ];

    let ledger = EquityWaterfallEngine::new(&spec)
        .run(&events)
        .expect("Operation succeeded");

    // Find clawback settlement row
    let clawback_row = ledger
        .rows
        .iter()
        .find(|r| r.tranche.contains("Clawback Settlement"))
        .expect("Should have clawback settlement");

    assert_eq!(clawback_row.date, test_date(2025, 6, 15));
}

/// Regression test: C16 — PreferredIrr tranche must not double-count same-date LP cash
/// already allocated by earlier tranches (e.g. ReturnOfCapital) within the same distribution.
///
/// Setup: LP contributes $1 000 at t=0. At t=5y the fund returns $1 500.
/// Tranches: ReturnOfCapital → PreferredIrr(8%) → PromoteTier(80/20).
///
/// RoC tranche allocates $1 000 to LP first.  Remaining $500 goes to preferred.
/// 8% IRR over 5y on $1 000 requires ~$1 469.33 total to LP.  The LP has already
/// received $1 000 as RoC, so the preferred tier should award at most ~$469.33 to LP.
///
/// The buggy code omits the $1 000 same-date RoC cash from the IRR solve, making it
/// think the LP has received $0 so far on this date, so it computes ~$1 469 as the
/// preferred requirement — far more than the $500 remaining — and allocates the full
/// $500 to the preferred tier, leaving $0 for the promote tier.
///
/// After the fix the preferred tier is correctly capped and the promote tier receives
/// its share of the remaining profit.
#[test]
fn preferred_irr_does_not_double_count_same_date_roc() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("Valid spec");

    // LP contributes $1 000; fund distributes $1 500 five years later.
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::from((1500_i64, test_currency())),
        ),
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).expect("Run succeeded");

    let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
    let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();

    // (1) Cash conservation: LP + GP must equal the $1 500 distribution exactly.
    let total_distributed = total_lp + total_gp;
    assert!(
        (total_distributed - 1500.0).abs() < 1e-4,
        "Cash not conserved: LP={total_lp:.4}, GP={total_gp:.4}, total={total_distributed:.4}"
    );

    // (2) The promote tier must receive some allocation (i.e. the preferred tier must NOT
    //     consume the entire remaining amount after RoC just because it ignores same-date
    //     RoC cash in the IRR solve).  GP gets 20% of profit above hurdle via promote.
    //     Profit = $500.  At 8% IRR over 5y, preferred ~ $469.  Remaining for promote
    //     = ~$31.  GP share ≈ 20% of ~$31 ≈ $6.  We just assert GP > 0.
    assert!(
        total_gp > 1e-4,
        "GP received nothing — preferred tier consumed all remaining cash (double-count bug still present). GP={total_gp:.6}"
    );

    // (3) Preferred tranche allocation must be strictly less than what the buggy code
    //     would compute.  Without the fix the preferred tier gets the full $500 remaining
    //     (since it thinks LP needs ~$1469 but only $500 is left after RoC).
    //     With the fix it gets less (the correct incremental preferred amount above RoC).
    let preferred_lp: f64 = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Preferred Return"))
        .map(|r| r.to_lp.amount())
        .sum();
    // Buggy value = 500.0 (full remaining).  Correct value < 500.0.
    assert!(
        preferred_lp < 499.99,
        "Preferred tranche consumed full remaining amount — double-count bug likely still present. preferred_lp={preferred_lp:.4}"
    );
}

#[test]
fn non_finite_preferred_irr_rejected_at_build() {
    // Bad hurdle inputs must fail loudly at construction, not mid-solve.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -1.1] {
        let result = PeFundWaterfallSpec::builder().preferred_irr(bad).build();
        assert!(
            matches!(result, Err(finstack_quant_core::Error::Validation(_))),
            "non-finite preferred IRR {bad} must be rejected at build"
        );
    }
}

#[test]
fn out_of_range_catchup_share_rejected_at_build() {
    for bad in [-0.1, 1.5, f64::NAN] {
        let result = PeFundWaterfallSpec::builder()
            .return_of_capital()
            .catch_up(bad)
            .promote_tier(0.0, 0.8, 0.2)
            .build();
        assert!(
            matches!(result, Err(finstack_quant_core::Error::Validation(_))),
            "catch-up GP share {bad} must be rejected at build"
        );
    }
}

#[test]
fn out_of_range_holdback_pct_rejected_at_build() {
    for bad in [-0.1, 1.5, f64::NAN] {
        let result = PeFundWaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .clawback(ClawbackSpec {
                holdback_decimal: Some(bad),
            })
            .build();
        assert!(
            matches!(result, Err(finstack_quant_core::Error::Validation(_))),
            "holdback percentage {bad} must be rejected at build"
        );
    }
}

#[test]
fn american_rejects_events_without_deal_id() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((1_000_000_i64, test_currency())),
        ),
        FundEvent::distribution(
            test_date(2022, 1, 1),
            Money::from((1_500_000_i64, test_currency())),
        ),
    ];

    let error = EquityWaterfallEngine::new(&spec)
        .run(&events)
        .expect_err("American waterfall requires deal-tagged events");
    assert!(error.to_string().contains("deal_id"));
}

#[test]
fn american_tracks_carry_independently_by_deal() {
    let spec = PeFundWaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        )
        .with_deal_id("WINNER"),
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::from((100_i64, test_currency())),
        )
        .with_deal_id("LOSER"),
        FundEvent::proceeds(
            test_date(2021, 1, 1),
            Money::from((150_i64, test_currency())),
            "WINNER",
        ),
        FundEvent::proceeds(
            test_date(2022, 1, 1),
            Money::from((50_i64, test_currency())),
            "LOSER",
        ),
    ];

    let ledger = EquityWaterfallEngine::new(&spec)
        .run(&events)
        .expect("deal-by-deal waterfall runs");
    let winner_gp: f64 = ledger
        .rows
        .iter()
        .filter(|row| row.deal_id.as_deref() == Some("WINNER"))
        .map(|row| row.to_gp.amount())
        .sum();
    let loser_gp: f64 = ledger
        .rows
        .iter()
        .filter(|row| row.deal_id.as_deref() == Some("LOSER"))
        .map(|row| row.to_gp.amount())
        .sum();

    assert!((winner_gp - 10.0).abs() < 1e-9);
    assert!(loser_gp.abs() < 1e-9);
    assert!(ledger.rows.iter().all(|row| row.deal_id.is_some()));
}

#[test]
fn same_date_events_are_order_invariant() {
    // A same-date contribution and distribution must produce the same
    // economics regardless of caller input order: contributions are
    // canonically processed first.
    let spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("spec builds");

    let contribution = FundEvent::contribution(
        test_date(2020, 1, 1),
        Money::from((1_000_000_i64, test_currency())),
    );
    let same_day_call = FundEvent::contribution(
        test_date(2022, 1, 1),
        Money::from((500_000_i64, test_currency())),
    );
    let distribution = FundEvent::distribution(
        test_date(2022, 1, 1),
        Money::from((1_200_000_i64, test_currency())),
    );

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger_a = engine
        .run(&[
            contribution.clone(),
            same_day_call.clone(),
            distribution.clone(),
        ])
        .expect("runs");
    let ledger_b = engine
        .run(&[contribution, distribution, same_day_call])
        .expect("runs");
    assert_eq!(ledger_a.rows, ledger_b.rows);

    // Same-day called capital is at risk before the distribution: the
    // return-of-capital tier covers the full 1.5M of contributions.
    let roc_lp: f64 = ledger_a
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Return of Capital"))
        .map(|r| r.to_lp.amount())
        .sum();
    assert!(
        (roc_lp - 1_200_000.0).abs() < 1e-6,
        "distribution should be consumed by RoC against 1.5M unreturned, got {roc_lp}"
    );
}

#[test]
fn lp_irr_to_date_propagates_invalid_day_count_and_handles_same_sign_flows() {
    let mut spec = PeFundWaterfallSpec::builder()
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .expect("valid spec");

    let engine = EquityWaterfallEngine::new(&spec);
    let same_sign = vec![
        (
            test_date(2020, 1, 1),
            Money::from((-100_i64, test_currency())),
        ),
        (
            test_date(2021, 1, 1),
            Money::from((-50_i64, test_currency())),
        ),
    ];
    assert_eq!(
        engine
            .calculate_lp_irr_to_date(&same_sign)
            .expect("economically undefined IRR yields None"),
        None
    );

    let two_sign = vec![
        (
            test_date(2020, 1, 1),
            Money::from((-100_i64, test_currency())),
        ),
        (
            test_date(2021, 1, 1),
            Money::from((120_i64, test_currency())),
        ),
    ];
    assert!(
        engine
            .calculate_lp_irr_to_date(&two_sign)
            .expect("valid two-sign flows")
            .is_some()
    );

    spec.day_count = DayCount::Thirty360;
    let bad_engine = EquityWaterfallEngine::new(&spec);
    let err = bad_engine
        .calculate_lp_irr_to_date(&two_sign)
        .expect_err("unsupported day_count must not be masked as None");
    assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
}
