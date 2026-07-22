//! Integration tests for fully custom payment waterfalls.
//!
//! A deal may replace the synthesized sequential template with an arbitrary
//! [`Waterfall`] via `with_waterfall` (or the `waterfall` JSON field). These
//! tests pin the three contracts that make that safe:
//! - attaching the template itself is an exact identity;
//! - a genuinely different structure changes payment *timing* (config
//!   sensitivity — the input must move the output);
//! - malformed structures fail loudly at construction and at pricing time.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::waterfall::CoverageTrigger;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, AssetType, DealType, PaymentCalculation, PaymentType, PoolAsset, Recipient,
    RecipientType, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    Waterfall, WaterfallTier,
};
use time::Month;

// ============================================================================
// Test Helpers
// ============================================================================

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::October, 5).unwrap()
}

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

/// Five fixed-rate bullet loans, 150M total, no floating index required.
fn create_test_pool() -> AssetPool {
    let mut pool = AssetPool::new("TEST_POOL", DealType::CLO, Currency::USD);
    for i in 0..5 {
        pool.assets.push(PoolAsset {
            day_count: finstack_quant_core::dates::DayCount::Act360,
            id: InstrumentId::new(format!("LOAN_{i}")),
            asset_type: AssetType::FirstLienLoan {
                industry: Some(format!("Industry_{}", i % 3)),
            },
            balance: Money::new(30_000_000.0, Currency::USD),
            rate: 0.08,
            spread_bps: None,
            index_id: None,
            maturity: maturity_date(),
            credit_quality: Some(CreditRating::BB),
            industry: Some(format!("Industry_{}", i % 3)),
            obligor_id: Some(format!("OBLIGOR_{i}")),
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: Some(test_date()),
            smm_override: None,
            mdr_override: None,
            contractual_payment: None,
        });
    }
    pool
}

/// Senior / subordinated / equity stack tiling 0-100.
fn create_test_tranches() -> TrancheStructure {
    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(15_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.15 },
        maturity_date(),
    )
    .expect("equity tranche");

    let sub = Tranche::new(
        "SUB_B",
        10.0,
        25.0,
        TrancheSeniority::Subordinated,
        Money::new(22_500_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.09 },
        maturity_date(),
    )
    .expect("subordinated tranche");

    let senior = Tranche::new(
        "SENIOR_A",
        25.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(112_500_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.06 },
        maturity_date(),
    )
    .expect("senior tranche");

    TrancheStructure::new(vec![equity, sub, senior]).expect("tranche structure")
}

