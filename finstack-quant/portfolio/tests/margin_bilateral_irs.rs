//! Portfolio margin over real interest rate swaps under bilateral regulatory CSAs.
//!
//! Two copies of the canonical `interest_rate_swap.json` example, each carrying
//! its own bilateral SIMM margin spec, are run through
//! `margin::portfolio_margin`. The swaps are close to par, so neither netting
//! set clears the regulatory minimum transfer amount and the variation margin
//! is exactly zero: it must reach the wire as `"0"`, never the Decimal negative
//! zero `"-0"`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::FxHashMap;
use finstack_quant_margin::types::{CsaSpec, OtcMarginSpec};
use finstack_quant_portfolio::margin::portfolio_margin;
use finstack_quant_portfolio::position::{Position, PositionSpec, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::Portfolio;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use time::macros::date;

fn swap_position(id: &str, csa: CsaSpec) -> Position {
    let example: serde_json::Value = serde_json::from_str(include_str!(
        "../../valuations/tests/instruments/json_examples/interest_rate_swap.json"
    ))
    .expect("canonical swap example parses");
    let InstrumentJson::InterestRateSwap(mut swap) =
        serde_json::from_value(example["instrument"].clone()).expect("swap example deserializes")
    else {
        panic!("interest_rate_swap.json is not an interest rate swap");
    };
    swap.id = id.into();
    swap.margin_spec = Some(OtcMarginSpec::bilateral_simm(csa));
    Position::from_spec(PositionSpec {
        position_id: id.into(),
        entity_id: "RATES".into(),
        instrument_id: id.into(),
        instrument_spec: Some(InstrumentJson::InterestRateSwap(swap)),
        quantity: 1.0,
        unit: PositionUnit::Units,
        book_id: None,
        attributes: Default::default(),
        meta: Default::default(),
    })
    .expect("swap position builds from spec")
}

#[test]
fn bilateral_irs_portfolio_margin_reports_finite_im_and_positive_zero_vm() {
    let as_of = date!(2023 - 12 - 28);
    let market = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, (-0.04_f64 * 10.0).exp())])
                .build()
                .expect("USD-OIS curve builds"),
        )
        .insert(
            ForwardCurve::builder("USD-SOFR-3M", 0.25)
                .base_date(as_of)
                .knots([(0.0, 0.04), (10.0, 0.04)])
                .build()
                .expect("USD-SOFR-3M curve builds"),
        );

    let usd = CsaSpec::usd_regulatory().expect("USD regulatory CSA");
    let csa_b = CsaSpec::regulatory_for_currency(Currency::USD, "CSA-B", "USD-OIS")
        .expect("second USD regulatory CSA");
    let portfolio = Portfolio::builder("MARGIN-BOOK")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("RATES"))
        .position(swap_position("IRS-A", usd))
        .position(swap_position("IRS-B", csa_b))
        .build()
        .expect("portfolio builds");

    let result =
        portfolio_margin(&portfolio, &market, as_of, &FxHashMap::default()).expect("margin runs");

    assert_eq!(result.by_netting_set.len(), 2, "one netting set per CSA");
    assert_eq!(result.positions_without_margin, 0);
    assert!(result.total_initial_margin.amount().is_finite());
    assert!(
        result.total_initial_margin.amount() > 0.0,
        "SIMM IM on a 10mm 5y swap"
    );
    for margin in result.by_netting_set.values() {
        let im = margin.initial_margin.amount();
        let vm = margin.variation_margin.amount();
        assert!(
            im.is_finite() && im > 0.0,
            "{}: IM {im}",
            margin.netting_set_id
        );
        assert!(vm.is_finite(), "{}: VM {vm}", margin.netting_set_id);
        assert_eq!(vm, 0.0, "near-par swap stays under the regulatory MTA");
        let wire = serde_json::to_value(margin.variation_margin).expect("VM serializes");
        assert_eq!(wire["amount"], "0", "{}: VM {wire}", margin.netting_set_id);
    }
    let total = serde_json::to_value(result.total_variation_margin).expect("total serializes");
    assert_eq!(total["amount"], "0");
}
