//! Draw option cost at the deal level: revolver draws at fixed margins
//! valued against the path's fair spread, allocated to tranches by running
//! the waterfall on actual and on fairly priced draw interest.

use finstack_quant_core::dates::Date;
use finstack_quant_models::credit::pool::StochasticDefaultSpec;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    CreditSpreadProcessSpec, RevolvingCreditPricer,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation_with_diagnostics, CoverageTestSpec, InstrumentCollateral, PricingMode,
    StochasticPricingResult, StructuredCredit, TrancheCoupon,
};
use time::macros::date;

use super::instrument_pool_tests::{deal_with, DealSpec};
use crate::revolving_credit::draw_option_cost::{
    market, revolver, widening_hazard_curve, HAZARD_ID,
};

const CLOSING: Date = date!(2024 - 01 - 15);
const MATURITY: Date = date!(2027 - 01 - 15);

fn pool_of(
    bonds: Vec<Bond>,
    spread: CreditSpreadProcessSpec,
    utilization_vol: f64,
) -> StructuredCredit {
    let mut deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            bonds,
            revolvers: vec![revolver("RCF-POOL", utilization_vol, spread, 2)],
            ..Default::default()
        },
        reserve: 45_000_000.0,
        reserve_target: None,
        closing: CLOSING,
        maturity: MATURITY,
        senior: 49_500_000.0,
        equity: 5_500_000.0,
    });
    deal.credit_model.recovery_spec =
        finstack_quant_valuations::instruments::fixed_income::structured_credit::RecoveryModelSpec::with_lag(0.4, 0);
    deal
}

fn monte_carlo(num_paths: usize) -> PricingMode {
    PricingMode::MonteCarlo {
        num_paths,
        antithetic: true,
    }
}

fn tranche_cost(result: &StochasticPricingResult, id: &str) -> f64 {
    result
        .tranche_results
        .iter()
        .find(|t| t.tranche_id == id)
        .unwrap_or_else(|| panic!("tranche {id}"))
        .draw_option_cost
        .amount()
}

/// A constant spread equal to the contractual margin: the counterfactual
/// waterfall is the actual one, so the deal and tranche option costs are
/// exactly zero on every path.
#[test]
fn constant_spread_at_the_margin_has_zero_deal_option_cost() {
    let deal = pool_of(Vec::new(), CreditSpreadProcessSpec::Constant(0.025), 0.25);
    let result = deal
        .price_stochastic_with_mode(&market(), CLOSING, monte_carlo(8))
        .expect("pricing");
    assert!(
        result.expected_collateral_draws.amount() > 0.0,
        "the paths must carry draws"
    );
    assert_eq!(result.draw_option_cost.amount(), 0.0);
    assert_eq!(result.draw_option_cost_paths.len(), 8);
    assert!(result.draw_option_cost_paths.iter().all(|c| *c == 0.0));
    assert_eq!(tranche_cost(&result, "A"), 0.0);
    assert_eq!(tranche_cost(&result, "EQ"), 0.0);
}

/// A single-revolver pass-through pool bears the same option cost the
/// standalone facility reports. The facility carries no credit risk (a zero
/// constant spread, no hazard curve), so both engines value the same
/// near-deterministic draws as forward loans at the full contractual margin
/// with unit survival; with credit risk the two engines apply different
/// default models to the annuity (the standalone its path hazard, the pool
/// the name's hazard curve or the deal model) and agree only in expectation.
#[test]
fn single_revolver_pool_matches_the_standalone_option_cost() {
    let spread = CreditSpreadProcessSpec::Constant(0.0);
    let facility = revolver("RCF-POOL", 1e-6, spread.clone(), 4);
    let market = market();
    let standalone = RevolvingCreditPricer::price_with_paths(&facility, &market, CLOSING)
        .expect("standalone pricing")
        .draw_option_cost
        .mean
        .amount();
    assert!(
        standalone > 0.0,
        "draws above a zero fair spread are worth the margin: {standalone}"
    );

    // A senior coupon the revolver's interest always covers, so every period's
    // interest reaches the notes as it arrives (no reserve draws for
    // shortfalls deferring cash to the deal's end).
    let mut deal = pool_of(Vec::new(), spread, 1e-6);
    for tranche in deal.tranches.tranches.iter_mut() {
        if tranche.id.as_str() == "A" {
            tranche.coupon = TrancheCoupon::Fixed { rate: 0.01 };
        }
    }
    let pooled = deal
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(4))
        .expect("pool pricing");
    let deal_cost = pooled.draw_option_cost.amount();
    assert!(
        (deal_cost - standalone).abs() <= 0.01 * standalone.abs(),
        "deal {deal_cost} vs standalone {standalone}"
    );
    // Pass-through: the tranche shares add up to the deal cost.
    let shares = tranche_cost(&pooled, "A") + tranche_cost(&pooled, "EQ");
    assert!((shares - deal_cost).abs() <= 1e-6 * deal_cost.abs());
}

