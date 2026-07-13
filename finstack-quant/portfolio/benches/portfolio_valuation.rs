//! Portfolio valuation benchmarks.
//!
//! Measures performance of portfolio operations for large institutional portfolios:
//! - Full portfolio valuation (all instrument types)
//! - Scaling with portfolio size
//! - Multi-currency aggregation
//! - Entity-level aggregation
//! - Metrics aggregation
//! - Attribute grouping and filtering
//!
//! Simulates realistic institutional portfolios with:
//! - Multiple entities (funds, accounts)
//! - All major instrument types
//! - Cross-currency positions
//! - Various position sizes

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::metrics::aggregate_metrics;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::{revalue_affected, value_portfolio};
use finstack_quant_portfolio::MarketFactorKey;
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CDSConvention, CdsValuationConvention, CreditDefaultSwap, PayReceive, PremiumLegSpec,
    ProtectionLegSpec,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOptionParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranche, TrancheSide,
};
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOptionParams;
use finstack_quant_valuations::instruments::equity::variance_swap::{
    RealizedVarMethod, VarianceSwap,
};
use finstack_quant_valuations::instruments::equity::Equity;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBondParams;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_quant_valuations::instruments::fx::fx_option::FxOption;
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap;
use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, CollateralType, Repo};
use finstack_quant_valuations::instruments::rates::swaption::Swaption;
use finstack_quant_valuations::instruments::rates::swaption::SwaptionParams;
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::EquityUnderlyingParams;
use finstack_quant_valuations::instruments::RatesCurveKind;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType, SettlementType};
use rust_decimal_macros::dec;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../valuations/tests/support/test_utils.rs"
    ));
}

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_2y() -> Date {
    Date::from_calendar_date(2027, Month::January, 1).unwrap()
}

fn maturity_5y() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

// Simple FX provider for multi-currency portfolios
struct SimpleFxProvider {
    eur_usd: f64,
    gbp_usd: f64,
    jpy_usd: f64,
}

impl FxProvider for SimpleFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        if from == to {
            return Ok(1.0);
        }

        // Convert to USD first, then to target
        let from_to_usd = match from {
            Currency::EUR => self.eur_usd,
            Currency::GBP => self.gbp_usd,
            Currency::JPY => self.jpy_usd,
            _ => 1.0,
        };

        let to_from_usd = match to {
            Currency::EUR => 1.0 / self.eur_usd,
            Currency::GBP => 1.0 / self.gbp_usd,
            Currency::JPY => 1.0 / self.jpy_usd,
            _ => 1.0,
        };

        Ok(from_to_usd * to_from_usd)
    }
}

