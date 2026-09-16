//! Instrument-backed collateral: representation, validation and materialization.

use finstack_quant_cashflows::traits::CashflowScheduleSource;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_pool_stats, AssetPool, AssetType, CallExercisePolicy, DealType, InstrumentCollateral,
    InstrumentExerciseOverride, PutExercisePolicy, ReserveInterestDestination, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use time::macros::date;

const CLOSING: Date = date!(2024 - 09 - 01);

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money fixture")
}

fn bond(id: &str, mut bond: Bond) -> Bond {
    bond.id = InstrumentId::new(id);
    bond
}

fn step_up_bond() -> Bond {
    let mut b = Bond::example().expect("example bond");
    b.id = InstrumentId::new("B-STEPUP");
    b.cashflow_spec = CashflowSpec::step_up(
        0.03,
        vec![
            (date!(2027 - 01 - 15), 0.045),
            (date!(2030 - 01 - 15), 0.06),
        ],
        Tenor::semi_annual(),
        DayCount::Thirty360,
    )
    .expect("step-up spec");
    b
}

fn custom_cashflow_bond() -> Bond {
    let mut b = Bond::example().expect("example bond");
    b.id = InstrumentId::new("B-CUSTOM");
    let schedule = b
        .raw_cashflow_schedule(&MarketContext::new(), CLOSING)
        .expect("fixed schedule needs no curves");
    b.custom_cashflows = Some(schedule);
    b
}

/// Every instrument form the plan requires: fixed, floating, callable,
/// amortizing, step-up and custom-cashflow bonds, a delayed-draw term loan
/// and a revolver.
fn full_collateral() -> InstrumentCollateral {
    InstrumentCollateral {
        bonds: vec![
            bond("B-FIX", Bond::example().expect("fixed")),
            bond("B-FRN", Bond::example_floating().expect("floating")),
            bond("B-CALL", Bond::example_callable().expect("callable")),
            bond("B-AMORT", Bond::example_amortizing().expect("amortizing")),
            step_up_bond(),
            custom_cashflow_bond(),
        ],
        term_loans: vec![TermLoan::example_floating_with_ddtl().expect("ddtl loan")],
        revolvers: vec![RevolvingCredit::example().expect("revolver")],
        call_exercise: CallExercisePolicy::FirstCall,
        put_exercise: PutExercisePolicy::Never,
        overrides: vec![InstrumentExerciseOverride {
            id: InstrumentId::new("B-CALL"),
            call: Some(CallExercisePolicy::RefinancingIncentive { threshold_bp: 50.0 }),
            put: None,
        }],
    }
}

fn pool_with(collateral: InstrumentCollateral, currency: Currency) -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::Clo, currency);
    pool.instruments = Some(collateral);
    pool
}

fn tranches() -> TrancheStructure {
    let senior = Tranche::new(
        "SENIOR",
        0.0,
        90.0,
        TrancheSeniority::Senior,
        usd(90_000_000.0),
        TrancheCoupon::Fixed { rate: 0.05 },
        date!(2034 - 01 - 15),
    )
    .expect("senior");
    let equity = Tranche::new(
        "EQUITY",
        90.0,
        100.0,
        TrancheSeniority::Equity,
        usd(10_000_000.0),
        TrancheCoupon::Fixed { rate: 0.0 },
        date!(2034 - 01 - 15),
    )
    .expect("equity");
    TrancheStructure::new(vec![senior, equity]).expect("tranches")
}

#[test]
fn instrument_collateral_round_trips_and_materializes() {
    let pool = pool_with(full_collateral(), Currency::USD);
    let json = serde_json::to_string(&pool).expect("serialize");
    let back: AssetPool = serde_json::from_str(&json).expect("deserialize");
    let collateral = back.instruments.as_ref().expect("instruments retained");
    assert_eq!(collateral.len(), 8);
    assert_eq!(
        collateral.call_policy_for(&InstrumentId::new("B-CALL")),
        CallExercisePolicy::RefinancingIncentive { threshold_bp: 50.0 }
    );
    assert_eq!(
        collateral.call_policy_for(&InstrumentId::new("B-FIX")),
        CallExercisePolicy::FirstCall
    );

    let normalized = back.normalized(CLOSING).expect("normalized");
    assert_eq!(normalized.assets.len(), 8);
    let row = |id: &str| {
        normalized
            .assets
            .iter()
            .find(|a| a.id.as_str() == id)
            .unwrap_or_else(|| panic!("row {id}"))
    };

    // Fixed bond: contractual coupon, no spread, notional balance.
    let fix = row("B-FIX");
    assert!((fix.rate - 0.0425).abs() < 1e-12);
    assert!(fix.spread_bp.is_none() && fix.index_id.is_none());
    assert_eq!(fix.balance, usd(1_000_000.0));
    assert!(matches!(fix.asset_type, AssetType::HighYieldBond { .. }));

    // Floating bond: spread and index recorded for projection.
    let frn = row("B-FRN");
    assert!(frn.spread_bp.is_some() && frn.index_id.is_some());

    // Step-up bond reports its initial coupon.
    assert!((row("B-STEPUP").rate - 0.03).abs() < 1e-12);

    // Delayed-draw loan: draws on or before closing are funded (10M + 5M),
    // commitment carried.
    let ddtl = row("TL-FLOAT-DDTL-7Y");
    assert_eq!(ddtl.balance, usd(15_000_000.0));
    assert_eq!(ddtl.commitment, Some(usd(20_000_000.0)));
    assert!(matches!(ddtl.asset_type, AssetType::FirstLienLoan { .. }));

    // Revolver: drawn balance, commitment, recovery carried.
    let rcf = row("RCF-USD-3Y");
    assert_eq!(rcf.balance, usd(10_000_000.0));
    assert_eq!(rcf.commitment, Some(usd(50_000_000.0)));
    assert!(matches!(rcf.asset_type, AssetType::RevolverLoan { .. }));
    assert_eq!(rcf.recovery_rate, Some(0.0));

    // Raw pool balance agrees with the materialized rows.
    let raw_total = pool.total_balance().expect("raw total").amount();
    let materialized_total: f64 = normalized.assets.iter().map(|a| a.balance.amount()).sum();
    assert!((raw_total - materialized_total).abs() < 1e-6);

    // Undrawn commitment: revolver 40M + DDTL 5M.
    let stats = calculate_pool_stats(&normalized, CLOSING);
    assert!((stats.undrawn_commitment - 45_000_000.0).abs() < 1e-6);
}

