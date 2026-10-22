//! Capital structure builder integration tests.
#![allow(clippy::expect_used, clippy::panic)]

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::FinancialStatementInstrument;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use time::Month;

#[test]
fn test_add_bond() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let builder = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_bond(
                "BOND-001",
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .expect("valid bond");
        model_builder
    };
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "BOND-001");
    assert!(matches!(
        cs.debt_instruments[0].spec,
        FinancialStatementInstrument::Bond(_)
    ));
}

fn swap_in(currency: Currency) -> finstack_quant_statements::Result<ModelBuilder> {
    {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        match model_builder.add_swap(finstack_quant_statements::capital_structure::SwapParams {
            id: ("SWAP-001").into(),
            notional: Money::new(5_000_000.0, currency).expect("valid money fixture"),
            fixed_rate: 0.04,
            start_date: Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
            maturity_date: Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
            discount_curve_id: ("OIS").into(),
            forward_curve_id: ("FWD-3M").into(),
        }) {
            Ok(_) => Ok(model_builder),
            Err(error) => Err(error),
        }
    }
}

#[test]
fn add_swap_rolls_on_the_currency_rates_calendar() {
    for (currency, calendar) in [
        (Currency::USD, "usny"),
        (Currency::EUR, "target2"),
        (Currency::AUD, "auce"),
        (Currency::CAD, "cato"),
        (Currency::CHF, "chzh"),
    ] {
        let model = swap_in(currency)
            .expect("mapped currency")
            .build()
            .expect("valid model");
        let cs = model.capital_structure.expect("capital structure");
        let FinancialStatementInstrument::InterestRateSwap(swap) = &cs.debt_instruments[0].spec
        else {
            panic!("expected a swap");
        };
        assert_eq!(swap.fixed_leg.calendar_id.as_deref(), Some(calendar));
        assert_eq!(swap.float_leg.calendar_id.as_deref(), Some(calendar));
    }
}

#[test]
fn add_swap_rejects_a_currency_without_a_standard_calendar() {
    let error = swap_in(Currency::SEK).expect_err("SEK has no standard rates calendar");
    assert!(
        error.to_string().contains("no standard rates calendar"),
        "unexpected error: {error}"
    );
}

#[test]
fn test_add_swap() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let builder = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_swap(finstack_quant_statements::capital_structure::SwapParams {
                id: ("SWAP-001").into(),
                notional: Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
                fixed_rate: 0.04,
                start_date: start,
                maturity_date: maturity,
                discount_curve_id: ("USD-OIS").into(),
                forward_curve_id: ("USD-SOFR-3M").into(),
            })
            .expect("valid swap");
        model_builder
    };
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "SWAP-001");
    assert!(matches!(
        cs.debt_instruments[0].spec,
        FinancialStatementInstrument::InterestRateSwap(_)
    ));
}

#[test]
fn test_add_multiple_instruments() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let builder = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_bond(
                "BOND-001",
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .expect("valid bond");
        model_builder
            .add_bond(
                "BOND-002",
                Money::new(2_000_000.0, Currency::USD).expect("valid money fixture"),
                0.06,
                issue,
                maturity,
                "USD-OIS",
            )
            .expect("valid bond");
        model_builder
    };
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 2);
}

#[test]
fn test_add_debt() {
    let instrument = FinancialStatementInstrument::RevolvingCredit(
        RevolvingCredit::example().expect("valid revolving credit example"),
    );
    let builder = {
        let mut model_builder = ModelBuilder::new("test");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder.add_debt("RCF-A", instrument);
        model_builder
    };
    let model = builder.build().expect("valid model");

    let cs = model
        .capital_structure
        .as_ref()
        .expect("capital_structure should exist");
    assert_eq!(cs.debt_instruments.len(), 1);

    assert_eq!(cs.debt_instruments[0].id, "RCF-A");
}

// --- Parity: add_bond vs add_bond_with_convention (USD defaults) ---

#[test]
fn parity_add_bond_and_add_bond_with_convention_same_id() {
    use finstack_quant_core::types::Rate;
    use finstack_quant_valuations::instruments::BondConvention;

    let issue = Date::from_calendar_date(2025, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("valid date");

    let model_simple = {
        let mut model_builder = ModelBuilder::new("simple");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_bond(
                "BOND-SIMPLE",
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .expect("valid bond");
        model_builder.build().expect("valid model")
    };

    let model_conv = {
        let mut model_builder = ModelBuilder::new("convention");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_bond_with_convention(
                finstack_quant_statements::capital_structure::BondConventionParams {
                    id: ("BOND-CONV").into(),
                    notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                    coupon_rate: Rate::from_decimal(0.05).expect("valid rate fixture"),
                    issue_date: issue,
                    maturity_date: maturity,
                    convention: BondConvention::UsCorporate,
                    discount_curve_id: ("USD-OIS").into(),
                },
            )
            .expect("valid bond with convention");
        model_builder.build().expect("valid model")
    };

    let cs_simple = model_simple
        .capital_structure
        .as_ref()
        .expect("capital_structure present");
    let cs_conv = model_conv
        .capital_structure
        .as_ref()
        .expect("capital_structure present");

    // Both produce a bond-tagged payload
    assert!(matches!(
        cs_simple.debt_instruments[0].spec,
        FinancialStatementInstrument::Bond(_)
    ));
    assert!(matches!(
        cs_conv.debt_instruments[0].spec,
        FinancialStatementInstrument::Bond(_)
    ));
}

// --- Parity: add_swap vs add_swap_with_conventions (USD defaults) ---

#[test]
fn parity_add_swap_and_add_swap_with_conventions_produce_swap_variant() {
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};

    let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

    let model_simple = {
        let mut model_builder = ModelBuilder::new("simple");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_swap(finstack_quant_statements::capital_structure::SwapParams {
                id: ("SWAP-SIMPLE").into(),
                notional: Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
                fixed_rate: 0.04,
                start_date: start,
                maturity_date: maturity,
                discount_curve_id: ("USD-OIS").into(),
                forward_curve_id: ("USD-SOFR-3M").into(),
            })
            .expect("valid swap");
        model_builder.build().expect("valid model")
    };

    let model_conv = {
        let mut model_builder = ModelBuilder::new("convention");
        model_builder
            .periods("2025Q1..2025Q2", None)
            .expect("valid period range");
        model_builder
            .add_swap_with_conventions(
                finstack_quant_statements::capital_structure::SwapParams {
                    id: ("SWAP-CONV").into(),
                    notional: Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"),
                    fixed_rate: 0.04,
                    start_date: start,
                    maturity_date: maturity,
                    discount_curve_id: ("USD-OIS").into(),
                    forward_curve_id: ("USD-SOFR-3M").into(),
                },
                finstack_quant_statements::capital_structure::SwapConventions {
                    fixed_frequency: Tenor::semi_annual(),
                    fixed_day_count: DayCount::Thirty360,
                    float_frequency: Tenor::quarterly(),
                    float_day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::ModifiedFollowing,
                },
            )
            .expect("valid swap with conventions");
        model_builder.build().expect("valid model")
    };

    let cs_simple = model_simple
        .capital_structure
        .as_ref()
        .expect("capital_structure present");
    let cs_conv = model_conv
        .capital_structure
        .as_ref()
        .expect("capital_structure present");

    assert!(matches!(
        cs_simple.debt_instruments[0].spec,
        FinancialStatementInstrument::InterestRateSwap(_)
    ));
    assert!(matches!(
        cs_conv.debt_instruments[0].spec,
        FinancialStatementInstrument::InterestRateSwap(_)
    ));
}
