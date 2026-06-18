//! Unit tests for risk metrics (Duration, Z-spread, CS01, YTM).
//!
//! Tests cover:
//! - Duration calculations from dated cashflows
//! - Z-spread solver convergence
//! - CS01 price sensitivity

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_convexity, calculate_tranche_cs01, calculate_tranche_duration,
    calculate_tranche_z_spread,
};
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn flat_discount_curve(rate: f64) -> DiscountCurve {
    let base = base_date();
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
        ])
        .build()
        .unwrap()
}

fn sample_cashflows() -> Vec<(Date, Money)> {
    vec![
        (
            Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            Money::new(60_000.0, Currency::USD),
        ),
        (
            Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            Money::new(40_000.0, Currency::USD),
        ),
    ]
}

#[test]
fn test_tranche_duration_is_true_modified_duration() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let day_count = DayCount::Act365F;
    let shift = 1e-4; // 1bp, matching the calculator
    let mut pv = 0.0;
    let mut shifted_pv = 0.0;
    let mut weighted_pv = 0.0;

    for (date, amount) in &flows {
        let t = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .unwrap();
        let df = curve.df_between_dates(as_of, *date).unwrap();
        let flow_pv = amount.amount() * df;
        pv += flow_pv;
        shifted_pv += flow_pv * (-shift * t).exp();
        weighted_pv += flow_pv * t;
    }

    // True modified duration: -(dP/dy)/P from a 1bp continuous-compounding
    // bump. For continuous compounding this is the Macaulay measure only to
    // first order; the calculator must reproduce the bump value exactly.
    let expected_duration = -(shifted_pv - pv) / (pv * shift);
    let macaulay = weighted_pv / pv;
    let duration =
        calculate_tranche_duration(&flows, &curve, as_of, Money::new(pv, Currency::USD)).unwrap();

    assert!(
        (duration - expected_duration).abs() < 1e-10,
        "Duration must equal the 1bp-bump modified duration; got {duration}, expected {expected_duration}"
    );
    // Sanity: for a 1bp continuous bump the modified duration is close to but
    // not identical to Macaulay (second-order convexity effect).
    assert!(
        (duration - macaulay).abs() < 1e-3,
        "modified duration {duration} should be near Macaulay {macaulay}"
    );
}

#[test]
fn test_z_spread_zero_for_curve_pv() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let mut pv = 0.0;
    for (date, amount) in &flows {
        let df = curve.df_between_dates(as_of, *date).unwrap();
        pv += amount.amount() * df;
    }

    let z_spread_bps =
        calculate_tranche_z_spread(&flows, &curve, Money::new(pv, Currency::USD), as_of).unwrap();

    assert!(
        z_spread_bps.abs() < 0.1,
        "Z-spread should be near 0 for curve-implied PV"
    );
}

#[test]
fn test_cs01_negative_for_long_position() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    let cs01 = calculate_tranche_cs01(&flows, &curve, 0.0, as_of).unwrap();
    assert!(
        cs01 < 0.0,
        "CS01 should be negative for a long position (wider spreads reduce PV), got {}",
        cs01
    );
}

#[test]
fn test_tranche_convexity_matches_analytic() {
    let as_of = base_date();
    let curve = flat_discount_curve(0.05);
    let flows = sample_cashflows();

    // Independent oracle: continuous-compounding convexity Σ(PV_i · t_i²) / PV.
    let day_count = DayCount::Act365F;
    let mut pv = 0.0;
    let mut weighted_t2 = 0.0;
    for (date, amount) in &flows {
        let t = day_count
            .year_fraction(as_of, *date, DayCountContext::default())
            .unwrap();
        let df = curve.df_between_dates(as_of, *date).unwrap();
        let flow_pv = amount.amount() * df;
        pv += flow_pv;
        weighted_t2 += flow_pv * t * t;
    }
    let expected = weighted_t2 / pv;

    let convexity = calculate_tranche_convexity(&flows, &curve, as_of).unwrap();
    assert!(
        convexity > 0.0,
        "convexity should be positive, got {convexity}"
    );
    assert!(
        (convexity - expected).abs() < 1e-3,
        "convexity {convexity} should match analytic {expected}"
    );
}

/// Discount margin (DM) for floating-rate structured-credit tranches.
mod discount_margin_tests {
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        calculate_tranche_discount_margin, AssetPool, DealType, PoolAsset, StructuredCredit,
        Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use time::Month;