fn create_market_context() -> MarketContext {
    let base = base_date();

    // USD curves
    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.78), (10.0, 0.60)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([(0.0, 0.04), (1.0, 0.042), (5.0, 0.045), (10.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // EUR curves
    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.82), (10.0, 0.65)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // GBP curves
    let gbp_disc = DiscountCurve::builder("GBP-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.77), (10.0, 0.59)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Hazard curve for credit
    let hazard_knots = [(0.0, 0.0), (1.0, 0.01), (5.0, 0.02), (10.0, 0.025)];
    let hazard = HazardCurve::builder("CORP-HAZARD")
        .base_date(base)
        .knots(hazard_knots)
        .build()
        .unwrap();

    // Inflation curve for inflation-linked bonds and swaps
    let inflation = InflationCurve::builder("USD-CPI")
        .base_date(base)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0), (10.0, 122.0)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Base correlation curve for CDS tranches
    let corr_knots = [
        (0.03, 0.20),
        (0.07, 0.25),
        (0.10, 0.30),
        (0.15, 0.35),
        (0.30, 0.40),
    ];
    let base_corr = BaseCorrelationCurve::builder("CDX-CORR")
        .knots(corr_knots)
        .build()
        .unwrap();

    // CreditIndexData for CDSTranche pricing (requires index hazard + base correlation)
    let hazard_for_index = HazardCurve::builder("CORP-HAZARD")
        .base_date(base)
        .knots(hazard_knots)
        .build()
        .unwrap();
    let base_corr_for_index = BaseCorrelationCurve::builder("CDX-CORR")
        .knots(corr_knots)
        .build()
        .unwrap();
    let credit_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(hazard_for_index))
        .base_correlation_curve(Arc::new(base_corr_for_index))
        .build()
        .unwrap();

    // Vol surfaces for options
    let equity_vol = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[100.0, 120.0, 140.0, 160.0, 180.0],
        &[0.25; 20], // Flat 25% vol
    )
    .unwrap();

    let swaption_vol = VolSurface::from_grid(
        "SWAPTION-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0],
        &[0.02, 0.03, 0.04, 0.05, 0.06],
        &[0.20; 25], // Flat 20% vol
    )
    .unwrap();

    let fx_vol = VolSurface::from_grid(
        "FX-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[1.05, 1.10, 1.15, 1.20, 1.25],
        &[0.10; 20], // Flat 10% FX vol
    )
    .unwrap();

    let cds_spread_vol = VolSurface::from_grid(
        "CDS-SPREAD-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 100.0, 150.0, 200.0, 300.0],
        &[0.30; 20], // Flat 30% spread vol
    )
    .unwrap();

    // FX matrix
    let fx = FxMatrix::new(Arc::new(SimpleFxProvider {
        eur_usd: 1.10,
        gbp_usd: 1.25,
        jpy_usd: 0.0067,
    }));

    // USD curve (aliased for Equity instruments)
    let usd_alias = DiscountCurve::builder("USD")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.78), (10.0, 0.60)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_alias = DiscountCurve::builder("EUR")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.82), (10.0, 0.65)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let gbp_alias = DiscountCurve::builder("GBP")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.77), (10.0, 0.59)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(usd_disc)
        .insert(usd_alias)
        .insert(usd_fwd)
        .insert(eur_disc)
        .insert(eur_alias)
        .insert(gbp_disc)
        .insert(gbp_alias)
        .insert(hazard)
        .insert(inflation)
        .insert(base_corr)
        .insert_surface(equity_vol)
        .insert_surface(swaption_vol)
        .insert_surface(fx_vol)
        .insert_surface(cds_spread_vol)
        .insert_fx(fx)
        // Equity market data
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(150.0))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02))
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(150.0, Currency::USD)),
        )
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
        // Repo collateral prices
        .insert_price(
            "BOND_0_PRICE",
            MarketScalar::Price(Money::new(1_000_000.0, Currency::USD)),
        )
        .insert_price(
            "BOND_1_PRICE",
            MarketScalar::Price(Money::new(1_000_000.0, Currency::USD)),
        )
        .insert_price(
            "BOND_2_PRICE",
            MarketScalar::Price(Money::new(1_000_000.0, Currency::USD)),
        )
        .insert_credit_index("CORP-HAZARD", credit_index)
}