/// Market-anchored widening with genuine utilization volatility: the deal
/// bears a negative option cost, the tranche shares sum to it, and the
/// equity tranche, which receives the residual interest, bears the largest
/// share.
#[test]
fn tranche_option_costs_sum_to_the_deal_cost_and_equity_bears_the_most() {
    let mut deal = pool_of(
        vec![Bond::example().expect("fixed bond")],
        CreditSpreadProcessSpec::MarketAnchored {
            credit_curve_id: "BORROWER-HZ".into(),
            kappa: 0.5,
            implied_vol: 0.4,
            tenor_years: None,
        },
        0.25,
    );
    deal.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(0.01, 0.3));
    let market = market().insert(widening_hazard_curve());
    let result = deal
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(200))
        .expect("pricing");

    let deal_cost = result.draw_option_cost.amount();
    assert!(deal_cost < 0.0, "deal cost {deal_cost}");
    assert_eq!(result.draw_option_cost_paths.len(), 200);
    let path_mean = result.draw_option_cost_paths.iter().sum::<f64>() / 200.0;
    assert!((path_mean - deal_cost).abs() <= 1e-6 * deal_cost.abs());

    let senior = tranche_cost(&result, "A");
    let equity = tranche_cost(&result, "EQ");
    assert!(
        (senior + equity - deal_cost).abs() <= 1e-6 * deal_cost.abs(),
        "senior {senior} + equity {equity} vs deal {deal_cost}"
    );
    assert!(
        equity.abs() > senior.abs(),
        "equity {equity} must bear more than senior {senior}"
    );
}

/// With credit risk the standalone facility discounts its annuity on the
/// spread path's hazard while the pool realizes defaults name by name, so the
/// two option costs agree in expectation rather than path by path: their
/// Monte Carlo means must coincide within the combined standard error.
#[test]
fn credit_risky_single_revolver_pool_matches_the_standalone_option_cost_in_expectation() {
    const PATHS: usize = 512;
    let spread = CreditSpreadProcessSpec::MarketAnchored {
        credit_curve_id: HAZARD_ID.into(),
        kappa: 0.5,
        implied_vol: 0.4,
        tenor_years: None,
    };
    let facility = revolver("RCF-POOL", 0.25, spread.clone(), PATHS);
    let market = market().insert(widening_hazard_curve());
    let standalone = RevolvingCreditPricer::price_with_paths(&facility, &market, CLOSING)
        .expect("standalone pricing")
        .draw_option_cost;

    let mut deal = pool_of(Vec::new(), spread, 0.25);
    for tranche in deal.tranches.tranches.iter_mut() {
        if tranche.id.as_str() == "A" {
            tranche.coupon = TrancheCoupon::Fixed { rate: 0.01 };
        }
    }
    let pooled = deal
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(PATHS))
        .expect("pool pricing");
    let paths = &pooled.draw_option_cost_paths;
    let n = paths.len() as f64;
    let pool_mean = paths.iter().sum::<f64>() / n;
    let pool_se =
        (paths.iter().map(|x| (x - pool_mean).powi(2)).sum::<f64>() / (n - 1.0) / n).sqrt();
    let standalone_mean = standalone.mean.amount();
    let tolerance = 3.0 * (pool_se.powi(2) + standalone.stderr.powi(2)).sqrt();
    assert!(
        standalone_mean < 0.0 && pool_mean < 0.0,
        "both costs are negative under widening: pool {pool_mean}, standalone {standalone_mean}"
    );
    assert!(
        (pool_mean - standalone_mean).abs() <= tolerance,
        "pool {pool_mean} (se {pool_se}) vs standalone {standalone_mean} (se {}) differ by more \
         than {tolerance}",
        standalone.stderr
    );
}