    fn closing() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).unwrap()
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2026, Month::January, 1).unwrap()
    }

    fn market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![(0.0, 1.0), (5.0, 0.90)])
            .build()
            .unwrap();
        // Floating tranches need their index forward; flat 4% SOFR-3M.
        let sofr = ForwardCurve::builder("SOFR-3M", 0.25)
            .base_date(closing())
            .day_count(DayCount::Act365F)
            .knots([(0.25, 0.04), (1.0, 0.04), (2.5, 0.04)])
            .build()
            .unwrap();
        MarketContext::new().insert(disc).insert(sofr)
    }

    fn floating_spec() -> FloatingRateSpec {
        FloatingRateSpec {
            index_id: CurveId::new("SOFR-3M"),
            spread_bp: rust_decimal_macros::dec!(150),
            gearing: rust_decimal_macros::dec!(1),
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "nyse".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }
    }

    fn deal(floating_senior: bool) -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity(),
            DayCount::Thirty360,
        ));
        let senior_coupon = if floating_senior {
            TrancheCoupon::Floating(floating_spec())
        } else {
            TrancheCoupon::Fixed { rate: 0.05 }
        };
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(800_000.0, Currency::USD),
                senior_coupon,
                maturity(),
            )
            .unwrap(),
            Tranche::new(
                "EQ",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(200_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .unwrap(),
        ])
        .unwrap();
        StructuredCredit::new_abs("ABS-DM", pool, tranches, closing(), maturity(), "USD-OIS")
            .with_payment_calendar("nyse")
    }

    #[test]
    fn discount_margin_is_zero_at_base_pv() {
        let sc = deal(true);
        let mkt = market();
        let pv = sc.value_tranche("SR", &mkt, closing()).unwrap();
        let dm = calculate_tranche_discount_margin(&sc, "SR", &mkt, closing(), pv).unwrap();
        assert!(dm.abs() < 1e-6, "DM at base PV should be ~0, got {dm}");
    }

    #[test]
    fn discount_margin_sign_tracks_target_price() {
        let sc = deal(true);
        let mkt = market();
        let pv = sc.value_tranche("SR", &mkt, closing()).unwrap();
        // A richer target (higher PV) needs a wider margin (positive DM);
        // a cheaper target (lower PV) needs a tighter margin (negative DM).
        let richer = Money::new(pv.amount() * 1.002, pv.currency());
        let cheaper = Money::new(pv.amount() * 0.998, pv.currency());
        let dm_rich =
            calculate_tranche_discount_margin(&sc, "SR", &mkt, closing(), richer).unwrap();
        let dm_cheap =
            calculate_tranche_discount_margin(&sc, "SR", &mkt, closing(), cheaper).unwrap();
        assert!(dm_rich > 0.0, "richer target -> positive DM, got {dm_rich}");
        assert!(
            dm_cheap < 0.0,
            "cheaper target -> negative DM, got {dm_cheap}"
        );
    }

    #[test]
    fn discount_margin_errors_on_fixed_tranche() {
        let sc = deal(false);
        let mkt = market();
        let pv = sc.value_tranche("SR", &mkt, closing()).unwrap();
        let result = calculate_tranche_discount_margin(&sc, "SR", &mkt, closing(), pv);
        assert!(result.is_err(), "DM on a fixed-rate tranche must error");
    }
}

/// Break-even CDR for structured-credit tranches.
mod breakeven_cdr_tests {
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        calculate_tranche_breakeven_cdr, AssetPool, DealType, PoolAsset, StructuredCredit, Tranche,
        TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use time::Month;

