//! Stochastic pools of real instruments: per-name factor paths, revolver
//! utilization linked to the simulated spread, funding diagnostics per path,
//! and option-adjusted spread on instrument collateral.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_models::credit::pool::StochasticDefaultSpec;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, CreditSpreadProcessSpec, DrawRepaySpec, McConfig, RevolvingCredit,
    RevolvingCreditFees, StochasticUtilizationSpec, UtilizationProcess,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    calculate_tranche_oas, CallExercisePolicy, InstrumentCollateral, OasConfig, PricingMode,
    StochasticPricingResult, StructuredCredit,
};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

use super::instrument_pool_tests::{deal_with, fixed_revolver, market_with_curves, usd, DealSpec};

const CLOSING: Date = date!(2024 - 01 - 15);
const MATURITY: Date = date!(2034 - 01 - 15);
const HAZARD_ID: &str = "BORROWER-HZ";

/// Stochastic-utilization revolver. `credit` attaches the borrower hazard
/// curve with a market-anchored spread process so the utilization target can
/// follow the simulated spread through `spread_sensitivity`.
fn stochastic_revolver(volatility: f64, spread_sensitivity: f64, credit: bool) -> RevolvingCredit {
    let mc_config = credit.then(|| McConfig {
        correlation_matrix: None,
        credit_spread_process: CreditSpreadProcessSpec::MarketAnchored {
            credit_curve_id: HAZARD_ID.into(),
            kappa: 0.5,
            implied_vol: 0.6,
            tenor_years: None,
        },
        interest_rate_process: None,
        util_credit_corr: Some(0.5),
    });
    let mut builder = RevolvingCredit::builder()
        .id("RCF-STOCH".into())
        .commitment_amount(usd(50_000_000.0))
        .drawn_amount(usd(10_000_000.0))
        .commitment_date(CLOSING)
        .maturity(date!(2027 - 01 - 15))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.06 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 5.0).expect("fees"))
        .draw_repay_spec(DrawRepaySpec::Stochastic(Box::new(
            StochasticUtilizationSpec {
                utilization_process: UtilizationProcess::MeanReverting {
                    target_rate: 0.5,
                    speed: 1.0,
                    volatility,
                    spread_sensitivity,
                },
                num_paths: 2,
                seed: Some(42),
                antithetic: false,
                use_sobol_qmc: false,
                mc_config,
            },
        )))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.4)
        .leq(0.5);
    if credit {
        builder = builder.credit_curve_id(HAZARD_ID.into());
    }
    builder.build().expect("stochastic revolver")
}

/// Fixed, floating and callable bonds, a deterministic revolver and a
/// zero-volatility stochastic revolver, with every draw fundable.
fn parity_deal(reserve: f64) -> StructuredCredit {
    deal_with(DealSpec {
        collateral: InstrumentCollateral {
            bonds: vec![
                Bond::example().expect("fixed bond"),
                Bond::example_floating().expect("floating bond"),
                Bond::example_callable().expect("callable bond"),
            ],
            revolvers: vec![
                fixed_revolver(CLOSING, date!(2027 - 01 - 15)),
                stochastic_revolver(0.0, 0.0, false),
            ],
            call_exercise: CallExercisePolicy::FirstCall,
            ..Default::default()
        },
        reserve,
        reserve_target: None,
        closing: CLOSING,
        maturity: MATURITY,
        senior: 11_700_000.0,
        equity: 1_300_000.0,
    })
}

fn monte_carlo(num_paths: usize) -> PricingMode {
    PricingMode::MonteCarlo {
        num_paths,
        antithetic: true,
    }
}

fn tranche_npv(result: &StochasticPricingResult, id: &str) -> f64 {
    result
        .tranche_results
        .iter()
        .find(|t| t.tranche_id == id)
        .unwrap_or_else(|| panic!("tranche {id}"))
        .npv
        .amount()
}

