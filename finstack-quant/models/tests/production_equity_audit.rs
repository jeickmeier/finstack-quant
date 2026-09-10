//! Independent equity-lattice regression evidence for production remediation.

use finstack_quant_models::trees::BinomialTree;
use finstack_quant_models::types::{OptionMarketParams, OptionType};

#[test]
fn b6_lr_matches_quantlib_for_all_exercise_styles() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/production_lr_quantlib.json"))
            .expect("independent QuantLib fixture");
    let exercise_times: Vec<f64> = fixture["exercise_days"]
        .as_array()
        .expect("dates")
        .iter()
        .map(|day| day.as_f64().expect("day") / 365.0)
        .collect();
    for case in fixture["cases"].as_array().expect("cases") {
        let params = OptionMarketParams::new(
            100.0,
            100.0,
            0.05,
            case["volatility"].as_f64().expect("sigma"),
            1.0,
            0.02,
            if case["side"] == "call" {
                OptionType::Call
            } else {
                OptionType::Put
            },
        );
        let steps = case["steps"].as_u64().expect("steps") as usize;
        let tree = BinomialTree::leisen_reimer(steps);
        let price = match case["style"].as_str().expect("style") {
            "european" => tree.price_european(&params),
            "american" => tree.price_american(&params),
            "bermudan" => tree.price_bermudan(&params, &exercise_times),
            style => panic!("unknown fixture style {style}"),
        }
        .expect("LR price");
        let expected = case["pv"].as_f64().expect("reference PV");
        assert!(
            (price - expected).abs() < 1e-9,
            "case={case}; actual={price}, independent={expected}"
        );
    }
}

#[test]
fn b6_lr_even_step_request_uses_the_next_odd_grid() {
    assert_eq!(BinomialTree::leisen_reimer(200).steps, 201);
}

#[test]
fn m12_grid_knot_dividend_is_applied_exactly_once() {
    use finstack_quant_models::monte_carlo::discretization::exact_gbm_dividends::ExactGbmWithDividends;
    use finstack_quant_models::monte_carlo::process::gbm_dividends::{Dividend, GbmWithDividends};
    use finstack_quant_models::monte_carlo::traits::Discretization;
    let knot = 9.0 / 365.0;
    let process = GbmWithDividends::with_params(0.0, 0.0, 0.0, vec![(knot, Dividend::Cash(2.0))])
        .expect("process");
    let scheme = ExactGbmWithDividends::new();
    let mut spot = [100.0];
    let grid = [0.0, 2.0 / 365.0, knot, 15.0 / 365.0];
    for interval in grid.windows(2) {
        scheme.step(
            &process,
            interval[0],
            interval[1] - interval[0],
            &mut spot,
            &[0.0, 0.0],
            &mut [],
        );
    }
    assert!(
        (spot[0] - 98.0).abs() < 1e-12,
        "dividend was omitted or repeated: {spot:?}"
    );
}
