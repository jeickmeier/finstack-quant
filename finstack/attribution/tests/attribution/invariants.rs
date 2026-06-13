//! Invariant tests for P&L attribution.
//!
//! These tests verify fundamental mathematical invariants that must hold
//! regardless of market conditions or instrument parameters:
//!
//! 1. **Buyer + Seller NPV ≈ 0**: Long and short positions should net to zero
//! 2. **Zero change identity**: No market change → zero P&L
//! 3. **Additivity**: P&L of portfolio = sum of individual P&Ls
//! 4. **Sign conventions**: Rates up → bond value down
//!
//! Uses proptest for property-based testing to discover edge cases.

use finstack_attribution::{attribute_pnl_parallel, ExecutionPolicy};
use finstack_core::config::FinstackConfig;
use finstack_core::currency::Currency;
use finstack_core::dates::{create_date, Date};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::Result;
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Attributes;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::pricer::InstrumentType;
use finstack_valuations::results::ValuationResult;
use proptest::prelude::*;
use std::sync::{Arc, OnceLock};
use time::Month;

/// Helper to build a flat discount curve.
fn build_flat_curve(curve_id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

#[derive(Clone)]
struct ScaledInstrument {
    id: String,
    inner: Arc<dyn Instrument>,
    scale: f64,
}

impl ScaledInstrument {
    fn new(id: &str, inner: Arc<dyn Instrument>, scale: f64) -> Self {
        Self {
            id: id.to_string(),
            inner,
            scale,
        }
    }
}

impl Instrument for ScaledInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        self.inner.key()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("ScaledInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<finstack_valuations::instruments::MarketDependencies> {
        self.inner.market_dependencies()
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        let base = self.inner.value(market, as_of)?;
        Ok(Money::new(base.amount() * self.scale, base.currency()))
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[finstack_valuations::metrics::MetricId],
        _options: finstack_valuations::instruments::PricingOptions,
    ) -> Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

#[derive(Clone)]
struct CompositeInstrument {
    id: String,
    left: Arc<dyn Instrument>,
    right: Arc<dyn Instrument>,
}

impl CompositeInstrument {
    fn new(id: &str, left: Arc<dyn Instrument>, right: Arc<dyn Instrument>) -> Self {
        Self {
            id: id.to_string(),
            left,
            right,
        }
    }
}

impl Instrument for CompositeInstrument {
    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        self.left.key()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        static ATTRS: OnceLock<Attributes> = OnceLock::new();
        ATTRS.get_or_init(Attributes::default)
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        unreachable!("CompositeInstrument::attributes_mut should not be called in tests")
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn market_dependencies(
        &self,
    ) -> finstack_core::Result<finstack_valuations::instruments::MarketDependencies> {
        let mut deps = self.left.market_dependencies()?;
        deps.merge(self.right.market_dependencies()?);
        Ok(deps)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        let left_val = self.left.value(market, as_of)?;
        let right_val = self.right.value(market, as_of)?;
        left_val.checked_add(right_val)
    }

    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        _metrics: &[finstack_valuations::metrics::MetricId],
        _options: finstack_valuations::instruments::PricingOptions,
    ) -> Result<ValuationResult> {
        let value = self.value(market, as_of)?;
        Ok(ValuationResult::stamped(self.id(), as_of, value))
    }
}

// =============================================================================
// Zero Change Identity Tests
// =============================================================================

/// Property: When market conditions are unchanged, total P&L should be approximately
/// equal to carry (time value) only.
///
/// This is a fundamental identity: if nothing changes except time passing,
/// the only P&L should come from carry (theta/accrual).
#[test]
fn test_zero_market_change_identity() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "ZERO-CHANGE-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Same curve at T0 and T1 (no rate change)
    let curve = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let market = MarketContext::new().insert(curve);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market,
        &market,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Rates P&L should be zero (no rate change)
    assert!(
        attribution.rates_curves_pnl.amount().abs() < 1.0,
        "Rates P&L should be near zero with no rate change, got {}",
        attribution.rates_curves_pnl.amount()
    );

    // FX P&L should be zero (no FX change for USD bond)
    assert!(
        attribution.fx_pnl.amount().abs() < 0.01,
        "FX P&L should be zero for USD bond, got {}",
        attribution.fx_pnl.amount()
    );

    // Total P&L should equal carry (approximately)
    let non_carry_pnl = attribution.total_pnl.amount() - attribution.carry.amount();
    assert!(
        non_carry_pnl.abs() < 100.0, // Allow small residual
        "Non-carry P&L should be near zero, got {} (total={}, carry={})",
        non_carry_pnl,
        attribution.total_pnl.amount(),
        attribution.carry.amount()
    );
}