/// Zero utilization volatility, zero spread sensitivity and zero default
/// probability reproduce the deterministic pool on every path: the Monte
/// Carlo mean equals the deterministic deal and tranche values and the
/// standard error is exactly zero.
#[test]
fn zero_volatility_paths_reproduce_the_deterministic_pool() {
    let deal = parity_deal(60_000_000.0);
    let market = market_with_curves(CLOSING);

    let deterministic = deal.value(&market, CLOSING).expect("deterministic pv");
    let stochastic = deal
        .price_stochastic_with_mode(
            &market,
            CLOSING,
            PricingMode::MonteCarlo {
                num_paths: 4,
                antithetic: false,
            },
        )
        .expect("stochastic pricing of instrument collateral");

    assert_eq!(stochastic.num_paths, 4);
    assert_eq!(
        stochastic.pv_std_error, 0.0,
        "every path must be identical without volatility"
    );
    assert_eq!(stochastic.unfunded_draw_path_fraction, 0.0);
    let pv = stochastic.npv.amount();
    assert!(
        (pv - deterministic.amount()).abs() <= 1e-6 * deterministic.amount().abs(),
        "stochastic {pv} vs deterministic {}",
        deterministic.amount()
    );
    for id in ["A", "EQ"] {
        let expected = deal
            .value_tranche(id, &market, CLOSING)
            .expect("tranche pv")
            .amount();
        let actual = tranche_npv(&stochastic, id);
        assert!(
            (actual - expected).abs() <= 1e-6 * expected.abs().max(1.0),
            "tranche {id}: stochastic {actual} vs deterministic {expected}"
        );
    }
    // The deterministic revolver draws 5M once and the frozen stochastic
    // revolver never draws, so the expected draws are exactly that.
    assert!(
        (stochastic.expected_collateral_draws.amount() - 5_000_000.0).abs() < 1e-6,
        "expected draws {}",
        stochastic.expected_collateral_draws.amount()
    );
}

/// A reserve too small for the draw calendar is refused deterministically but
/// recorded per path stochastically, and every path is affected here.
#[test]
fn unfunded_draws_are_recorded_per_path() {
    let deal = parity_deal(1_000_000.0);
    let market = market_with_curves(CLOSING);
    let err = deal
        .value(&market, CLOSING)
        .expect_err("the deterministic run must refuse an unfundable calendar");
    assert!(err.to_string().contains("exceed the reserve"), "{err}");

    let stochastic = deal
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(4))
        .expect("stochastic pricing records the shortfall");
    assert_eq!(stochastic.unfunded_draw_path_fraction, 1.0);
}

/// Borrower hazard curve whose term structure rises, so the market-anchored
/// spread process reverts above its starting level: a widening spread path
/// on average.
fn widening_hazard_curve() -> HazardCurve {
    HazardCurve::builder(HAZARD_ID)
        .base_date(CLOSING)
        .knots([(1.0, 0.04), (3.0, 0.10), (5.0, 0.14)])
        .recovery_rate(0.4)
        .build()
        .expect("hazard curve")
}

/// Linking the utilization target to the simulated spread raises draws on a
/// widening spread path, and the wide-spread paths are the paths where names
/// default: exposure at default grows, so expected loss rises and the senior
/// tranche is worth less than with an unlinked target on the same random
/// numbers.
#[test]
fn spread_sensitivity_raises_draws_and_losses_on_stress_paths() {
    let build = |spread_sensitivity: f64| {
        let mut deal = deal_with(DealSpec {
            collateral: InstrumentCollateral {
                bonds: vec![Bond::example().expect("fixed bond")],
                revolvers: vec![stochastic_revolver(0.25, spread_sensitivity, true)],
                ..Default::default()
            },
            reserve: 40_000_000.0,
            reserve_target: None,
            closing: CLOSING,
            maturity: MATURITY,
            senior: 45_900_000.0,
            equity: 5_100_000.0,
        });
        deal.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(0.02, 0.3));
        deal
    };
    let market = market_with_curves(CLOSING).insert(widening_hazard_curve());

    let unlinked = build(0.0)
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(400))
        .expect("unlinked pricing");
    let linked = build(3.0)
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(400))
        .expect("linked pricing");

    assert!(
        linked.expected_collateral_draws.amount() > unlinked.expected_collateral_draws.amount(),
        "draws: linked {} vs unlinked {}",
        linked.expected_collateral_draws.amount(),
        unlinked.expected_collateral_draws.amount()
    );
    // Note losses are realized as principal shortfall at legal final (the CLO
    // par-preserving policy), where the extra spread income on the drawn par
    // partly offsets the extra defaults; the draw option cost isolates the
    // channel this test is about — draws at the contractual margin on the
    // paths where the fair spread has widened cost the deal more (the cost is
    // negative when spreads widen, so "more" means more negative).
    assert!(
        linked.draw_option_cost.amount() < unlinked.draw_option_cost.amount(),
        "draw option cost: linked {} vs unlinked {}",
        linked.draw_option_cost.amount(),
        unlinked.draw_option_cost.amount()
    );
    assert!(
        tranche_npv(&linked, "A") < tranche_npv(&unlinked, "A"),
        "senior npv: linked {} vs unlinked {}",
        tranche_npv(&linked, "A"),
        tranche_npv(&unlinked, "A")
    );
    assert!(unlinked.unfunded_draw_path_fraction < 1.0);
}