    fn closing() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).unwrap()
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2026, Month::January, 1).unwrap()
    }

    fn market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![(0.0, 1.0), (5.0, 0.90)])
            .build()
            .unwrap();
        MarketContext::new().insert(disc)
    }

    /// 80% senior / 20% equity ABS, no prepayment, 40% recovery: the senior is
    /// loss-remote until cumulative collateral loss exceeds the 20% subordination.
    fn deal() -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity(),
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(800_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .unwrap(),
            Tranche::new(
                "EQ",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(200_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .unwrap(),
        ])
        .unwrap();
        let mut sc =
            StructuredCredit::new_abs("ABS-BE", pool, tranches, closing(), maturity(), "USD-OIS")
                .with_payment_calendar("nyse");
        sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        sc
    }

    fn senior_writedown(sc: &StructuredCredit, mkt: &MarketContext, cdr: f64) -> f64 {
        let mut d = sc.clone();
        d.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
        d.get_tranche_cashflows("SR", mkt, closing())
            .unwrap()
            .total_writedown
            .amount()
    }

    #[test]
    fn breakeven_cdr_brackets_first_senior_writedown() {
        let sc = deal();
        let mkt = market();
        let be = calculate_tranche_breakeven_cdr(&sc, "SR", &mkt, closing()).unwrap();
        assert!(
            be > 0.0 && be < 1.0,
            "break-even CDR should be interior, got {be}"
        );
        // Just above the break-even there is a writedown; just below there is not.
        assert!(
            senior_writedown(&sc, &mkt, be + 0.02) > 1.0,
            "above break-even the senior must take a writedown"
        );
        assert!(
            senior_writedown(&sc, &mkt, (be - 0.02).max(0.0)) <= 1.0,
            "below break-even the senior must be loss-remote"
        );
    }
}

/// Scenario / yield table for structured-credit tranches.
mod scenario_table_tests {
    use finstack_quant_cashflows::builder::{PrepaymentModelSpec, RecoveryModelSpec};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        scenario_table, AssetPool, DealType, PoolAsset, ScenarioGrid, StructuredCredit, Tranche,
        TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use time::Month;

    fn closing() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).unwrap()
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2026, Month::January, 1).unwrap()
    }

    fn market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![(0.0, 1.0), (5.0, 0.90)])
            .build()
            .unwrap();
        MarketContext::new().insert(disc)
    }

    fn deal() -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity(),
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(800_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .unwrap(),
            Tranche::new(
                "EQ",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(200_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .unwrap(),
        ])
        .unwrap();
        let mut sc =
            StructuredCredit::new_abs("ABS-SCN", pool, tranches, closing(), maturity(), "USD-OIS")
                .with_payment_calendar("nyse");
        sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        sc
    }

    #[test]
    fn scenario_table_sweeps_grid_and_is_monotone_in_cdr() {
        let sc = deal();
        let mkt = market();
        let grid = ScenarioGrid {
            cprs: vec![0.0, 0.10],
            cdrs: vec![0.0, 0.50],
            severities: vec![0.60],
        };
        let table = scenario_table(&sc, "SR", &mkt, closing(), &grid).unwrap();

        // 2 CPR x 2 CDR x 1 severity, in CPR-major then CDR order:
        // [0]=(0,0) [1]=(0,0.5) [2]=(0.1,0) [3]=(0.1,0.5).
        assert_eq!(table.cells.len(), 4);
        assert_eq!(table.tranche_id, "SR");

        // Zero-default cells (cdr = 0) are loss-remote with a positive price.
        for cell in [&table.cells[0], &table.cells[2]] {
            assert!(cell.writedown.abs() < 1.0, "no defaults -> no writedown");
            assert!(cell.price > 0.0, "price should be positive");
        }

        // Holding CPR fixed, a higher CDR cannot reduce the writedown.
        assert!(
            table.cells[1].writedown >= table.cells[0].writedown,
            "writedown must be non-decreasing in CDR at cpr=0"
        );
        assert!(
            table.cells[3].writedown >= table.cells[2].writedown,
            "writedown must be non-decreasing in CDR at cpr=0.10"
        );
    }
}

/// Option-adjusted spread (rate/credit/both coupling).
mod oas_tests {
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        calculate_tranche_oas, calculate_tranche_z_spread, AssetPool, DealType, OasConfig,
        PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use time::Month;