// =============================================================================
// Sign Convention Tests
// =============================================================================

/// Property: For a long bond position, rates increasing should produce negative P&L.
///
/// This is a fundamental fixed income relationship: bond prices fall when rates rise.
#[test]
fn test_rates_pnl_sign_convention() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "SIGN-CONVENTION-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Rates increase from 4% to 5%
    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.05);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Rates P&L should be negative (rates up → bond value down)
    assert!(
        attribution.rates_curves_pnl.amount() < 0.0,
        "Rates P&L should be negative when rates increase, got {}",
        attribution.rates_curves_pnl.amount()
    );
}

/// Property: For a long bond position, rates decreasing should produce positive P&L.
#[test]
fn test_rates_pnl_positive_when_rates_decrease() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "RATES-DOWN-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Rates decrease from 4% to 3%
    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.03);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Rates P&L should be positive (rates down → bond value up)
    assert!(
        attribution.rates_curves_pnl.amount() > 0.0,
        "Rates P&L should be positive when rates decrease, got {}",
        attribution.rates_curves_pnl.amount()
    );
}

// =============================================================================
// Portfolio and Position Invariants
// =============================================================================

#[test]
fn test_long_short_net_zero_invariant() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "LONG-SHORT-BOND",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.05);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let long: Arc<dyn Instrument> = Arc::new(bond);
    let short: Arc<dyn Instrument> =
        Arc::new(ScaledInstrument::new("SHORT-BOND", Arc::clone(&long), -1.0));

    let long_attr = attribute_pnl_parallel(
        &long,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let short_attr = attribute_pnl_parallel(
        &short,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let total_sum = long_attr.total_pnl.amount() + short_attr.total_pnl.amount();
    assert!(
        total_sum.abs() < 0.01,
        "Long + short total P&L should net to zero, got {}",
        total_sum
    );

    let rates_sum = long_attr.rates_curves_pnl.amount() + short_attr.rates_curves_pnl.amount();
    assert!(
        rates_sum.abs() < 0.01,
        "Long + short rates P&L should net to zero, got {}",
        rates_sum
    );

    let carry_sum = long_attr.carry.amount() + short_attr.carry.amount();
    assert!(
        carry_sum.abs() < 0.01,
        "Long + short carry should net to zero, got {}",
        carry_sum
    );
}

#[test]
fn test_portfolio_additivity_invariant() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();

    let bond_a = Bond::fixed(
        "PORTFOLIO-BOND-A",
        Money::new(1_000_000.0, Currency::USD),
        0.04,
        issue,
        create_date(2029, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let bond_b = Bond::fixed(
        "PORTFOLIO-BOND-B",
        Money::new(2_000_000.0, Currency::USD),
        0.06,
        issue,
        create_date(2032, Month::January, 1).unwrap(),
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.035);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.045);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let inst_a: Arc<dyn Instrument> = Arc::new(bond_a);
    let inst_b: Arc<dyn Instrument> = Arc::new(bond_b);
    let portfolio: Arc<dyn Instrument> = Arc::new(CompositeInstrument::new(
        "PORTFOLIO-AB",
        Arc::clone(&inst_a),
        Arc::clone(&inst_b),
    ));

    let attr_a = attribute_pnl_parallel(
        &inst_a,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let attr_b = attribute_pnl_parallel(
        &inst_b,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let attr_portfolio = attribute_pnl_parallel(
        &portfolio,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    let total_sum = attr_a.total_pnl.amount() + attr_b.total_pnl.amount();
    assert!(
        (attr_portfolio.total_pnl.amount() - total_sum).abs() < 0.01,
        "Portfolio total P&L should equal sum of components, portfolio={}, sum={}",
        attr_portfolio.total_pnl.amount(),
        total_sum
    );

    let rates_sum = attr_a.rates_curves_pnl.amount() + attr_b.rates_curves_pnl.amount();
    assert!(
        (attr_portfolio.rates_curves_pnl.amount() - rates_sum).abs() < 0.01,
        "Portfolio rates P&L should equal sum of components, portfolio={}, sum={}",
        attr_portfolio.rates_curves_pnl.amount(),
        rates_sum
    );
}

// =============================================================================
// Property-Based Tests
// =============================================================================

proptest! {
    // Fixed RNG seed per the repo testing standard (deterministic runs;
    // failures reproduce locally without the persistence file).
    #![proptest_config(ProptestConfig {
        cases: 15,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x5EED_2026),
        ..ProptestConfig::default()
    })]

    /// Property: Carry should be non-negative for a coupon-bearing bond held over one day.
    ///
    /// A coupon bond accrues interest over time. For a single-day holding period,
    /// carry should be non-negative (could be zero if no coupon accrual on that day).
    #[test]
    fn prop_carry_non_negative_for_coupon_bond(
        coupon_rate in 0.02f64..0.08f64,
        flat_rate in 0.01f64..0.07f64,
        maturity_years in 2u32..15u32,
    ) {
        let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

        let issue = create_date(2025, Month::January, 1).unwrap();
        let maturity = create_date(2025 + maturity_years as i32, Month::January, 1).unwrap();

        let bond = Bond::fixed(
            "PROP-CARRY-TEST",
            Money::new(1_000_000.0, Currency::USD),
            coupon_rate,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        let curve = build_flat_curve("USD-OIS", as_of_t0, flat_rate);
        let market = MarketContext::new().insert(curve);

        let config = FinstackConfig::default();
        let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

        let attribution = attribute_pnl_parallel(
            &bond_instrument,
            &market,
            &market,
            as_of_t0,
            as_of_t1,
            &config,
            ExecutionPolicy::Parallel,
        )
        .unwrap();

        // Carry should be non-negative (allow small negative due to numerical precision)
        prop_assert!(
            attribution.carry.amount() >= -1.0,
            "Carry should be non-negative for coupon bond, got {}",
            attribution.carry.amount()
        );
    }

    /// Property: Rates P&L magnitude should increase with longer maturity.
    ///
    /// Longer maturity bonds have higher duration, so they should have larger
    /// P&L from rate changes (in absolute terms).
    #[test]
    fn prop_rates_pnl_increases_with_maturity(
        rate_shift_bp in 10i32..100i32,
    ) {
        let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 16).unwrap();
        let issue = create_date(2025, Month::January, 1).unwrap();

        let base_rate = 0.04f64;
        let rate_shift = (rate_shift_bp as f64) / 10_000.0;

        let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, base_rate);
        let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, base_rate + rate_shift);

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        let config = FinstackConfig::default();

        // Compare 3Y vs 10Y bond
        let maturities = [3, 10];
        let mut pnl_magnitudes = Vec::new();

        for years in maturities {
            let maturity = create_date(2025 + years, Month::January, 1).unwrap();
            let id = format!("PROP-MATURITY-{}Y", years);
            let bond = Bond::fixed(
                id.as_str(),
                Money::new(1_000_000.0, Currency::USD),
                0.05,
                issue,
                maturity,
                "USD-OIS",
            )
            .unwrap();

            let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

            let attribution = attribute_pnl_parallel(
                &bond_instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
                ExecutionPolicy::Parallel,
            )
            .unwrap();

            pnl_magnitudes.push(attribution.rates_curves_pnl.amount().abs());
        }

        // 10Y bond should have larger rates P&L magnitude than 3Y bond
        prop_assert!(
            pnl_magnitudes[1] > pnl_magnitudes[0],
            "10Y bond rates P&L ({:.2}) should exceed 3Y bond ({:.2})",
            pnl_magnitudes[1], pnl_magnitudes[0]
        );
    }

    /// Property: Rates P&L should scale approximately linearly with rate change magnitude
    /// for small moves (first-order approximation).
    #[test]
    fn prop_rates_pnl_scales_with_rate_change(
        base_shift_bp in 5i32..20i32,
    ) {
        let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

        let issue = create_date(2025, Month::January, 1).unwrap();
        let maturity = create_date(2030, Month::January, 1).unwrap();

        let bond = Bond::fixed(
            "PROP-SCALING-TEST",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .unwrap();

        let base_rate = 0.04f64;
        let small_shift = (base_shift_bp as f64) / 10_000.0;
        let large_shift = small_shift * 2.0;

        let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, base_rate);
        let curve_small = build_flat_curve("USD-OIS", as_of_t1, base_rate + small_shift);
        let curve_large = build_flat_curve("USD-OIS", as_of_t1, base_rate + large_shift);

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_small = MarketContext::new().insert(curve_small);
        let market_large = MarketContext::new().insert(curve_large);

        let config = FinstackConfig::default();
        let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

        let attr_small = attribute_pnl_parallel(
            &bond_instrument,
            &market_t0,
            &market_small,
            as_of_t0,
            as_of_t1,
            &config,
            ExecutionPolicy::Parallel,
        )
        .unwrap();

        let attr_large = attribute_pnl_parallel(
            &bond_instrument,
            &market_t0,
            &market_large,
            as_of_t0,
            as_of_t1,
            &config,
            ExecutionPolicy::Parallel,
        )
        .unwrap();

        let pnl_small = attr_small.rates_curves_pnl.amount().abs();
        let pnl_large = attr_large.rates_curves_pnl.amount().abs();

        // For small moves, P&L should scale roughly 2x (with some convexity adjustment)
        // Allow 20% deviation from perfect 2x scaling due to convexity
        let ratio = pnl_large / pnl_small;
        prop_assert!(
            ratio > 1.6 && ratio < 2.4,
            "P&L scaling ratio should be near 2.0, got {} (small={:.2}, large={:.2})",
            ratio, pnl_small, pnl_large
        );
    }
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test attribution with very small notional.
#[test]
fn test_small_notional_edge_case() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    // Very small notional
    let bond = Bond::fixed(
        "SMALL-NOTIONAL-TEST",
        Money::new(100.0, Currency::USD), // $100 notional
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.05);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Should still compute without error
    assert!(attribution.total_pnl.amount().is_finite());
    assert!(attribution.rates_curves_pnl.amount().is_finite());

    // Rates P&L should still be negative (rates increased)
    assert!(
        attribution.rates_curves_pnl.amount() < 0.0,
        "Rates P&L should be negative even for small notional"
    );
}

