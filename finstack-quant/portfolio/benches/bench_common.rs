//! Shared fixtures for portfolio benchmarks.
//!
//! Include this file in each benchmark via:
//! ```rust
//! #[path = "bench_common.rs"]
//! mod bench_common;
//! ```

#![allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]

use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::{Portfolio, PortfolioBuilder};
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CDSConvention, CdsValuationConvention, CreditDefaultSwap, PayReceive, PremiumLegSpec,
    ProtectionLegSpec,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::{
    CDSOption, CDSOptionParams,
};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranche, TrancheSide,
};
use finstack_quant_valuations::instruments::equity::equity_option::{
    EquityOption, EquityOptionParams,
};
use finstack_quant_valuations::instruments::equity::variance_swap::{
    RealizedVarMethod, VarianceSwap,
};
use finstack_quant_valuations::instruments::equity::Equity;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    InflationLinkedBond, InflationLinkedBondParams,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_quant_valuations::instruments::fx::fx_option::FxOption;
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap;
use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, CollateralType, Repo};
use finstack_quant_valuations::instruments::rates::swaption::{Swaption, SwaptionParams};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::EquityUnderlyingParams;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType, SettlementType};
use rust_decimal_macros::dec;
use std::sync::Arc;
use time::Month;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../valuations/tests/support/test_utils.rs"
    ));
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Returns the shared valuation base date used by portfolio benchmarks.
pub fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Returns the first business-day fixture after the shared base date.
pub fn t1_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).unwrap()
}

/// Returns the shared two-year maturity fixture for portfolio benchmarks.
pub fn maturity_2y() -> Date {
    Date::from_calendar_date(2027, Month::January, 1).unwrap()
}

/// Returns the shared five-year maturity fixture for portfolio benchmarks.
pub fn maturity_5y() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

// ---------------------------------------------------------------------------
// FX provider
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Market context builders
// ---------------------------------------------------------------------------

/// T0 market context (valuation date = 2025-01-01).
pub fn create_market_context() -> MarketContext {
    build_market_context(base_date(), 0.0)
}

/// T1 market context (valuation date = 2025-01-02, rates +10bp vs T0).
///
/// Used by attribution benchmarks to simulate a realistic day-over-day move.
pub fn create_t1_market_context() -> MarketContext {
    // Keep curve base dates at T0 so floating coupons observed on the
    // attribution start date can be resolved when valuing at T1.
    build_market_context(base_date(), 0.001)
}