// Create a diverse institutional portfolio
fn create_institutional_portfolio(num_positions: usize) -> finstack_quant_portfolio::Portfolio {
    let base = base_date();
    let mut builder = PortfolioBuilder::new("INSTITUTIONAL_PORTFOLIO")
        .name("Large Investment Organization")
        .base_ccy(Currency::USD)
        .as_of(base);

    // Add entities (funds/accounts)
    for i in 0..5 {
        builder = builder.entity(Entity::new(format!("FUND_{}", i + 1)));
    }

    // Calculate positions per instrument type
    // Common instruments get more weight (30%)
    let common_positions = (num_positions as f64 * 0.30) as usize;
    let positions_per_common = common_positions / 6; // 6 common types

    // Less common derivatives (20%)
    let derivative_positions = (num_positions as f64 * 0.20) as usize;
    let positions_per_derivative = (derivative_positions / 6).max(2); // At least 2 each

    // Exotic/complex instruments (50% split among many types)
    let exotic_positions = num_positions - common_positions - derivative_positions;
    let positions_per_exotic = (exotic_positions / 6).max(2); // At least 2 each

    let mut position_id = 0;

    // === Common Instruments (30% of portfolio) ===

    // 1. Deposits (short-term cash)
    for i in 0..positions_per_common {
        let ccy = match i % 3 {
            0 => Currency::USD,
            1 => Currency::EUR,
            _ => Currency::GBP,
        };
        let discount_curve_id = match ccy {
            Currency::EUR => "EUR-OIS",
            Currency::GBP => "GBP-OIS",
            _ => "USD-OIS",
        };

        let deposit_id = format!("DEPOSIT_{}", i);
        let deposit = Deposit::builder()
            .id(deposit_id.clone().into())
            .notional(Money::new(1_000_000.0 * (i + 1) as f64, ccy))
            .start_date(base)
            .maturity(maturity_2y())
            .day_count(DayCount::Act360)
            .discount_curve_id(discount_curve_id.into())
            .quote_rate_opt(Some(dec!(0.04)))
            .build()
            .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &deposit_id,
                Arc::new(deposit),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 2. Bonds (government and corporate)
    for i in 0..positions_per_common {
        let ccy = if i % 2 == 0 {
            Currency::USD
        } else {
            Currency::EUR
        };
        let discount_curve_id = if ccy == Currency::EUR {
            "EUR-OIS"
        } else {
            "USD-OIS"
        };

        let bond_id = format!("BOND_{}", i);
        let bond = Bond::fixed(
            bond_id.clone(),
            Money::new(1_000_000.0, ccy),
            0.05,
            base,
            maturity_5y(),
            discount_curve_id,
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &bond_id,
                Arc::new(bond),
                (i + 1) as f64,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 3. Interest Rate Swaps
    for i in 0..positions_per_common {
        let swap_id = format!("IRS_{}", i);
        let notional = Money::new(5_000_000.0 * (i + 1) as f64, Currency::USD);
        let swap = finstack_quant_test_utils::usd_irs_swap(
            swap_id.clone(),
            notional,
            0.04,
            base,
            maturity_5y(),
            if i % 2 == 0 {
                PayReceive::Pay
            } else {
                PayReceive::Receive
            },
        )
        .expect("Failed to create swap for benchmark");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &swap_id,
                Arc::new(swap),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 4. Equity (direct holdings)
    for i in 0..positions_per_common {
        let equity_id = format!("EQUITY_{}", i);
        let equity = Equity::new(equity_id.clone(), "AAPL", Currency::USD);

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &equity_id,
                Arc::new(equity),
                100.0 * (i + 1) as f64, // shares as quantity
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 5. Equity Options
    for i in 0..positions_per_common {
        let option_id = format!("OPTION_{}", i);
        let contract_size = 100.0;
        let option_notional = Money::new(contract_size, Currency::USD);
        let option_params =
            EquityOptionParams::new(150.0, maturity_2y(), OptionType::Call, option_notional)
                .with_exercise_style(ExerciseStyle::European)
                .with_settlement(SettlementType::Cash);

        let underlying_params = EquityUnderlyingParams::new("AAPL", "EQUITY-SPOT", Currency::USD)
            .with_dividend_yield("EQUITY-DIVYIELD")
            .with_contract_size(contract_size);

        let option = EquityOption::new(
            option_id.clone(),
            &option_params,
            &underlying_params,
            "USD-OIS".into(),
            "EQUITY-VOL".into(),
        );

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &option_id,
                Arc::new(option),
                (i + 1) as f64,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 6. Credit Default Swaps
    for i in 0..positions_per_common {
        let cds_id = format!("CDS_{}", i);
        let convention = CDSConvention::IsdaNa;
        let premium = PremiumLegSpec {
            start: base,
            end: maturity_5y(),
            frequency: convention.frequency(),
            stub: convention.stub_convention(),
            bdc: convention.business_day_convention(),
            calendar_id: None,
            day_count: convention.day_count(),
            spread_bp: rust_decimal::Decimal::from(100),
            discount_curve_id: "USD-OIS".into(),
        };

        let protection = ProtectionLegSpec {
            credit_curve_id: "CORP-HAZARD".into(),
            recovery_rate: 0.40,
            settlement_delay: convention.settlement_delay(),
        };

        let cds = CreditDefaultSwap {
            id: cds_id.clone().into(),
            notional: Money::new(10_000_000.0, Currency::USD),
            side: if i % 2 == 0 {
                PayReceive::Pay
            } else {
                PayReceive::Receive
            },
            convention,
            premium,
            protection,
            upfront: None,
            doc_clause: None,
            protection_effective_date: None,
            pricing_overrides: Default::default(),
            valuation_convention: CdsValuationConvention::default(),
            attributes: Default::default(),
            margin_spec: None,
        };

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &cds_id,
                Arc::new(cds),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // === Derivative Instruments (20% of portfolio) ===

    // 7. FX Spot (currency pairs)
    for i in 0..positions_per_derivative.min(3) {
        let fx_id = format!("FXSPOT_{}", i);
        let (base, quote) = match i % 3 {
            0 => (Currency::EUR, Currency::USD),
            1 => (Currency::GBP, Currency::USD),
            _ => (Currency::USD, Currency::JPY),
        };

        let fx_spot = FxSpot::new(fx_id.clone().into(), base, quote);

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &fx_id,
                Arc::new(fx_spot),
                1_000_000.0, // Notional in base currency
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 8. Repos (collateralized lending)
    for i in 0..positions_per_derivative.min(3) {
        let repo_id = format!("REPO_{}", i);
        let collateral = CollateralSpec {
            collateral_type: CollateralType::General,
            instrument_id: format!("BOND_{}", i),
            quantity: 1_000_000.0,
            market_value_id: format!("BOND_{}_PRICE", i),
        };

        let repo = Repo::term(
            repo_id.clone(),
            Money::new(5_000_000.0, Currency::USD),
            collateral,
            0.03,
            base,
            maturity_2y(),
            "USD-OIS",
        )
        .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &repo_id,
                Arc::new(repo),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 9. Swaptions (options on swaps)
    for i in 0..positions_per_derivative.min(3) {
        let swaption_id = format!("SWAPTION_{}", i);
        let expiry = Date::from_calendar_date(2025, Month::July, 1).unwrap();
        let swap_start = expiry;
        let swap_end = Date::from_calendar_date(2030, Month::July, 1).unwrap();

        let params = SwaptionParams::payer(
            Money::new(5_000_000.0, Currency::USD),
            0.04,
            expiry,
            swap_start,
            swap_end,
        )
        .expect("valid benchmark swaption params");

        let swaption = Swaption::new_payer(
            swaption_id.clone(),
            &params,
            "USD-OIS",
            "USD-SOFR-3M",
            "SWAPTION-VOL",
        );

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &swaption_id,
                Arc::new(swaption),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 10. FX Options
    for i in 0..positions_per_derivative.min(2) {
        let fx_option_id = format!("FXOPTION_{}", i);
        let fx_option = FxOption::builder()
            .id(fx_option_id.clone().into())
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .strike(1.15)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(maturity_2y())
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .vol_surface_id("FX-VOL".into())
            .pricing_overrides(Default::default())
            .attributes(Attributes::default())
            .build()
            .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &fx_option_id,
                Arc::new(fx_option),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 11. CDS Options
    for i in 0..positions_per_derivative.min(2) {
        let cds_option_id = format!("CDSOPTION_{}", i);

        let option_params = CDSOptionParams::call(
            rust_decimal::Decimal::new(1, 2), // strike = 0.01 (100bp)
            base + time::Duration::days(180), // expiry
            maturity_5y(),                    // CDS maturity
            Money::new(10_000_000.0, Currency::USD),
        )
        .expect("valid CDS option params");

        let credit_params = CreditParams::new(
            "CORP",
            0.40, // recovery
            "CORP-HAZARD",
        );

        let cds_option = CDSOption::new(
            cds_option_id.clone(),
            &option_params,
            &credit_params,
            "USD-OIS",
            "CDS-SPREAD-VOL",
        )
        .expect("valid CDS option");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &cds_option_id,
                Arc::new(cds_option),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 12. Variance Swaps (volatility exposure)
    for i in 0..positions_per_derivative.min(2) {
        let var_swap_id = format!("VARSWAP_{}", i);
        let var_swap = VarianceSwap::builder()
            .id(var_swap_id.clone().into())
            .underlying_ticker("AAPL".to_string())
            .notional(Money::new(100_000.0, Currency::USD))
            .strike_variance(0.0625) // 25% vol squared
            .start_date(base)
            .maturity(maturity_2y())
            .observation_freq(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(
                finstack_quant_valuations::instruments::equity::variance_swap::PayReceive::Receive,
            )
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .attributes(Attributes::default())
            .build()
            .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &var_swap_id,
                Arc::new(var_swap),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // === Complex/Exotic Instruments (50% of portfolio) ===

    // 13. CDS Tranches (structured credit risk)
    for i in 0..positions_per_exotic.min(2) {
        let tranche_id = format!("CDSTRANCHE_{}", i);

        let tranche_params = CDSTrancheParams::equity_tranche(
            "CDX.NA.IG",
            42, // series
            Money::new(10_000_000.0, Currency::USD),
            maturity_5y(),
            500.0, // 500bp running
        );

        let schedule_params = ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        };

        let tranche = CDSTranche::new(
            tranche_id.clone(),
            &tranche_params,
            &schedule_params,
            "USD-OIS",
            "CORP-HAZARD",
            TrancheSide::BuyProtection,
        )
        .expect("Valid tranche parameters");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &tranche_id,
                Arc::new(tranche),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 14. Inflation-Linked Bonds (real yield bonds)
    for i in 0..positions_per_exotic.min(2) {
        let ilb_id = format!("TIPS_{}", i);

        let bond_params = InflationLinkedBondParams::new(
            Money::new(1_000_000.0, Currency::USD),
            0.01,
            base,
            maturity_5y(),
            100.0,
            Tenor::semi_annual(),
            DayCount::Act365F,
        )
        .expect("valid literal coupon");

        let ilb = InflationLinkedBond::new_tips(ilb_id.clone(), &bond_params, "USD-OIS", "USD-CPI");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &ilb_id,
                Arc::new(ilb),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 15. Inflation Swaps (inflation vs fixed)
    for i in 0..positions_per_exotic.min(2) {
        let infl_swap_id = format!("INFLSWAP_{}", i);
        let infl_swap = InflationSwap::builder()
            .id(infl_swap_id.clone().into())
            .notional(Money::new(10_000_000.0, Currency::USD))
            .start_date(base)
            .maturity(maturity_5y())
            .fixed_rate(rust_decimal::Decimal::try_from(0.02).expect("valid literal")) // 2% fixed real rate
            .inflation_index_id("USD-CPI".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(DayCount::Act365F)
            .side(PayReceive::Pay)
            .attributes(Attributes::default())
            .build()
            .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &infl_swap_id,
                Arc::new(infl_swap),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 16. Structured Credit (CLO/ABS/RMBS/CMBS)
    for i in 0..positions_per_exotic.min(2) {
        let sc_id = format!("CLO_{}", i);

        // Create simple pool
        let mut pool = AssetPool::new(sc_id.clone(), DealType::CLO, Currency::USD);
        for j in 0..10 {
            pool.assets.push(PoolAsset::fixed_rate_bond(
                format!("{}_ASSET_{}", sc_id, j),
                Money::new(1_000_000.0, Currency::USD),
                0.06,
                maturity_5y(),
                DayCount::Act360,
            ));
        }

        // Create tranches
        let senior = Tranche::new(
            format!("{}_SENIOR", sc_id),
            0.0,
            100.0, // Must reach 100% for valid structure
            TrancheSeniority::Senior,
            Money::new(10_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.04 },
            maturity_5y(),
        )
        .unwrap();
        let tranches = TrancheStructure::new(vec![senior]).unwrap();

        let sc = StructuredCredit::new_clo(
            sc_id.clone(),
            pool,
            tranches,
            base,
            maturity_5y(),
            "USD-OIS",
        )
        .with_payment_calendar("nyse");

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &sc_id,
                Arc::new(sc),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 17. Convertible Bonds (hybrid debt-equity)
    for i in 0..positions_per_exotic.min(3) {
        let conv_id = format!("CONV_{}", i);
        let conversion_spec = ConversionSpec {
            ratio: Some(10.0),
            price: None,
            policy: ConversionPolicy::Voluntary,
            anti_dilution: AntiDilutionPolicy::None,
            dividend_adjustment: DividendAdjustment::None,
            dilution_events: Vec::new(),
        };

        let fixed_coupon = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: dec!(0.03),
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                freq: Tenor::semi_annual(),

                dc: DayCount::Act365F,

                bdc: BusinessDayConvention::Following,

                calendar_id: "weekends_only".to_string(),

                stub: StubKind::None,

                end_of_month: false,

                payment_lag_days: 0,

                adjust_accrual_dates: false,
            },
        };

        let convertible = ConvertibleBond {
            id: conv_id.clone().into(),
            notional: Money::new(1_000_000.0, Currency::USD),
            issue_date: base,
            maturity: maturity_5y(),
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            conversion: conversion_spec,
            underlying_equity_id: Some("AAPL".to_string()),
            call_put: None,
            soft_call_trigger: None,
            settlement_days: None,
            recovery_rate: None,
            fixed_coupon: Some(fixed_coupon),
            floating_coupon: None,
            pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
            attributes: Default::default(),
        };

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &conv_id,
                Arc::new(convertible),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // Fill remaining positions with deposits if needed
    while position_id < num_positions {
        let i = position_id;
        let deposit_id = format!("DEPOSIT_FILLER_{}", i);
        let deposit = Deposit::builder()
            .id(deposit_id.clone().into())
            .notional(Money::new(100_000.0, Currency::USD))
            .start_date(base)
            .maturity(maturity_2y())
            .day_count(DayCount::Act360)
            .discount_curve_id("USD-OIS".into())
            .quote_rate_opt(Some(dec!(0.04)))
            .build()
            .unwrap();

        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &deposit_id,
                Arc::new(deposit),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    builder.build().unwrap()
}

// ============================================================================
// Portfolio Valuation Benchmarks
// ============================================================================

fn bench_portfolio_valuation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_valuation");
    let market = create_market_context();
    let config = FinstackConfig::default();

    {
        let num_positions = &250;
        let portfolio = create_institutional_portfolio(*num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            num_positions,
            |b, _| {
                b.iter(|| {
                    value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &Default::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Entity Aggregation Benchmarks
// ============================================================================

fn bench_entity_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_entity_aggregation");
    let market = create_market_context();
    let config = FinstackConfig::default();

    {
        let num_positions = &250;
        let portfolio = create_institutional_portfolio(*num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            num_positions,
            |b, _| {
                b.iter(|| {
                    let valuation = value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &Default::default(),
                    )
                    .unwrap();
                    // Access entity aggregates
                    for i in 1..=5 {
                        let _ = valuation.get_entity_value(&format!("FUND_{}", i));
                    }
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Multi-Currency Aggregation Benchmarks
// ============================================================================

fn bench_multicurrency_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_multicurrency");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let portfolio = create_institutional_portfolio(100);

    group.bench_function("100pos_multicurrency", |b| {
        b.iter(|| {
            value_portfolio(
                black_box(&portfolio),
                black_box(&market),
                black_box(&config),
                &Default::default(),
            )
        });
    });
    group.finish();
}

// ============================================================================
// Position Filtering Benchmarks
// ============================================================================

fn bench_position_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_filtering");
    let portfolio = create_institutional_portfolio(250);

    group.bench_function("filter_by_entity", |b| {
        b.iter(|| {
            let _ = portfolio.positions_for_entity(black_box("FUND_1"));
        });
    });

    group.bench_function("iterate_all_positions", |b| {
        b.iter(|| {
            let count = portfolio.positions().len();
            black_box(count);
        });
    });

    group.finish();
}

// ============================================================================
// Metrics Calculation Benchmarks
// ============================================================================

fn bench_portfolio_with_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_with_metrics");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let as_of = base_date();

    {
        let num_positions = 250usize;
        let portfolio = create_institutional_portfolio(num_positions);
        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default()).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    aggregate_metrics(
                        black_box(&valuation),
                        black_box(Currency::USD),
                        black_box(&market),
                        black_box(as_of),
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Scaling Benchmarks
// ============================================================================

fn bench_portfolio_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_scaling");
    let market = create_market_context();
    let config = FinstackConfig::default();

    {
        let num_positions = &250;
        let portfolio = create_institutional_portfolio(*num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            num_positions,
            |b, _| {
                b.iter(|| {
                    value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &Default::default(),
                    )
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// Selective Repricing Benchmarks
// ============================================================================

/// Bench `revalue_affected` under three "changed factor" scenarios that exercise
/// different affected-position fractions:
///
/// - `broad_curve`  — USD-OIS discount curve changed → reprices nearly all instruments
/// - `equity_spot`  — AAPL spot changed → reprices only equity / equity-option positions
/// - `hazard_curve` — CORP-HAZARD changed → reprices CDS / CDS-option positions
fn bench_revalue_affected(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_revalue_affected");
    let market = create_market_context();
    let config = FinstackConfig::default();

    let broad_curve = vec![MarketFactorKey::curve(
        "USD-OIS".into(),
        RatesCurveKind::Discount,
    )];
    let equity_spot = vec![MarketFactorKey::spot("AAPL")];
    let hazard_curve = vec![MarketFactorKey::curve(
        "CORP-HAZARD".into(),
        RatesCurveKind::Credit,
    )];

    let scenarios: &[(&str, &[MarketFactorKey])] = &[
        ("broad_curve", &broad_curve),
        ("equity_spot", &equity_spot),
        ("hazard_curve", &hazard_curve),
    ];

    for num_positions in [250usize] {
        let portfolio = create_institutional_portfolio(num_positions);
        let prior = value_portfolio(&portfolio, &market, &config, &Default::default()).unwrap();

        for (label, changed) in scenarios {
            group.bench_with_input(
                BenchmarkId::new(format!("{}pos", num_positions), label),
                changed,
                |b, changed| {
                    b.iter(|| {
                        revalue_affected(
                            black_box(&portfolio),
                            black_box(&market),
                            black_box(&config),
                            black_box(&Default::default()),
                            black_box(&prior),
                            black_box(changed),
                        )
                        .unwrap()
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_portfolio_valuation,
    bench_entity_aggregation,
    bench_multicurrency_aggregation,
    bench_position_filtering,
    bench_portfolio_with_metrics,
    bench_portfolio_scaling,
    bench_revalue_affected,
);
criterion_main!(benches);