/// Instrument-pool twin of the positional coverage test: an OC position
/// after A's coupon traps the residual and pays it to A's principal only.
/// The draw option cost allocation runs the same waterfall, so the tranche
/// shares still sum to the deal cost.
#[test]
fn senior_coverage_position_diverts_to_the_senior_note_only() {
    let build = |tests: Vec<CoverageTestSpec>| {
        let mut deal = pool_of(
            vec![Bond::example().expect("fixed bond")],
            CreditSpreadProcessSpec::MarketAnchored {
                credit_curve_id: HAZARD_ID.into(),
                kappa: 0.5,
                implied_vol: 0.4,
                tenor_years: None,
            },
            0.25,
        )
        .with_coverage_triggers(tests)
        .expect("coverage tests");
        // A 1% senior coupon leaves excess spread every period, so there is
        // a residual for the failing position to trap.
        for tranche in deal.tranches.tranches.iter_mut() {
            if tranche.id.as_str() == "A" {
                tranche.coupon = TrancheCoupon::Fixed { rate: 0.01 };
            }
        }
        deal.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(0.01, 0.3));
        deal
    };
    // A 5.0x OC level the collateral can never reach: the position fails on
    // every payment date and diverts everything below A's coupon.
    let breached = build(vec![CoverageTestSpec::oc("A", 5.0)]);
    let control = build(Vec::new());
    let market = market().insert(widening_hazard_curve());

    let run = run_simulation_with_diagnostics(&breached, &market, CLOSING)
        .expect("simulation")
        .tranches;
    let base = run_simulation_with_diagnostics(&control, &market, CLOSING)
        .expect("simulation")
        .tranches;
    let on = |flows: &[(Date, finstack_quant_core::money::Money)], date: Date| -> f64 {
        flows
            .iter()
            .filter(|(flow_date, _)| *flow_date == date)
            .map(|(_, amount)| amount.amount())
            .sum()
    };
    assert!(
        run["A"].deferred_flows.is_empty(),
        "A's coupon ranks above the test and is never deferred"
    );
    // Before legal final A is outstanding, so the trapped residual can only
    // pay A: on every date the control deal paid equity, the breached deal
    // pays at least that residual to A's principal on top of the scheduled
    // paydown, and exactly it on the first date, before the earlier cures
    // have lowered A's coupon and enlarged the residual.
    let residual_dates: Vec<Date> = base["EQ"]
        .interest_flows
        .iter()
        .filter(|(date, amount)| *date < MATURITY && amount.amount() > 0.0)
        .map(|(date, _)| *date)
        .collect();
    assert!(
        residual_dates.len() >= 4,
        "the control deal pays equity a residual most periods, got {residual_dates:?}"
    );
    for (i, date) in residual_dates.iter().enumerate() {
        let residual = on(&base["EQ"].interest_flows, *date);
        let extra_paydown =
            on(&run["A"].principal_flows, *date) - on(&base["A"].principal_flows, *date);
        assert!(
            extra_paydown >= residual - 1.0,
            "on {date} A's extra paydown {extra_paydown} must cover the trapped residual {residual}"
        );
        assert!(
            i > 0 || (extra_paydown - residual).abs() < 1.0,
            "on the first breach {date} A's extra paydown {extra_paydown} is exactly the \
             residual {residual}"
        );
        assert!(
            on(&run["EQ"].interest_flows, *date) < 1.0
                && on(&run["EQ"].principal_flows, *date) < 1.0,
            "equity receives nothing on {date} while the position fails"
        );
    }

    let result = breached
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(100))
        .expect("pricing");
    let deal_cost = result.draw_option_cost.amount();
    assert!(deal_cost < 0.0, "deal cost {deal_cost}");
    let senior_share = tranche_cost(&result, "A");
    let equity_share = tranche_cost(&result, "EQ");
    assert!(
        (senior_share + equity_share - deal_cost).abs() <= 1e-6 * deal_cost.abs(),
        "senior {senior_share} + equity {equity_share} vs deal {deal_cost}"
    );
    // The cure only accelerates A's principal; the fair-versus-actual draw
    // interest still lands on the residual holder at legal final.
    assert!(
        equity_share.abs() > senior_share.abs(),
        "equity {equity_share} still bears more of the cost than senior {senior_share}"
    );
}
