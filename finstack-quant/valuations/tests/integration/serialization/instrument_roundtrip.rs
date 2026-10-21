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
use finstack_quant_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption;
use finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward;
use finstack_quant_valuations::instruments::commodity::commodity_future::CommodityFuture;
use finstack_quant_valuations::instruments::commodity::commodity_future_option::CommodityFutureOption;
use finstack_quant_valuations::instruments::commodity::commodity_spot_option::CommoditySpotOption;
use finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap;
use finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CdsIndex;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CdsOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CdsTranche;
use finstack_quant_valuations::instruments::equity::autocallable::Autocallable;
use finstack_quant_valuations::instruments::equity::cliquet_option::CliquetOption;
use finstack_quant_valuations::instruments::equity::equity_future::EquityFuture;
use finstack_quant_valuations::instruments::equity::equity_future_option::EquityFutureOption;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::equity::equity_total_return_future::EquityTotalReturnFuture;
use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
use finstack_quant_valuations::instruments::equity::pe_fund::PrivateMarketsFund;
use finstack_quant_valuations::instruments::equity::spot::Equity;
use finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap;
use finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture;
use finstack_quant_valuations::instruments::exotics::asian_option::AsianOption;
use finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations::instruments::exotics::basket::Basket;
use finstack_quant_valuations::instruments::exotics::lookback_option::LookbackOption;
use finstack_quant_valuations::instruments::exotics::range_accrual::RangeAccrual;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::cmo::AgencyCmo;
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
use finstack_quant_valuations::instruments::fixed_income::tba::AgencyTba;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::fx::fx_barrier_option::FxBarrierOption;
use finstack_quant_valuations::instruments::fx::fx_digital_option::FxDigitalOption;
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::fx::fx_future::FxFuture;
use finstack_quant_valuations::instruments::fx::fx_future_option::FxFutureOption;
use finstack_quant_valuations::instruments::fx::fx_option::FxOption;
use finstack_quant_valuations::instruments::fx::fx_swap::FxSwap;
use finstack_quant_valuations::instruments::fx::fx_touch_option::FxTouchOption;
use finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap;
use finstack_quant_valuations::instruments::fx::ndf::Ndf;
use finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption;
use finstack_quant_valuations::instruments::rates::cms_option::CmsOption;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap;
use finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture;
use finstack_quant_valuations::instruments::rates::ir_future_option::InterestRateFutureOption;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::rates::repo::Repo;
use finstack_quant_valuations::instruments::rates::swaption::Swaption;
use finstack_quant_valuations::instruments::*;
//
fn assert_roundtrip(expected_id: &str, json: json_loader::InstrumentJson) {
    let original = json_loader::InstrumentEnvelope {
        schema: finstack_quant_valuations::instruments::json_loader::InstrumentSchema::CURRENT,
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
    let ex = InflationLinkedBond::example().expect("example");
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
    let ex = InterestRateSwap::example().expect("Example should construct");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InterestRateSwap(ex));
    let ex = InflationSwap::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InflationSwap(ex));
    let ex = ForwardRateAgreement::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::ForwardRateAgreement(ex));
    let ex = Swaption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Swaption(ex));
    let ex = InterestRateFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::InterestRateFuture(ex));
    let ex = CmsOption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CmsOption(ex));
    let ex = Deposit::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Deposit(ex));
    let ex = Repo::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::Repo(ex));
    //
    // Credit
    let ex = CreditDefaultSwap::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CreditDefaultSwap(ex));
    let ex = CdsIndex::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CdsIndex(ex));
    let ex = CdsTranche::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CdsTranche(ex));
    let ex = CdsOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CdsOption(ex));
    //
    // Equity
    let ex = Equity::example().expect("example");
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
    let ex = EquityFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::EquityFuture(ex));
    let ex = VolatilityIndexFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::VolatilityIndexFuture(ex));
    let ex = InterestRateFutureOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(
        &id,
        json_loader::InstrumentJson::InterestRateFutureOption(ex),
    );
    let ex = EquityFutureOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::EquityFutureOption(ex));
    let ex = FxFutureOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxFutureOption(ex));
    let ex = CommodityFutureOption::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityFutureOption(ex));
    let ex = CommodityFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityFuture(ex));
    let ex = FxFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxFuture(ex));
    let ex = EquityTotalReturnFuture::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(
        &id,
        json_loader::InstrumentJson::EquityTotalReturnFuture(ex),
    );
    //
    // FX
    let ex = FxSwap::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxSwap(ex));
    let ex = FxForward::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxForward(ex));
    let ex = Ndf::example().expect("example");
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
    let ex = FxBarrierOption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxBarrierOption(ex));
    let ex = FxVarianceSwap::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::FxVarianceSwap(ex));
    let ex = QuantoOption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::QuantoOption(ex));
    //
    // Commodity
    let ex = CommoditySpotOption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommoditySpotOption(ex));
    let ex = CommodityAsianOption::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityAsianOption(ex));
    let ex = CommodityForward::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommodityForward(ex));
    let ex = CommoditySwap::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::CommoditySwap(ex));
    let ex = CommoditySwaption::example().expect("example");
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
    let ex = RangeAccrual::example().expect("example");
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::RangeAccrual(ex));
    //
    // TRS
    let ex = EquityTotalReturnSwap::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::TrsEquity(ex));
    let ex = FiIndexTotalReturnSwap::example().unwrap();
    let id = ex.id.as_str().to_string();
    assert_roundtrip(&id, json_loader::InstrumentJson::TrsFixedIncomeIndex(ex));
    //
    // Structured Credit
    let ex = StructuredCredit::example().expect("example");
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