    fn closing() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).unwrap()
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2029, Month::January, 1).unwrap()
    }

    fn market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![(0.0, 1.0), (10.0, 0.70)])
            .build()
            .unwrap();
        MarketContext::new().insert(disc)
    }

    /// A steeply curved discount curve (sharp short-end roll-down then a flat
    /// long end): a flat-rate proxy mis-prices it badly, so it cleanly separates
    /// curve-consistent discounting from a single-rate approximation.
    fn steep_market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![
                (0.0, 1.0),
                (0.5, 0.94),
                (1.0, 0.90),
                (2.0, 0.875),
                (5.0, 0.86),
                (10.0, 0.85),
            ])
            .build()
            .unwrap();
        MarketContext::new().insert(disc)
    }

    fn deal() -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.06,
            maturity(),
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(800_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.05 },
                maturity(),
            )
            .unwrap(),
            Tranche::new(
                "EQ",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(200_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .unwrap(),
        ])
        .unwrap();
        let mut sc =
            StructuredCredit::new_abs("ABS-OAS", pool, tranches, closing(), maturity(), "USD-OIS")
                .with_payment_calendar("nyse");
        sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.10);
        sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.02);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        sc
    }

    #[test]
    fn deterministic_oas_matches_z_spread() {
        // With neither dimension stochastic, OAS is a single curve-discounted
        // scenario over the deterministic cashflows — i.e. the z-spread.
        let sc = deal();
        let mkt = market();
        let as_of = closing();
        let pv = sc.value_tranche("SR", &mkt, as_of).unwrap();
        let original = 800_000.0;
        // Quote 2% below the model's own price so the spread is non-trivial.
        let market_price = 0.98 * pv.amount() / original * 100.0;
        let target_pv = Money::new(market_price / 100.0 * original, Currency::USD);

        let cf = sc.get_tranche_cashflows("SR", &mkt, as_of).unwrap();
        let disc = mkt.get_discount(&sc.discount_curve_id).unwrap();
        let z_bps =
            calculate_tranche_z_spread(&cf.cashflows, disc.as_ref(), target_pv, as_of).unwrap();

        let config = OasConfig {
            stochastic_rates: false,
            stochastic_credit: false,
            ..Default::default()
        };
        let oas = calculate_tranche_oas(&sc, "SR", market_price, &mkt, as_of, &config).unwrap();

        assert!(
            (oas.oas - z_bps / 10_000.0).abs() < 1e-4,
            "deterministic OAS {} should equal z-spread {} (decimal)",
            oas.oas,
            z_bps / 10_000.0
        );
    }

    #[test]
    fn zero_vol_stochastic_rates_reproduce_the_curve() {
        // Curve-consistency (no-arbitrage) anchor for the Hull-White discounting:
        // with stochastic rates enabled but zero HW volatility and zero prepay
        // sensitivity, the rate path collapses to the deterministic forward curve
        // and modulates nothing, so discounting must reproduce the curve exactly —
        // OAS equals the z-spread even for a steeply curved discount curve.
        let sc = deal();
        let mkt = steep_market();
        let as_of = closing();
        let pv = sc.value_tranche("SR", &mkt, as_of).unwrap();
        let original = 800_000.0;
        let market_price = 0.98 * pv.amount() / original * 100.0;
        let target_pv = Money::new(market_price / 100.0 * original, Currency::USD);

        let cf = sc.get_tranche_cashflows("SR", &mkt, as_of).unwrap();
        let disc = mkt.get_discount(&sc.discount_curve_id).unwrap();
        let z_bps =
            calculate_tranche_z_spread(&cf.cashflows, disc.as_ref(), target_pv, as_of).unwrap();

        let config = OasConfig {
            num_paths: 4,
            stochastic_rates: true,
            stochastic_credit: false,
            hw_sigma: 0.0,
            prepay_beta: 0.0,
            ..Default::default()
        };
        let oas = calculate_tranche_oas(&sc, "SR", market_price, &mkt, as_of, &config).unwrap();
        assert!(
            (oas.oas - z_bps / 10_000.0).abs() < 1e-4,
            "zero-vol stochastic-rate OAS {} should equal the z-spread {} (decimal) \
             when discounting is curve-consistent",
            oas.oas,
            z_bps / 10_000.0
        );
    }

    #[test]
    fn stochastic_oas_converges_to_market_price() {
        // Rate + credit coupling both on: the solver should still reprice the
        // tranche to the quoted price, with a finite OAS and non-negative MC SE.
        let sc = deal();
        let mkt = market();
        let config = OasConfig {
            num_paths: 64,
            stochastic_rates: true,
            stochastic_credit: true,
            ..Default::default()
        };
        let oas = calculate_tranche_oas(&sc, "SR", 99.0, &mkt, closing(), &config).unwrap();
        assert!(oas.oas.is_finite(), "OAS must be finite, got {}", oas.oas);
        assert!(oas.price_std_error >= 0.0);
        assert!(
            (oas.model_price - 99.0).abs() < 0.5,
            "model price {} should reprice to the 99.0 quote",
            oas.model_price
        );
    }
}
