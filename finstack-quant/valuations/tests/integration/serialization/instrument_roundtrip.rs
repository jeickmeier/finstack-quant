//! Round-trip instrument examples through the JSON envelope path.
//!
//! For each instrument example we:
//! - Wrap it in `InstrumentEnvelope`
//! - Serialize to JSON
//! - Deserialize back via the manual enum deserializer
//! - Assert the resulting boxed instrument preserves the expected runtime ID
//!
//! This confirms that the envelope format reconstructs each instrument shape
//! without losing the identity needed for runtime dispatch.
//
use finstack_quant_valuations::instruments::*;
//
fn assert_roundtrip(expected_id: &str, json: json_loader::InstrumentJson) {
    let original = json_loader::InstrumentEnvelope {
        schema: "finstack_quant.instrument/1".to_string(),
        instrument: json,
    };
    let s = serde_json::to_string_pretty(&original).unwrap();
    // First deserialize back to an envelope to surface any serde errors clearly
    let envelope: json_loader::InstrumentEnvelope =
        serde_json::from_str(&s).expect("Envelope serde round-trip failed");
    // Then construct the runtime instrument
    let boxed = envelope
        .instrument
        .into_boxed()
        .expect("into_boxed() failed for round-tripped instrument");
    assert_eq!(boxed.id(), expected_id);
}
//
#[test]
fn all_examples_roundtrip() {
    // Fixed Income
    let ex = Bond::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Bond(ex));
    let ex = ConvertibleBond::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::ConvertibleBond(ex));
    let ex = InflationLinkedBond::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InflationLinkedBond(ex));
    let ex = TermLoan::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::TermLoan(ex));
    let ex = RevolvingCredit::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::RevolvingCredit(ex));
    let ex = AgencyMbsPassthrough::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::AgencyMbsPassthrough(ex));
    let ex = AgencyTba::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::AgencyTba(ex));
    let ex = AgencyCmo::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::AgencyCmo(ex));
    let ex = DollarRoll::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::DollarRoll(ex));
    //
    // Rates
    let ex = InterestRateSwap::example_standard().expect("Example should construct");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InterestRateSwap(ex));
    let ex = InflationSwap::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InflationSwap(ex));
    let ex = ForwardRateAgreement::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::ForwardRateAgreement(ex));
    let ex = Swaption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Swaption(ex));
    let ex = InterestRateFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InterestRateFuture(ex));
    let ex = CmsOption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CmsOption(ex));
    let ex = Deposit::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Deposit(ex));
    let ex = Repo::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Repo(ex));
    //
    // Credit
    let ex = CreditDefaultSwap::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CreditDefaultSwap(ex));
    let ex = CDSIndex::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CDSIndex(ex));
    let ex = CDSTranche::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CDSTranche(ex));
    let ex = CDSOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CDSOption(ex));
    //
    // Equity
    let ex = Equity::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Equity(ex));
    let ex = EquityOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::EquityOption(ex));
    let ex = AsianOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::AsianOption(ex));
    let ex = BarrierOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::BarrierOption(ex));
    let ex = LookbackOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::LookbackOption(ex));
    let ex = VarianceSwap::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::VarianceSwap(ex));
    let ex = EquityIndexFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::EquityIndexFuture(ex));
    let ex = VolatilityIndexFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::VolatilityIndexFuture(ex));
    let ex = VolatilityIndexOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::VolatilityIndexOption(ex));
    //
    // FX
    let ex = FxSwap::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxSwap(ex));
    let ex = FxForward::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxForward(ex));
    let ex = Ndf::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Ndf(ex));
    let ex = FxOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxOption(ex));
    let ex = FxDigitalOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxDigitalOption(ex));
    let ex = FxTouchOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxTouchOption(ex));
    let ex = FxBarrierOption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxBarrierOption(ex));
    let ex = FxVarianceSwap::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxVarianceSwap(ex));
    let ex = QuantoOption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::QuantoOption(ex));
    //
    // Commodity
    let ex = CommodityOption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityOption(ex));
    let ex = CommodityAsianOption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityAsianOption(ex));
    let ex = CommodityForward::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityForward(ex));
    let ex = CommoditySwap::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommoditySwap(ex));
    let ex = CommoditySwaption::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommoditySwaption(ex));
    //
    // Exotic Options
    let ex = Autocallable::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Autocallable(ex));
    let ex = CliquetOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CliquetOption(ex));
    let ex = RangeAccrual::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::RangeAccrual(ex));
    //
    // TRS
    let ex = EquityTotalReturnSwap::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::TrsEquity(ex));
    let ex = FIIndexTotalReturnSwap::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::TrsFixedIncomeIndex(ex));
    //
    // Structured Credit
    let ex = StructuredCredit::example();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(
        &id,
        json_loader::InstrumentJson::StructuredCredit(Box::new(ex)),
    );
    //
    // Other
    let ex = Basket::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Basket(ex));
    let ex = PrivateMarketsFund::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::PrivateMarketsFund(ex));
}
