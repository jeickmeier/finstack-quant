//! Theta calculator tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_theta_finite() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "THETA1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let theta = *result.measures.get("theta").unwrap();
    assert!(theta.is_finite());
}

#[test]
fn test_theta_sign_diagnostic() {
    use finstack_quant_core::market_data::context::MarketContext;

    let as_of = date!(2025 - 01 - 15);

    let bond = Bond::fixed(
        "THETA_SIGN",
        Money::new(1_000_000.0, Currency::USD),
        0.04,
        date!(2024 - 01 - 15),
        date!(2028 - 01 - 15),
        "USD-OIS",
    )
    .unwrap();

    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (0.25, 0.9888),
                (0.5, 0.9775),
                (1.0, 0.955),
                (2.0, 0.91),
                (3.0, 0.87),
                (5.0, 0.80),
                (7.0, 0.73),
                (10.0, 0.65),
            ])
            .build()
            .unwrap();
    let market = MarketContext::new().insert(curve);

    let base_pv = Instrument::value(&bond, &market, as_of).unwrap();
    let rolled_date = as_of + time::Duration::days(1);
    let rolled_pv = Instrument::value(&bond, &market, rolled_date).unwrap();
    eprintln!("--- Bond theta diagnostic ---");
    eprintln!("as_of = {as_of}");
    eprintln!("rolled_date = {rolled_date}");
    eprintln!("base_pv = {}", base_pv.amount());
    eprintln!("rolled_pv = {}", rolled_pv.amount());
    eprintln!(
        "pv_change (rolled - base) = {}",
        rolled_pv.amount() - base_pv.amount()
    );

    let disc = market.get_discount("USD-OIS").unwrap();
    let df_1d = disc.df_between_dates(as_of, rolled_date).unwrap();
    let df_base_to_rolled = disc.df_on_date_curve(rolled_date).unwrap();
    let df_base_to_as_of = disc.df_on_date_curve(as_of).unwrap();
    eprintln!("df(as_of -> rolled_date) = {df_1d}");
    eprintln!("df_on_curve(rolled_date) = {df_base_to_rolled}");
    eprintln!("df_on_curve(as_of) = {df_base_to_as_of}");

    use finstack_quant_cashflows::traits::CashflowProvider;
    let dated_flows_base = CashflowProvider::dated_cashflows(&bond, &market, as_of).unwrap();
    let dated_flows_rolled =
        CashflowProvider::dated_cashflows(&bond, &market, rolled_date).unwrap();
    eprintln!("dated_flows at as_of: {} items", dated_flows_base.len());
    for (d, m) in &dated_flows_base {
        eprintln!("  {d}: {}", m.amount());
    }
    eprintln!(
        "dated_flows at rolled_date: {} items",
        dated_flows_rolled.len()
    );
    for (d, m) in &dated_flows_rolled {
        eprintln!("  {d}: {}", m.amount());
    }
    eprintln!("---");

    let result = Instrument::price_with_metrics(
        &bond,
        &market,
        as_of,
        &[
            MetricId::Theta,
            MetricId::ThetaCarry,
            MetricId::ThetaRollDown,
        ],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )
    .unwrap();
    let theta = *result.measures.get("theta").unwrap();
    let carry = result
        .measures
        .get("theta_carry")
        .copied()
        .unwrap_or(f64::NAN);
    let roll_down = result
        .measures
        .get("theta_roll_down")
        .copied()
        .unwrap_or(f64::NAN);
    eprintln!("theta = {theta}");
    eprintln!("theta_carry = {carry}");
    eprintln!("theta_roll_down = {roll_down}");

    assert!(
        theta > 0.0,
        "Bond theta should be positive (carry > 0 for long bond), got {theta}"
    );
}