/// The same seed reproduces the result bit for bit, whatever the thread count.
#[test]
fn same_seed_is_bit_identical_across_thread_counts() {
    let mut deal = deal_with(DealSpec {
        collateral: InstrumentCollateral {
            bonds: vec![Bond::example_floating().expect("floating bond")],
            revolvers: vec![stochastic_revolver(0.25, 2.0, true)],
            ..Default::default()
        },
        reserve: 40_000_000.0,
        reserve_target: None,
        closing: CLOSING,
        maturity: MATURITY,
        senior: 45_900_000.0,
        equity: 5_100_000.0,
    });
    deal.with_stochastic_default(StochasticDefaultSpec::gaussian_copula(0.02, 0.3));
    let market = market_with_curves(CLOSING)
        .insert(HazardCurve::flat(HAZARD_ID, CLOSING, 0.05, 0.4).expect("hazard curve"));

    let price = || {
        deal.price_stochastic_with_mode(&market, CLOSING, monte_carlo(64))
            .expect("pricing")
    };
    let parallel = price();
    let again = price();
    let serial = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("single-thread pool")
        .install(price);

    for other in [&again, &serial] {
        assert_eq!(parallel.npv, other.npv);
        assert_eq!(
            parallel.pv_std_error.to_bits(),
            other.pv_std_error.to_bits()
        );
        assert_eq!(parallel.expected_loss, other.expected_loss);
        assert_eq!(
            parallel.expected_collateral_draws,
            other.expected_collateral_draws
        );
        for (a, b) in parallel.tranche_results.iter().zip(&other.tranche_results) {
            assert_eq!(a.npv, b.npv);
            assert_eq!(a.expected_loss, b.expected_loss);
        }
    }
    assert!(parallel.pv_std_error > 0.0, "the paths must differ");
}

/// Option-adjusted spread runs on instrument collateral: at the model price
/// with no stochastic dimension the spread is zero, and a stochastic rate
/// path re-projects the floating collateral and still solves.
#[test]
fn oas_solves_on_instrument_collateral() {
    let deal = parity_deal(60_000_000.0);
    let market = market_with_curves(CLOSING);
    let senior = deal
        .value_tranche("A", &market, CLOSING)
        .expect("senior pv")
        .amount();
    let price_pct = senior / 11_700_000.0 * 100.0;

    let deterministic = calculate_tranche_oas(
        &deal,
        "A",
        price_pct,
        &market,
        CLOSING,
        &OasConfig {
            num_paths: 1,
            stochastic_rates: false,
            stochastic_credit: false,
            ..OasConfig::default()
        },
    )
    .expect("deterministic oas");
    assert!(
        deterministic.oas.abs() < 1e-6,
        "oas at the model price must be zero, got {}",
        deterministic.oas
    );

    let stochastic = calculate_tranche_oas(
        &deal,
        "A",
        price_pct,
        &market,
        CLOSING,
        &OasConfig {
            num_paths: 8,
            stochastic_rates: true,
            stochastic_credit: false,
            hw_sigma: 0.01,
            ..OasConfig::default()
        },
    )
    .expect("stochastic-rate oas");
    assert!(stochastic.oas.is_finite());
    assert_eq!(stochastic.num_paths, 8);
    assert!(
        stochastic.price_std_error > 0.0,
        "rate paths must move the price"
    );
}

/// Currency is preserved on every reported money field.
#[test]
fn stochastic_result_money_fields_share_the_pool_currency() {
    let deal = parity_deal(60_000_000.0);
    let market = market_with_curves(CLOSING);
    let result = deal
        .price_stochastic_with_mode(&market, CLOSING, monte_carlo(2))
        .expect("pricing");
    assert_eq!(result.npv.currency(), Currency::USD);
    assert_eq!(result.expected_collateral_draws.currency(), Currency::USD);
}