#[test]
fn duplicate_ids_are_rejected() {
    let collateral = InstrumentCollateral {
        bonds: vec![Bond::example().expect("a"), Bond::example().expect("b")],
        ..Default::default()
    };
    let err = pool_with(collateral, Currency::USD)
        .normalized(CLOSING)
        .expect_err("duplicate ids");
    assert!(err.to_string().contains("duplicate id"), "{err}");
}

#[test]
fn currency_mismatch_is_rejected() {
    let collateral = InstrumentCollateral {
        revolvers: vec![RevolvingCredit::example().expect("revolver")],
        ..Default::default()
    };
    let err = pool_with(collateral, Currency::EUR)
        .normalized(CLOSING)
        .expect_err("currency mismatch");
    assert!(err.to_string().contains("base currency"), "{err}");
}

#[test]
fn unknown_override_id_is_rejected() {
    let mut collateral = full_collateral();
    collateral.overrides.push(InstrumentExerciseOverride {
        id: InstrumentId::new("NOT-HELD"),
        call: Some(CallExercisePolicy::FirstCall),
        put: None,
    });
    let err = pool_with(collateral, Currency::USD)
        .normalized(CLOSING)
        .expect_err("unknown override");
    assert!(err.to_string().contains("NOT-HELD"), "{err}");
}

#[test]
fn worst_policy_requires_a_quoted_price() {
    let mut collateral = full_collateral();
    collateral.call_exercise = CallExercisePolicy::Worst;
    collateral.overrides.clear();
    let err = pool_with(collateral.clone(), Currency::USD)
        .normalized(CLOSING)
        .expect_err("worst without price");
    assert!(err.to_string().contains("quoted clean price"), "{err}");

    for bond in collateral.bonds.iter_mut() {
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(99.5);
    }
    for loan in collateral.term_loans.iter_mut() {
        loan.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(99.0);
    }
    pool_with(collateral, Currency::USD)
        .normalized(CLOSING)
        .expect("worst with prices");
}

#[test]
fn mixing_representations_is_rejected() {
    let mut pool = pool_with(full_collateral(), Currency::USD);
    pool.assets.push(
        finstack_quant_valuations::instruments::fixed_income::structured_credit::PoolAsset::fixed_rate_bond(
            "A1",
            usd(1.0),
            0.05,
            date!(2030 - 01 - 01),
            DayCount::Thirty360,
        ),
    );
    let err = pool.total_balance().expect_err("assets and instruments");
    assert!(err.to_string().contains("exactly one"), "{err}");
}

#[test]
fn reserve_interest_destination_must_name_an_existing_tranche() {
    let mut pool = pool_with(full_collateral(), Currency::USD);
    pool.reserve_account = usd(5_000_000.0);
    pool.reserve_account_rate = 0.04;
    pool.reserve_interest_destination = ReserveInterestDestination::Tranche {
        tranche_id: InstrumentId::new("MEZZ"),
    };
    let err = pool
        .validate_reserve_config(&tranches())
        .expect_err("unknown tranche");
    assert!(err.to_string().contains("MEZZ"), "{err}");

    pool.reserve_interest_destination = ReserveInterestDestination::Tranche {
        tranche_id: InstrumentId::new("EQUITY"),
    };
    pool.validate_reserve_config(&tranches())
        .expect("equity tranche exists");

    pool.reserve_account_rate = -0.01;
    assert!(pool.validate_reserve_config(&tranches()).is_err());

    // Defaults serialize compactly and read back as the waterfall destination.
    let plain = AssetPool::new("P", DealType::Clo, Currency::USD);
    let json = serde_json::to_string(&plain).expect("serialize");
    assert!(!json.contains("reserve_interest_destination"));
    assert!(!json.contains("instruments"));
    let back: AssetPool = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        back.reserve_interest_destination,
        ReserveInterestDestination::Waterfall
    );
}
