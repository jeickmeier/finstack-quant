use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_models::rates::clock::model_time_on_curve;
use finstack_quant_models::volatility::sabr::SabrParameters;
use serde_json::{Value, json};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut checks = Vec::new();
    for name in [
        "cdx_ig_46_cdso_vol_isolated_last_step.json",
        "credit_vol_large_10x21.json",
    ] {
        let load = |root: &str| -> Value {
            serde_json::from_str(
                &std::fs::read_to_string(std::path::Path::new(root).join(name)).unwrap(),
            )
            .unwrap()
        };
        let before = load(&args[1]);
        let after = load(&args[2]);
        let market: MarketContext =
            serde_json::from_value(after["result"]["final_market"].clone()).unwrap();
        let discount = market
            .get_discount(if name.starts_with("credit_vol_large") {
                "CREDIT-FLAT"
            } else {
                "USD-OIS"
            })
            .unwrap();
        assert_eq!(discount.base_date().to_string(), "2026-05-08");
        let report = |value: &Value| -> Vec<Value> {
            value["result"]["step_reports"]
                .as_object()
                .unwrap()
                .values()
                .flat_map(|report| report["fitted_slices"].as_array().unwrap().iter().cloned())
                .collect()
        };
        for (old, new) in report(&before).iter().zip(report(&after)) {
            let time = new["expiry"].as_f64().unwrap();
            let curve_time = model_time_on_curve(
                discount.base_date(),
                time,
                discount.base_date(),
                discount.day_count(),
            )
            .unwrap();
            let forward = 0.01 / discount.df_between_times(0.0, curve_time).unwrap();
            let old_parameters: SabrParameters =
                serde_json::from_value(old["parameters"]["sabr"].clone()).unwrap();
            let new_parameters: SabrParameters =
                serde_json::from_value(new["parameters"]["sabr"].clone()).unwrap();
            let mut points = Vec::new();
            let mut max_delta = 0.0_f64;
            for i in 0..81 {
                let strike = 0.002 + i as f64 * 0.000225;
                let old_vol = old_parameters.implied_vol(forward, strike, time).unwrap();
                let new_vol = new_parameters.implied_vol(forward, strike, time).unwrap();
                max_delta = max_delta.max((new_vol - old_vol).abs());
                points.push(json!({"strike":strike, "before":old_vol, "after":new_vol}));
            }
            assert!(max_delta < 1e-9, "off-grid change {max_delta} at {time}");
            checks.push(json!({"case": name, "expiry":time, "forward":forward, "max_abs_vol_delta":max_delta,"points":points}));
        }
    }
    println!("{}", serde_json::to_string_pretty(&checks).unwrap());
}