/// Test attribution with very large notional.
#[test]
fn test_large_notional_edge_case() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    // Very large notional
    let bond = Bond::fixed(
        "LARGE-NOTIONAL-TEST",
        Money::new(1_000_000_000.0, Currency::USD), // $1B notional
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.041); // 10bp increase

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Should compute without overflow
    assert!(attribution.total_pnl.amount().is_finite());
    assert!(attribution.rates_curves_pnl.amount().is_finite());

    // For $1B notional, 10bp move should produce ~$4-5M P&L
    // (DV01 ≈ $450k per bp for 5Y bond with $1B notional)
    let expected_magnitude = 4_000_000.0;
    assert!(
        attribution.rates_curves_pnl.amount().abs() > expected_magnitude * 0.5,
        "Large notional should produce significant P&L, got {}",
        attribution.rates_curves_pnl.amount()
    );
}

/// Test attribution with extreme rate levels.
#[test]
fn test_extreme_rate_levels() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "EXTREME-RATES-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    // Test with very low rates (near zero)
    let curve_low = build_flat_curve("USD-OIS", as_of_t0, 0.001); // 10bp
    let curve_lower = build_flat_curve("USD-OIS", as_of_t1, 0.0005); // 5bp

    let market_low = MarketContext::new().insert(curve_low);
    let market_lower = MarketContext::new().insert(curve_lower);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution_low = attribute_pnl_parallel(
        &bond_instrument,
        &market_low,
        &market_lower,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    assert!(
        attribution_low.total_pnl.amount().is_finite(),
        "Attribution should compute with very low rates"
    );

    // Test with high rates
    let curve_high = build_flat_curve("USD-OIS", as_of_t0, 0.15); // 15%
    let curve_higher = build_flat_curve("USD-OIS", as_of_t1, 0.16); // 16%

    let market_high = MarketContext::new().insert(curve_high);
    let market_higher = MarketContext::new().insert(curve_higher);

    let attribution_high = attribute_pnl_parallel(
        &bond_instrument,
        &market_high,
        &market_higher,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    assert!(
        attribution_high.total_pnl.amount().is_finite(),
        "Attribution should compute with high rates"
    );

    // Sign should still be correct
    assert!(
        attribution_high.rates_curves_pnl.amount() < 0.0,
        "Rates P&L should be negative when rates increase, even at high levels"
    );
}

/// Test attribution near maturity (short time to expiry).
#[test]
fn test_near_maturity_edge_case() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2024, Month::January, 1).unwrap();
    let maturity = create_date(2025, Month::February, 1).unwrap(); // 16 days to maturity

    let bond = Bond::fixed(
        "NEAR-MATURITY-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
    .unwrap();

    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, 0.05);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl_parallel(
        &bond_instrument,
        &market_t0,
        &market_t1,
        as_of_t0,
        as_of_t1,
        &config,
        ExecutionPolicy::Parallel,
    )
    .unwrap();

    // Should compute without error
    assert!(attribution.total_pnl.amount().is_finite());

    // Near-maturity bond should have low duration, hence low rates P&L
    // For a 16-day bond, rates P&L should be small
    assert!(
        attribution.rates_curves_pnl.amount().abs() < 5000.0,
        "Near-maturity bond should have low rates sensitivity, got {}",
        attribution.rates_curves_pnl.amount()
    );
}

finstack_valuations::impl_empty_cashflow_provider!(
    ScaledInstrument,
    finstack_cashflows::builder::CashflowRepresentation::NoResidual
);
finstack_valuations::impl_empty_cashflow_provider!(
    CompositeInstrument,
    finstack_cashflows::builder::CashflowRepresentation::NoResidual
);