fn build_market_context(base: Date, rate_shift: f64) -> MarketContext {
    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.96 - rate_shift),
            (5.0, 0.78 - rate_shift),
            (10.0, 0.60 - rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.04 + rate_shift),
            (1.0, 0.042 + rate_shift),
            (5.0, 0.045 + rate_shift),
            (10.0, 0.05 + rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97 - rate_shift),
            (5.0, 0.82 - rate_shift),
            (10.0, 0.65 - rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let gbp_disc = DiscountCurve::builder("GBP-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.95 - rate_shift),
            (5.0, 0.77 - rate_shift),
            (10.0, 0.59 - rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let hazard_knots = [(0.0, 0.01), (1.0, 0.01), (5.0, 0.02), (10.0, 0.025)];
    let hazard = HazardCurve::builder("CORP-HAZARD")
        .base_date(base)
        .knots(hazard_knots)
        .build()
        .unwrap();

    let corr_knots = [
        (0.03, 0.20),
        (0.07, 0.25),
        (0.10, 0.30),
        (0.15, 0.35),
        (0.30, 0.40),
    ];

    // Duplicate curves for CreditIndexData (CDSTranche pricing requires a credit index
    // wrapping the same hazard and base-correlation data).
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

    let inflation = InflationCurve::builder("USD-CPI")
        .base_date(base)
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0), (10.0, 122.0)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let base_corr = BaseCorrelationCurve::builder("CDX-CORR")
        .knots(corr_knots)
        .build()
        .unwrap();

    let equity_vol = VolSurface::from_grid(
        "EQUITY-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[100.0, 120.0, 140.0, 160.0, 180.0],
        &[0.25; 20],
    )
    .unwrap();

    let swaption_vol = VolSurface::from_grid(
        "SWAPTION-VOL",
        &[0.25, 0.5, 1.0, 2.0, 5.0],
        &[0.02, 0.03, 0.04, 0.05, 0.06],
        &[0.20; 25],
    )
    .unwrap();

    let fx_vol = VolSurface::from_grid(
        "FX-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[1.05, 1.10, 1.15, 1.20, 1.25],
        &[0.10; 20],
    )
    .unwrap();

    let cds_spread_vol = VolSurface::from_grid(
        "CDS-SPREAD-VOL",
        &[0.25, 0.5, 1.0, 2.0],
        &[50.0, 100.0, 150.0, 200.0, 300.0],
        &[0.30; 20],
    )
    .unwrap();

    // FX rates shift slightly between T0 and T1
    let fx_shift = rate_shift * 10.0; // small EUR/USD move
    let fx = FxMatrix::new(Arc::new(SimpleFxProvider {
        eur_usd: 1.10 + fx_shift,
        gbp_usd: 1.25 + fx_shift,
        jpy_usd: 0.0067,
    }));
    let aapl_history = ScalarTimeSeries::new(
        "AAPL",
        (-2_i64..=260)
            .map(|offset| {
                (
                    base + time::Duration::days(offset),
                    150.0 + offset as f64 * 0.01,
                )
            })
            .collect(),
        None,
    )
    .unwrap();

    let usd_alias = DiscountCurve::builder("USD")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.96 - rate_shift),
            (5.0, 0.78 - rate_shift),
            (10.0, 0.60 - rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_alias = DiscountCurve::builder("EUR")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.97 - rate_shift),
            (5.0, 0.82 - rate_shift),
            (10.0, 0.65 - rate_shift),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let gbp_alias = DiscountCurve::builder("GBP")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.95 - rate_shift),
            (5.0, 0.77 - rate_shift),
            (10.0, 0.59 - rate_shift),
        ])
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
        .insert_series(aapl_history)
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(150.0))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02))
        .insert_price(
            "AAPL",
            MarketScalar::Price(Money::new(150.0, Currency::USD)),
        )
        .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL_IMPL_VOL", MarketScalar::Unitless(0.25))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.02))
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

// ---------------------------------------------------------------------------
// Portfolio fixture
// ---------------------------------------------------------------------------

