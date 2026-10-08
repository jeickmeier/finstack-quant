//! Black-Karasinski short-rate tree reached through bond pricing overrides.
//!
//! The Black-Derman-Toy numbers pinned here were captured before
//! Black-Karasinski became a selectable model, so they also guard the
//! binomial BDT path against drift.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

const AS_OF: Date = date!(2025 - 01 - 01);
const TREE_STEPS: usize = 200;
const SIGMA: f64 = 0.20;

fn market() -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(AS_OF)
        .knots([(0.0, 1.0), (7.0, 0.82)])
        .build()
        .expect("discount curve");
    MarketContext::new().insert(curve)
}

/// 7Y 5% bond callable at `call_price_pct` on 2028-01-01, priced on the
/// rates-only tree selected by `model_config`.
fn callable_bond(call_price_pct: f64, model_config: serde_json::Value) -> Bond {
    let mut bond = Bond::fixed(
        "BK-CALLABLE",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        AS_OF,
        date!(2032 - 01 - 01),
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: date!(2028 - 01 - 01),
            end: date!(2028 - 01 - 01),
            price_pct_of_par: call_price_pct,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = serde_json::from_value(serde_json::json!({
        "market_quotes": { "quoted_clean_price_pct": 103.0 },
        "model_config": model_config,
    }))
    .expect("pricing overrides should deserialize");
    bond
}

fn bdt_config() -> serde_json::Value {
    serde_json::json!({
        "tree_steps": TREE_STEPS,
        "tree_model": "black_derman_toy",
        "bdt_sigma": SIGMA,
    })
}

fn bk_config(kappa: f64) -> serde_json::Value {
    serde_json::json!({
        "tree_steps": TREE_STEPS,
        "tree_model": "black_karasinski",
        "bk_sigma": SIGMA,
        "bk_mean_reversion": kappa,
    })
}

/// Model price at zero OAS and the OAS (decimal) implied by the 103 clean quote.
fn price_and_oas(bond: &Bond) -> (f64, f64) {
    let market = market();
    let mut unquoted = bond.clone();
    unquoted.instrument_pricing_overrides.market_quotes = Default::default();
    let price = unquoted.value(&market, AS_OF).expect("tree price").amount();
    let oas = bond
        .price_with_metrics(&market, AS_OF, &[MetricId::Oas], PricingOptions::default())
        .expect("OAS")
        .measures["oas"];
    (price, oas)
}

#[test]
fn bdt_callable_price_and_oas_are_unchanged() {
    let (price, oas) = price_and_oas(&callable_bond(100.0, bdt_config()));
    assert!((price - 1_068_307.126_309_004).abs() < 1e-6, "{price}");
    assert!((oas - 0.011_723_795_807).abs() < 1e-10, "{oas}");
}

/// As κ → 0 the trinomial Black-Karasinski lattice tends to the binomial
/// Black-Derman-Toy one. Measured at 200 steps, σ = 20%:
///
/// | κ | price − BDT | relative | OAS − BDT |
/// |---|-------------|----------|-----------|
/// | 1e-2 | 186.62 | 1.75e-4 | 1.055 bp |
/// | 1e-3 | 21.09 | 1.97e-5 | 0.087 bp |
/// | 1e-4 | 2.61 | 2.45e-6 | −0.012 bp |
///
/// The residual at κ = 1e-4 is the trinomial-versus-binomial discretization
/// difference (it stays near 0.02 bp of OAS as κ shrinks further), so the
/// bounds below are twice the measured gaps rather than zero.
#[test]
fn black_karasinski_callable_converges_to_bdt_as_mean_reversion_vanishes() {
    let (bdt_price, bdt_oas) = price_and_oas(&callable_bond(100.0, bdt_config()));

    let gaps: Vec<(f64, f64)> = [1e-2, 1e-3, 1e-4]
        .into_iter()
        .map(|kappa| {
            let (price, oas) = price_and_oas(&callable_bond(100.0, bk_config(kappa)));
            (
                ((price - bdt_price) / bdt_price).abs(),
                ((oas - bdt_oas) * 1e4).abs(),
            )
        })
        .collect();

    for pair in gaps.windows(2) {
        assert!(
            pair[1].0 < pair[0].0 && pair[1].1 < pair[0].1,
            "price and OAS gaps to BDT must shrink with κ: {gaps:?}"
        );
    }
    let (price_gap, oas_gap_bp) = gaps[2];
    assert!(
        price_gap < 5e-6,
        "κ = 1e-4 relative price gap to BDT: {price_gap:e}"
    );
    assert!(
        oas_gap_bp < 0.025,
        "κ = 1e-4 OAS gap to BDT: {oas_gap_bp} bp"
    );
}

/// Mean reversion narrows the long-horizon rate distribution, so the issuer
/// call is worth less and the callable bond more than under BDT.
#[test]
fn black_karasinski_mean_reversion_cheapens_the_issuer_call() {
    let (bdt_price, bdt_oas) = price_and_oas(&callable_bond(100.0, bdt_config()));
    let (slow_price, slow_oas) = price_and_oas(&callable_bond(100.0, bk_config(0.03)));
    let (fast_price, fast_oas) = price_and_oas(&callable_bond(100.0, bk_config(0.10)));

    assert!(
        bdt_price < slow_price && slow_price < fast_price,
        "callable price must rise with κ: bdt={bdt_price}, κ=0.03 {slow_price}, κ=0.10 {fast_price}"
    );
    assert!(
        bdt_oas < slow_oas && slow_oas < fast_oas,
        "OAS at a fixed quote must rise with κ: bdt={bdt_oas}, κ=0.03 {slow_oas}, κ=0.10 {fast_oas}"
    );
}

/// With the call struck far out of reach the tree value is the calibrated
/// lattice's own discounting of the fixed cashflows, so it must equal the
/// discount-curve PV. The lattice is calibrated to 0.1 bp per step; the
/// measured difference is below 1e-6 on a 1.135M PV.
#[test]
fn black_karasinski_tree_reprices_the_discount_curve() {
    let mut straight = callable_bond(100.0, bdt_config());
    straight.call_put = None;
    straight.instrument_pricing_overrides = Default::default();
    let discounted = straight
        .value(&market(), AS_OF)
        .expect("straight PV")
        .amount();

    for kappa in [1e-3, 0.03, 0.10] {
        let (tree_price, _) = price_and_oas(&callable_bond(1000.0, bk_config(kappa)));
        let rel = ((tree_price - discounted) / discounted).abs();
        assert!(
            rel < 1e-9,
            "κ={kappa}: tree={tree_price}, discounted={discounted}, rel={rel:e}"
        );
    }
}

/// Both Black-Karasinski inputs are required, and a mean reversion of zero is
/// Black-Derman-Toy, not Black-Karasinski.
#[test]
fn black_karasinski_pricing_rejects_incomplete_inputs() {
    let market = market();
    for (model_config, expected) in [
        (
            serde_json::json!({ "tree_model": "black_karasinski", "bk_mean_reversion": 0.03 }),
            "model_config.bk_sigma",
        ),
        (
            serde_json::json!({ "tree_model": "black_karasinski", "bk_sigma": SIGMA }),
            "model_config.bk_mean_reversion",
        ),
        (
            serde_json::json!({
                "tree_model": "black_karasinski",
                "bk_sigma": SIGMA,
                "bk_mean_reversion": 0.0
            }),
            "model_config.bk_mean_reversion",
        ),
        (
            serde_json::json!({
                "tree_model": "black_derman_toy",
                "bdt_sigma": SIGMA,
                "bk_mean_reversion": 0.03
            }),
            "model_config.bk_mean_reversion but selects the black_derman_toy tree",
        ),
    ] {
        let mut bond = callable_bond(100.0, model_config);
        bond.instrument_pricing_overrides.market_quotes = Default::default();
        let error = bond
            .value(&market, AS_OF)
            .expect_err("incomplete Black-Karasinski inputs must not price");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

/// Vega bumps `bk_sigma` alone (the mean reversion override stays in place);
/// more rate volatility makes the issuer call dearer to the holder.
#[test]
fn black_karasinski_callable_vega_is_negative_for_the_holder() {
    let vega = callable_bond(100.0, bk_config(0.03))
        .price_with_metrics(
            &market(),
            AS_OF,
            &[MetricId::Oas, MetricId::Vega],
            PricingOptions::default(),
        )
        .expect("Black-Karasinski OAS and vega")
        .measures["vega"];
    assert!(vega.is_finite() && vega < 0.0, "vega={vega}");
}