fn create_test_market() -> MarketContext {
    let discount_curve = DiscountCurve::builder("USD_OIS")
        .base_date(test_date())
        .knots(vec![(0.0, 1.0), (0.25, 0.9875), (1.0, 0.95), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("discount curve");
    MarketContext::new().insert(discount_curve)
}

fn create_test_deal() -> StructuredCredit {
    StructuredCredit::new_clo(
        "TEST_CLO_CUSTOM_WF",
        create_test_pool(),
        create_test_tranches(),
        closing_date(),
        maturity_date(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse")
}

/// A by-class waterfall: Class A interest AND principal rank ahead of any
/// Class B payment — inexpressible with the sequential template, which pays
/// all interest before any principal.
fn by_class_waterfall() -> Waterfall {
    Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("a_interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("A_int", "SENIOR_A")),
        )
        .add_tier(
            WaterfallTier::new("a_principal", 2, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("A_prin", "SENIOR_A", None)),
        )
        .add_tier(
            WaterfallTier::new("b_interest", 3, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("B_int", "SUB_B")),
        )
        .add_tier(
            WaterfallTier::new("b_principal", 4, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("B_prin", "SUB_B", None)),
        )
        .add_tier(
            WaterfallTier::new("equity", 5, PaymentType::Residual).add_recipient(Recipient::new(
                "equity_distribution",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("by-class waterfall")
}

/// Template-shaped custom waterfall (all interest before principal) with a
/// configurable SUB_B interest definition: `None` = no interest claim at all.
fn sequential_waterfall_with(sub_b_interest: Option<PaymentCalculation>) -> Waterfall {
    let mut builder = Waterfall::builder(Currency::USD).add_tier(
        WaterfallTier::new("a_interest", 1, PaymentType::Interest)
            .add_recipient(Recipient::tranche_interest("A_int", "SENIOR_A")),
    );
    if let Some(calc) = sub_b_interest {
        builder = builder.add_tier(
            WaterfallTier::new("b_interest", 2, PaymentType::Interest).add_recipient(
                Recipient::new("B_int", RecipientType::Tranche("SUB_B".to_string()), calc),
            ),
        );
    }
    builder
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("A_prin", "SENIOR_A", None))
                .add_recipient(Recipient::tranche_principal("B_prin", "SUB_B", None)),
        )
        .add_tier(
            WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(Recipient::new(
                "equity_distribution",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("sequential waterfall")
}

// ============================================================================
// Identity: custom waterfall equal to the template prices identically
// ============================================================================

#[test]
fn attaching_the_template_waterfall_is_an_exact_identity() {
    let base = create_test_deal();
    let custom = base
        .clone()
        .with_waterfall(base.create_waterfall())
        .expect("template waterfall must validate");
    let market = create_test_market();

    for tranche_id in ["SENIOR_A", "SUB_B", "EQUITY"] {
        let a = base
            .get_tranche_cashflows(tranche_id, &market, test_date())
            .expect("base flows");
        let b = custom
            .get_tranche_cashflows(tranche_id, &market, test_date())
            .expect("custom flows");
        assert_eq!(
            a.cashflows, b.cashflows,
            "total flows must match for {tranche_id}"
        );
        assert_eq!(
            a.interest_flows, b.interest_flows,
            "interest flows must match for {tranche_id}"
        );
        assert_eq!(
            a.principal_flows, b.principal_flows,
            "principal flows must match for {tranche_id}"
        );
    }
}

// ============================================================================
// Config sensitivity: a different structure must move the output
// ============================================================================

#[test]
fn by_class_waterfall_delays_subordinated_interest() {
    let template_deal = create_test_deal();
    let custom_deal = create_test_deal()
        .with_waterfall(by_class_waterfall())
        .expect("by-class waterfall must validate");
    let market = create_test_market();

    let template_b = template_deal
        .get_tranche_cashflows("SUB_B", &market, test_date())
        .expect("template SUB_B flows");
    let custom_b = custom_deal
        .get_tranche_cashflows("SUB_B", &market, test_date())
        .expect("custom SUB_B flows");

    // Under the template, all note interest is paid before any principal, so
    // SUB_B receives interest at the first payment date. Under the by-class
    // waterfall, SENIOR_A's untargeted principal tier sweeps every remaining
    // dollar until A retires, so SUB_B's first interest arrives strictly later.
    let first_interest = |flows: &finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows| {
        flows
            .interest_flows
            .iter()
            .find(|(_, m)| m.amount() > 0.0)
            .map(|(d, _)| *d)
    };

    let template_first = first_interest(&template_b).expect("template SUB_B receives interest");
    // `None` is the even stronger outcome: A never retires early enough for B
    // to see any interest at all.
    if let Some(custom_first) = first_interest(&custom_b) {
        assert!(
            custom_first > template_first,
            "by-class waterfall must delay SUB_B interest: template {template_first}, \
             custom {custom_first}"
        );
    }

    // Timing, not lifetime totals, is the correct yardstick: assert the
    // structural change moved per-period allocation at the first payment date.
    let template_first_amount = template_b
        .interest_flows
        .iter()
        .find(|(d, _)| *d == template_first)
        .map(|(_, m)| m.amount())
        .unwrap_or(0.0);
    let custom_first_amount = custom_b
        .interest_flows
        .iter()
        .find(|(d, _)| *d == template_first)
        .map(|(_, m)| m.amount())
        .unwrap_or(0.0);
    assert_ne!(
        template_first_amount, custom_first_amount,
        "the waterfall change must alter SUB_B's first-period interest"
    );
}

// ============================================================================
// Claim definition (F3): the waterfall spec defines what interest is OWED
// ============================================================================

#[test]
fn capped_interest_defines_the_claim_and_never_defers() {
    let market = create_test_market();
    let uncapped_deal = create_test_deal()
        .with_waterfall(sequential_waterfall_with(Some(
            PaymentCalculation::TrancheInterest {
                tranche_id: "SUB_B".to_string(),
                rounding: None,
            },
        )))
        .expect("uncapped custom waterfall");
    let capped_deal = create_test_deal()
        .with_waterfall(sequential_waterfall_with(Some(
            PaymentCalculation::CappedTrancheInterest {
                tranche_id: "SUB_B".to_string(),
                cap_rate: 0.04,
                rounding: None,
            },
        )))
        .expect("capped custom waterfall");

    let uncapped = uncapped_deal
        .get_tranche_cashflows("SUB_B", &market, test_date())
        .expect("uncapped flows");
    let capped = capped_deal
        .get_tranche_cashflows("SUB_B", &market, test_date())
        .expect("capped flows");

    // First-period interest scales exactly by cap/coupon = 0.04/0.09: same
    // balance, same accrual, only the claimed rate differs.
    let first = |f: &finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows| {
        f.interest_flows
            .iter()
            .find(|(_, m)| m.amount() > 0.0)
            .map(|(d, m)| (*d, m.amount()))
            .expect("SUB_B receives interest")
    };
    let (d_u, i_u) = first(&uncapped);
    let (d_c, i_c) = first(&capped);
    assert_eq!(d_u, d_c, "cap must not change the payment date");
    assert!(
        (i_c / i_u - 0.04 / 0.09).abs() < 1e-9,
        "capped claim must accrue at the cap rate: uncapped {i_u}, capped {i_c}"
    );

    // The capped-off coupon is NOT owed: it must not appear as a deferred
    // claim. (Before the spec-derived claim seam, Step 5 recorded the full
    // coupon as due and booked the capped-off portion as a phantom deferral.)
    assert!(
        capped.deferred_flows.iter().all(|(_, m)| m.amount() == 0.0),
        "capped-off interest must not defer, got {:?}",
        capped.deferred_flows
    );
}

#[test]
fn tranche_without_interest_recipient_owes_nothing() {
    let market = create_test_market();
    let deal = create_test_deal()
        .with_waterfall(sequential_waterfall_with(None))
        .expect("interest-less custom waterfall");

    let flows = deal
        .get_tranche_cashflows("SUB_B", &market, test_date())
        .expect("SUB_B flows");

    assert!(
        flows.interest_flows.iter().all(|(_, m)| m.amount() == 0.0),
        "a tranche with no interest recipient must receive no interest"
    );
    assert!(
        flows.deferred_flows.iter().all(|(_, m)| m.amount() == 0.0),
        "a tranche with no interest claim must not accrue deferrals"
    );
    assert!(
        flows.cashflows.iter().any(|(_, m)| m.amount() > 0.0),
        "the principal-only tranche must still receive principal"
    );
}

#[test]
fn duplicate_interest_recipient_is_rejected() {
    let waterfall = Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest_1", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("B_int_1", "SUB_B")),
        )
        .add_tier(
            WaterfallTier::new("interest_2", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("B_int_2", "SUB_B")),
        )
        .build()
        .expect("build");

    let err = create_test_deal()
        .with_waterfall(waterfall)
        .expect_err("two interest recipients for one tranche must be rejected");
    assert!(
        err.to_string().contains("exactly one recipient"),
        "error should explain the claim ambiguity, got: {err}"
    );
}

// ============================================================================
// OAS rate path: allocation rides the same shifted coupon as recording
// ============================================================================

#[test]
fn executor_allocates_floating_interest_on_the_shifted_path() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        execute_waterfall, WaterfallContext,
    };

    let float_tranche = Tranche::new(
        "FLOAT",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Floating(finstack_quant_cashflows::builder::FloatingRateSpec {
            index_id: finstack_quant_core::types::CurveId::new("SOFR-3M".to_string()),
            spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
            gearing: rust_decimal::Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: finstack_quant_core::dates::Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }),
        maturity_date(),
    )
    .expect("floating tranche");
    let tranches = TrancheStructure::new(vec![float_tranche]).expect("structure");
    let pool = AssetPool::new("POOL", DealType::CLO, Currency::USD);

    let forward_curve = ForwardCurve::builder("SOFR-3M", 0.25)
        .base_date(test_date())
        .knots(vec![(0.0, 0.05), (5.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("forward curve");
    let market = MarketContext::new().insert(forward_curve).insert_series(
        ScalarTimeSeries::new("FIXING:SOFR-3M", vec![(test_date(), 0.05)], None).expect("fixings"),
    );

    let waterfall = Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("F_int", "FLOAT")),
        )
        .build()
        .expect("waterfall");

    // Future accrual period, so the coupon projects off the forward curve.
    let period_start = Date::from_calendar_date(2026, Month::January, 5).expect("date");
    let payment_date = Date::from_calendar_date(2026, Month::April, 6).expect("date");
    let run = |shift: f64| {
        let ctx = WaterfallContext {
            available_cash: Money::new(10_000_000.0, Currency::USD),
            interest_collections: Money::new(10_000_000.0, Currency::USD),
            principal_collections: Money::new(0.0, Currency::USD),
            payment_date,
            period_start,
            valuation_date: test_date(),
            pool_balance: Money::new(100_000_000.0, Currency::USD),
            market: &market,
            tranche_balances: None,
            asset_balances: None,
            deferred_interest: None,
            reserve_balance: Money::new(0.0, Currency::USD),
            restricted_cash: Money::new(0.0, Currency::USD),
            recovery_proceeds: Money::new(0.0, Currency::USD),
            floating_rate_shift: shift,
        };
        let dist = execute_waterfall(&waterfall, &tranches, &pool, ctx).expect("execute");
        dist.distributions
            .get(&RecipientType::Tranche("FLOAT".to_string()))
            .map(|m| m.amount())
            .unwrap_or(0.0)
    };

    let base = run(0.0);
    let shifted = run(0.02);
    let accrual = finstack_quant_core::dates::DayCount::Act360
        .year_fraction(
            period_start,
            payment_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .expect("accrual");
    let expected_diff = 100_000_000.0 * 0.02 * accrual;
    assert!(
        ((shifted - base) - expected_diff).abs() < 1.0,
        "floating allocation must ride the shifted path: base {base}, shifted {shifted}, \
         expected diff {expected_diff}"
    );
}

// ============================================================================
// Validation: malformed structures fail loudly
// ============================================================================

#[test]
fn custom_waterfall_rejects_unknown_tranche_reference() {
    let waterfall = Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("X_int", "CLASS_X")),
        )
        .build()
        .expect("build");

    let err = create_test_deal()
        .with_waterfall(waterfall)
        .expect_err("unknown tranche reference must be rejected");
    assert!(
        err.to_string().contains("CLASS_X"),
        "error should name the unresolved tranche, got: {err}"
    );
}

#[test]
fn custom_waterfall_rejects_equity_tranche_paid_by_id() {
    let waterfall = Waterfall::builder(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("eq_int", "EQUITY")),
        )
        .build()
        .expect("build");

    let err = create_test_deal()
        .with_waterfall(waterfall)
        .expect_err("equity paid by tranche id must be rejected");
    assert!(
        err.to_string().contains("RecipientType::Equity"),
        "error should direct the user to RecipientType::Equity, got: {err}"
    );
}

#[test]
fn custom_waterfall_conflicts_with_deal_level_fees() {
    let deal = create_test_deal().with_standard_fees();
    let err = deal
        .with_waterfall(by_class_waterfall())
        .expect_err("deal-level fees must conflict with a custom waterfall");
    assert!(
        err.to_string().contains("fees"),
        "error should explain the fees conflict, got: {err}"
    );
}

#[test]
fn fees_attached_after_custom_waterfall_fail_at_pricing_time() {
    // `with_waterfall` cannot see fees added later; the engine re-validates so
    // the JSON path (which never calls `with_waterfall`) is equally protected.
    let deal = create_test_deal()
        .with_waterfall(by_class_waterfall())
        .expect("valid custom waterfall")
        .with_standard_fees();
    let market = create_test_market();

    let err = deal
        .get_tranche_cashflows("SENIOR_A", &market, test_date())
        .expect_err("pricing must reject fees + custom waterfall");
    assert!(
        err.to_string().contains("fees"),
        "error should explain the fees conflict, got: {err}"
    );
}

#[test]
fn duplicate_coverage_trigger_across_waterfall_and_deal_is_rejected() {
    let mut waterfall = by_class_waterfall();
    waterfall = waterfall.add_coverage_trigger(CoverageTrigger {
        tranche_id: "SENIOR_A".to_string(),
        oc_trigger: Some(1.2),
        ic_trigger: None,
    });

    let deal = create_test_deal()
        .with_coverage_triggers(vec![CoverageTrigger {
            tranche_id: "SENIOR_A".to_string(),
            oc_trigger: Some(1.25),
            ic_trigger: None,
        }])
        .expect("deal-level trigger");

    let err = deal
        .with_waterfall(waterfall)
        .expect_err("duplicate trigger for one tranche must be rejected");
    assert!(
        err.to_string().contains("duplicate coverage trigger"),
        "error should name the duplication, got: {err}"
    );
}

// ============================================================================
// Serde: the JSON path (Python/WASM bindings) carries the custom waterfall
// ============================================================================

#[test]
fn custom_waterfall_survives_json_round_trip_and_prices_identically() {
    let deal = create_test_deal()
        .with_waterfall(by_class_waterfall())
        .expect("valid custom waterfall");
    let market = create_test_market();

    let json = serde_json::to_string(&deal).expect("serialize");
    assert!(
        json.contains("\"waterfall\""),
        "serialized deal must carry the custom waterfall"
    );
    let round_tripped: StructuredCredit = serde_json::from_str(&json).expect("deserialize");

    for tranche_id in ["SENIOR_A", "SUB_B", "EQUITY"] {
        let a = deal
            .get_tranche_cashflows(tranche_id, &market, test_date())
            .expect("original flows");
        let b = round_tripped
            .get_tranche_cashflows(tranche_id, &market, test_date())
            .expect("round-tripped flows");
        assert_eq!(
            a.cashflows, b.cashflows,
            "round-tripped deal must price identically for {tranche_id}"
        );
    }

    // A template deal must NOT serialize a waterfall field (identity default).
    let template_json = serde_json::to_string(&create_test_deal()).expect("serialize template");
    assert!(
        !template_json.contains("\"waterfall\":"),
        "template deals must omit the waterfall field"
    );
}