/// Build a diverse institutional portfolio with the given number of positions.
///
/// Instrument mix (approximately):
/// - 30% common: deposits, bonds, IRS, equities, equity options, CDS
/// - 20% derivatives: FX spot, repos, swaptions, FX options, CDS options, variance swaps
/// - 50% complex: CDS tranches, ILBs, inflation swaps, CLOs, convertibles
///
/// Remaining slots are filled with deposits.
pub fn create_institutional_portfolio(num_positions: usize) -> Portfolio {
    let base = base_date();
    let mut builder = PortfolioBuilder::new("INSTITUTIONAL_PORTFOLIO")
        .name("Large Investment Organization")
        .base_ccy(Currency::USD)
        .as_of(base);

    for i in 0..5 {
        builder = builder.entity(Entity::new(format!("FUND_{}", i + 1)));
    }

    let common_positions = (num_positions as f64 * 0.30) as usize;
    let positions_per_common = common_positions / 6;

    let derivative_positions = (num_positions as f64 * 0.20) as usize;
    let positions_per_derivative = (derivative_positions / 6).max(2);

    let exotic_positions = num_positions - common_positions - derivative_positions;
    let positions_per_exotic = (exotic_positions / 6).max(2);

    let mut position_id = 0;

    // 1. Deposits
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

    // 2. Bonds
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

    // 4. Equities
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
                100.0 * (i + 1) as f64,
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
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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

    // 7. FX Spot
    for i in 0..positions_per_derivative.min(3) {
        let fx_id = format!("FXSPOT_{}", i);
        let (base_ccy, quote_ccy) = match i % 3 {
            0 => (Currency::EUR, Currency::USD),
            1 => (Currency::GBP, Currency::USD),
            _ => (Currency::USD, Currency::JPY),
        };
        let fx_spot = FxSpot::new(fx_id.clone().into(), base_ccy, quote_ccy);
        let entity_id = format!("FUND_{}", (i % 5) + 1);
        builder = builder.position(
            Position::new(
                format!("POS_{}", position_id),
                entity_id,
                &fx_id,
                Arc::new(fx_spot),
                1_000_000.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
        position_id += 1;
    }

    // 8. Repos
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

    // 9. Swaptions
    for i in 0..positions_per_derivative.min(3) {
        let swaption_id = format!("SWAPTION_{}", i);
        let expiry = Date::from_calendar_date(2026, Month::July, 1).unwrap();
        let swap_end = Date::from_calendar_date(2030, Month::July, 1).unwrap();
        let params = SwaptionParams::payer(
            Money::new(5_000_000.0, Currency::USD),
            0.04,
            expiry,
            expiry,
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
            .instrument_pricing_overrides(Default::default())
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
            rust_decimal::Decimal::new(1, 2),
            base + time::Duration::days(365),
            maturity_5y(),
            Money::new(10_000_000.0, Currency::USD),
        )
        .expect("valid CDS option params");
        let credit_params = CreditParams::new("CORP", 0.40, "CORP-HAZARD");
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

    // 12. Variance Swaps
    for i in 0..positions_per_derivative.min(2) {
        let var_swap_id = format!("VARSWAP_{}", i);
        let var_swap = VarianceSwap::builder()
            .id(var_swap_id.clone().into())
            .underlying_ticker("AAPL".to_string())
            .notional(Money::new(100_000.0, Currency::USD))
            .strike_variance(0.0625)
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

    // 13. CDS Tranches
    for i in 0..positions_per_exotic.min(2) {
        let tranche_id = format!("CDSTRANCHE_{}", i);
        let tranche_params = CDSTrancheParams::equity_tranche(
            "CDX.NA.IG",
            42,
            Money::new(10_000_000.0, Currency::USD),
            maturity_5y(),
            500.0,
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

    // 14. Inflation-Linked Bonds
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

    // 15. Inflation Swaps
    for i in 0..positions_per_exotic.min(2) {
        let infl_swap_id = format!("INFLSWAP_{}", i);
        let infl_swap = InflationSwap::builder()
            .id(infl_swap_id.clone().into())
            .notional(Money::new(10_000_000.0, Currency::USD))
            .start_date(base)
            .maturity(maturity_5y())
            .fixed_rate(rust_decimal::Decimal::try_from(0.02).expect("valid literal"))
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

    // 16. Structured Credit (CLO)
    for i in 0..positions_per_exotic.min(2) {
        let sc_id = format!("CLO_{}", i);
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
        let senior = Tranche::new(
            format!("{}_SENIOR", sc_id),
            0.0,
            100.0,
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

    // 17. Convertible Bonds
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
            instrument_pricing_overrides:
                finstack_quant_valuations::instruments::InstrumentPricingOverrides::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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

    // Fill remaining slots with deposits
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

/// Build a portfolio suited to generic P&L attribution benchmarks.
///
/// CDS tranches are covered by valuation and metrics benchmarks, but the generic
/// attribution path builds intermediate market states that do not preserve
/// credit-index aggregates. Excluding them keeps this benchmark focused on
/// portfolio attribution mechanics across instruments with direct dependencies.
pub fn create_attribution_portfolio(num_positions: usize) -> Portfolio {
    let portfolio = create_institutional_portfolio(num_positions);
    let mut builder = PortfolioBuilder::new(format!("ATTRIBUTION_{}", num_positions))
        .name("Attribution Benchmark Portfolio")
        .base_ccy(portfolio.base_ccy)
        .as_of(portfolio.as_of);

    for entity in portfolio.entities.values().cloned() {
        builder = builder.entity(entity);
    }

    for position in portfolio
        .positions()
        .iter()
        .filter(|position| !position.instrument_id.as_str().starts_with("CDSTRANCHE_"))
        .cloned()
    {
        builder = builder.position(position);
    }

    builder.build().unwrap()
}
