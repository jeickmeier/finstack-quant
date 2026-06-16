//! Capital structure builder integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::{ModelBuilder, NeedPeriods};
use time::Month;

#[test]
fn test_add_bond() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let builder = ModelBuilder::<NeedPeriods>::new("test")
        .periods("2025Q1..2025Q2", None)
        .expect("valid period range")
        .add_bond(
            "BOND-001",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("valid bond");
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "BOND-001");
    assert_eq!(cs.debt_instruments[0].spec["type"], "bond");
}

#[test]
fn test_add_swap() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let builder = ModelBuilder::<NeedPeriods>::new("test")
        .periods("2025Q1..2025Q2", None)
        .expect("valid period range")
        .add_swap(
            "SWAP-001",
            Money::new(5_000_000.0, Currency::USD),
            0.04,
            start,
            maturity,
            "USD-OIS",
            "USD-SOFR-3M",
        )
        .expect("valid swap");
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "SWAP-001");
    assert_eq!(cs.debt_instruments[0].spec["type"], "interest_rate_swap");
}

#[test]
fn test_add_multiple_instruments() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let builder = ModelBuilder::<NeedPeriods>::new("test")
        .periods("2025Q1..2025Q2", None)
        .expect("valid period range")
        .add_bond(
            "BOND-001",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("valid bond")
        .add_bond(
            "BOND-002",
            Money::new(2_000_000.0, Currency::USD),
            0.06,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("valid bond");
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 2);
}

#[test]
fn test_add_custom_debt() {
    let builder = ModelBuilder::<NeedPeriods>::new("test")
        .periods("2025Q1..2025Q2", None)
        .expect("valid period range")
        .add_custom_debt(
            "TL-A",
            serde_json::json!({
                "type": "term_loan",
                "spec": {
                    "id": "TL-A",
                    "notional": { "amount": 10_000_000.0, "currency": "USD" }
                }
            }),
        );
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "TL-A");
}

// --- Parity: add_bond vs add_bond_with_convention (USD defaults) ---

#[test]
fn parity_add_bond_and_add_bond_with_convention_same_id() {
    use finstack_quant_core::types::Rate;
    use finstack_quant_valuations::instruments::BondConvention;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let model_simple =
        ModelBuilder::<finstack_quant_statements::builder::NeedPeriods>::new("simple")
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range")
            .add_bond(
                "BOND-SIMPLE",
                Money::new(1_000_000.0, Currency::USD),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .expect("valid bond")
            .build()
            .expect("valid model");

    let model_conv =
        ModelBuilder::<finstack_quant_statements::builder::NeedPeriods>::new("convention")
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range")
            .add_bond_with_convention(
                "BOND-CONV",
                Money::new(1_000_000.0, Currency::USD),
                Rate::from_decimal(0.05),
                issue,
                maturity,
                BondConvention::Corporate,
                "USD-OIS",
            )
            .expect("valid bond with convention")
            .build()
            .expect("valid model");

    let cs_simple = model_simple
        .capital_structure
        .as_ref()
        .expect("capital_structure present");
    let cs_conv = model_conv
        .capital_structure
        .as_ref()
        .expect("capital_structure present");

    // Both produce a bond-tagged payload
    assert_eq!(
        cs_simple.debt_instruments[0].spec["type"], "bond",
        "add_bond should produce a bond payload"
    );
    assert_eq!(
        cs_conv.debt_instruments[0].spec["type"], "bond",
        "add_bond_with_convention should produce a bond payload"
    );
}

// --- Parity: add_swap vs add_swap_with_conventions (USD defaults) ---

#[test]
fn parity_add_swap_and_add_swap_with_conventions_produce_swap_variant() {
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};

    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let model_simple =
        ModelBuilder::<finstack_quant_statements::builder::NeedPeriods>::new("simple")
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range")
            .add_swap(
                "SWAP-SIMPLE",
                Money::new(5_000_000.0, Currency::USD),
                0.04,
                start,
                maturity,
                "USD-OIS",
                "USD-SOFR-3M",
            )
            .expect("valid swap")
            .build()
            .expect("valid model");

    let model_conv =
        ModelBuilder::<finstack_quant_statements::builder::NeedPeriods>::new("convention")
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range")
            .add_swap_with_conventions(
                "SWAP-CONV",
                Money::new(5_000_000.0, Currency::USD),
                0.04,
                start,
                maturity,
                "USD-OIS",
                "USD-SOFR-3M",
                Tenor::semi_annual(),
                DayCount::Thirty360,
                Tenor::quarterly(),
                DayCount::Act360,
                BusinessDayConvention::ModifiedFollowing,
            )
            .expect("valid swap with conventions")
            .build()
            .expect("valid model");

    let cs_simple = model_simple
        .capital_structure
        .as_ref()
        .expect("capital_structure present");
    let cs_conv = model_conv
        .capital_structure
        .as_ref()
        .expect("capital_structure present");

    assert_eq!(
        cs_simple.debt_instruments[0].spec["type"], "interest_rate_swap",
        "add_swap should produce an interest_rate_swap payload"
    );
    assert_eq!(
        cs_conv.debt_instruments[0].spec["type"], "interest_rate_swap",
        "add_swap_with_conventions should produce an interest_rate_swap payload"
    );
}
