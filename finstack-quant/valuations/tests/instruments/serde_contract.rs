//! Cross-cutting serde contract: every instrument with an `example()` constructor
//! must (a) survive a JSON round-trip and (b) reject unknown fields
//! (the `#[serde(deny_unknown_fields)]` invariant).

/// `$ctor` must be an expression evaluating to the instrument value
/// (call `.expect(...)` inside it for `Result`-returning `example()`).
macro_rules! serde_contract {
    ($name:ident, $ty:ty, $ctor:expr) => {
        #[test]
        fn $name() {
            let inst: $ty = $ctor;
            let value = serde_json::to_value(&inst).expect("serialize to value");
            assert!(
                value.is_object(),
                concat!(stringify!($name), ": expected a top-level JSON object")
            );
            // round-trip
            let _back: $ty = serde_json::from_value(value.clone()).expect("round-trip deserialize");
            // unknown-field rejection
            let mut tampered = value;
            tampered
                .as_object_mut()
                .unwrap()
                .insert("__nonexistent_field__".to_string(), serde_json::json!(123));
            let err = serde_json::from_value::<$ty>(tampered)
                .expect_err("unknown field must be rejected");
            assert!(
                err.to_string().contains("unknown field"),
                concat!(stringify!($name), ": expected unknown-field error, got: {}"),
                err
            );
        }
    };
}

// ---------------------------------------------------------------------------
// Rates
// ---------------------------------------------------------------------------
serde_contract!(
    serde_deposit,
    finstack_quant_valuations::instruments::rates::deposit::Deposit,
    finstack_quant_valuations::instruments::rates::deposit::Deposit::example().expect("example")
);
serde_contract!(
    serde_fra,
    finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement,
    finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement::example()
        .expect("example")
);
serde_contract!(
    serde_cap_floor,
    finstack_quant_valuations::instruments::rates::cap_floor::CapFloor,
    finstack_quant_valuations::instruments::rates::cap_floor::CapFloor::example().expect("example")
);
serde_contract!(
    serde_basis_swap,
    finstack_quant_valuations::instruments::rates::basis_swap::BasisSwap,
    finstack_quant_valuations::instruments::rates::basis_swap::BasisSwap::example()
        .expect("example")
);
serde_contract!(
    serde_ir_future,
    finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture,
    finstack_quant_valuations::instruments::rates::ir_future::InterestRateFuture::example()
        .expect("example")
);
serde_contract!(
    serde_ir_future_option,
    finstack_quant_valuations::instruments::rates::ir_future_option::IrFutureOption,
    finstack_quant_valuations::instruments::rates::ir_future_option::IrFutureOption::example()
        .expect("example")
);
serde_contract!(
    serde_cms_option,
    finstack_quant_valuations::instruments::rates::cms_option::CmsOption,
    finstack_quant_valuations::instruments::rates::cms_option::CmsOption::example()
);
serde_contract!(
    serde_cms_swap,
    finstack_quant_valuations::instruments::rates::cms_swap::CmsSwap,
    finstack_quant_valuations::instruments::rates::cms_swap::CmsSwap::example()
);
serde_contract!(
    serde_cms_spread_option,
    finstack_quant_valuations::instruments::rates::cms_spread_option::CmsSpreadOption,
    finstack_quant_valuations::instruments::rates::cms_spread_option::CmsSpreadOption::example()
);
serde_contract!(
    serde_inflation_swap,
    finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap,
    finstack_quant_valuations::instruments::rates::inflation_swap::InflationSwap::example()
);
serde_contract!(
    serde_yoy_inflation_swap,
    finstack_quant_valuations::instruments::rates::inflation_swap::YoYInflationSwap,
    finstack_quant_valuations::instruments::rates::inflation_swap::YoYInflationSwap::example()
);
serde_contract!(
    serde_inflation_cap_floor,
    finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationCapFloor,
    finstack_quant_valuations::instruments::rates::inflation_cap_floor::InflationCapFloor::example(
    )
);
serde_contract!(
    serde_swaption,
    finstack_quant_valuations::instruments::rates::swaption::Swaption,
    finstack_quant_valuations::instruments::rates::swaption::Swaption::example()
);
serde_contract!(
    serde_bermudan_swaption,
    finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption,
    finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption::example()
);
serde_contract!(
    serde_callable_range_accrual,
    finstack_quant_valuations::instruments::rates::callable_range_accrual::CallableRangeAccrual,
    finstack_quant_valuations::instruments::rates::callable_range_accrual::CallableRangeAccrual::example(
    )
);
serde_contract!(
    serde_range_accrual,
    finstack_quant_valuations::instruments::rates::range_accrual::RangeAccrual,
    finstack_quant_valuations::instruments::rates::range_accrual::RangeAccrual::example()
);
serde_contract!(
    serde_tarn,
    finstack_quant_valuations::instruments::rates::tarn::Tarn,
    finstack_quant_valuations::instruments::rates::tarn::Tarn::example()
);
serde_contract!(
    serde_xccy_swap,
    finstack_quant_valuations::instruments::rates::xccy_swap::XccySwap,
    finstack_quant_valuations::instruments::rates::xccy_swap::XccySwap::example()
);
serde_contract!(
    serde_repo,
    finstack_quant_valuations::instruments::rates::repo::Repo,
    finstack_quant_valuations::instruments::rates::repo::Repo::example()
);

// ---------------------------------------------------------------------------
// FX
// ---------------------------------------------------------------------------
serde_contract!(
    serde_fx_spot,
    finstack_quant_valuations::instruments::fx::fx_spot::FxSpot,
    finstack_quant_valuations::instruments::fx::fx_spot::FxSpot::example().expect("example")
);
serde_contract!(
    serde_fx_forward,
    finstack_quant_valuations::instruments::fx::fx_forward::FxForward,
    finstack_quant_valuations::instruments::fx::fx_forward::FxForward::example().expect("example")
);
serde_contract!(
    serde_fx_option,
    finstack_quant_valuations::instruments::fx::fx_option::FxOption,
    finstack_quant_valuations::instruments::fx::fx_option::FxOption::example().expect("example")
);
serde_contract!(
    serde_fx_digital_option,
    finstack_quant_valuations::instruments::fx::fx_digital_option::FxDigitalOption,
    finstack_quant_valuations::instruments::fx::fx_digital_option::FxDigitalOption::example()
        .expect("example")
);
serde_contract!(
    serde_fx_touch_option,
    finstack_quant_valuations::instruments::fx::fx_touch_option::FxTouchOption,
    finstack_quant_valuations::instruments::fx::fx_touch_option::FxTouchOption::example()
        .expect("example")
);
serde_contract!(
    serde_fx_swap,
    finstack_quant_valuations::instruments::fx::fx_swap::FxSwap,
    finstack_quant_valuations::instruments::fx::fx_swap::FxSwap::example()
);
serde_contract!(
    serde_fx_barrier_option,
    finstack_quant_valuations::instruments::fx::fx_barrier_option::FxBarrierOption,
    finstack_quant_valuations::instruments::fx::fx_barrier_option::FxBarrierOption::example()
);
serde_contract!(
    serde_fx_variance_swap,
    finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap,
    finstack_quant_valuations::instruments::fx::fx_variance_swap::FxVarianceSwap::example()
);
serde_contract!(
    serde_quanto_option,
    finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption,
    finstack_quant_valuations::instruments::fx::quanto_option::QuantoOption::example()
);
serde_contract!(
    serde_ndf,
    finstack_quant_valuations::instruments::fx::ndf::Ndf,
    finstack_quant_valuations::instruments::fx::ndf::Ndf::example()
);

// ---------------------------------------------------------------------------
// Credit derivatives
// ---------------------------------------------------------------------------
serde_contract!(
    serde_cds,
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap,
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap::example()
);
serde_contract!(
    serde_cds_index,
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex,
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex::example()
);
serde_contract!(
    serde_cds_option,
    finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption,
    finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption::example()
        .expect("example")
);
serde_contract!(
    serde_cds_tranche,
    finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche,
    finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche::example()
);

// ---------------------------------------------------------------------------
// Equity
// ---------------------------------------------------------------------------
serde_contract!(
    serde_equity_spot,
    finstack_quant_valuations::instruments::equity::spot::Equity,
    finstack_quant_valuations::instruments::equity::spot::Equity::example()
);
serde_contract!(
    serde_equity_option,
    finstack_quant_valuations::instruments::equity::equity_option::EquityOption,
    finstack_quant_valuations::instruments::equity::equity_option::EquityOption::example()
        .expect("example")
);
serde_contract!(
    serde_equity_index_future,
    finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture,
    finstack_quant_valuations::instruments::equity::equity_index_future::EquityIndexFuture::example()
        .expect("example")
);
serde_contract!(
    serde_equity_trs,
    finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap,
    finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap::example()
        .expect("example")
);
serde_contract!(
    serde_variance_swap,
    finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap,
    finstack_quant_valuations::instruments::equity::variance_swap::VarianceSwap::example()
        .expect("example")
);
serde_contract!(
    serde_vol_index_future,
    finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture,
    finstack_quant_valuations::instruments::equity::vol_index_future::VolatilityIndexFuture::example()
        .expect("example")
);
serde_contract!(
    serde_vol_index_option,
    finstack_quant_valuations::instruments::equity::vol_index_option::VolatilityIndexOption,
    finstack_quant_valuations::instruments::equity::vol_index_option::VolatilityIndexOption::example()
        .expect("example")
);
serde_contract!(
    serde_dcf_equity,
    finstack_quant_valuations::instruments::equity::dcf_equity::DiscountedCashFlow,
    finstack_quant_valuations::instruments::equity::dcf_equity::DiscountedCashFlow::example()
        .expect("example")
);
serde_contract!(
    serde_pe_fund,
    finstack_quant_valuations::instruments::equity::pe_fund::PrivateMarketsFund,
    finstack_quant_valuations::instruments::equity::pe_fund::PrivateMarketsFund::example()
        .expect("example")
);
serde_contract!(
    serde_real_estate,
    finstack_quant_valuations::instruments::equity::real_estate::RealEstateAsset,
    finstack_quant_valuations::instruments::equity::real_estate::RealEstateAsset::example()
        .expect("example")
);
serde_contract!(
    serde_real_estate_levered,
    finstack_quant_valuations::instruments::equity::real_estate::LeveredRealEstateEquity,
    finstack_quant_valuations::instruments::equity::real_estate::LeveredRealEstateEquity::example()
        .expect("example")
);
serde_contract!(
    serde_autocallable,
    finstack_quant_valuations::instruments::equity::autocallable::Autocallable,
    finstack_quant_valuations::instruments::equity::autocallable::Autocallable::example()
        .expect("example")
);
serde_contract!(
    serde_cliquet_option,
    finstack_quant_valuations::instruments::equity::cliquet_option::CliquetOption,
    finstack_quant_valuations::instruments::equity::cliquet_option::CliquetOption::example()
        .expect("example")
);

// ---------------------------------------------------------------------------
// Exotics
// ---------------------------------------------------------------------------
serde_contract!(
    serde_asian_option,
    finstack_quant_valuations::instruments::exotics::asian_option::AsianOption,
    finstack_quant_valuations::instruments::exotics::asian_option::AsianOption::example()
        .expect("example")
);
serde_contract!(
    serde_barrier_option,
    finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption,
    finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption::example()
        .expect("example")
);
serde_contract!(
    serde_basket,
    finstack_quant_valuations::instruments::exotics::basket::Basket,
    finstack_quant_valuations::instruments::exotics::basket::Basket::example().expect("example")
);
serde_contract!(
    serde_lookback_option,
    finstack_quant_valuations::instruments::exotics::lookback_option::LookbackOption,
    finstack_quant_valuations::instruments::exotics::lookback_option::LookbackOption::example()
        .expect("example")
);

// ---------------------------------------------------------------------------
// Fixed income
// ---------------------------------------------------------------------------
serde_contract!(
    serde_bond,
    finstack_quant_valuations::instruments::fixed_income::bond::Bond,
    finstack_quant_valuations::instruments::fixed_income::bond::Bond::example().expect("example")
);
serde_contract!(
    serde_bond_future,
    finstack_quant_valuations::instruments::fixed_income::bond_future::BondFuture,
    finstack_quant_valuations::instruments::fixed_income::bond_future::BondFuture::example()
        .expect("example")
);
serde_contract!(
    serde_cmo,
    finstack_quant_valuations::instruments::fixed_income::cmo::AgencyCmo,
    finstack_quant_valuations::instruments::fixed_income::cmo::AgencyCmo::example()
        .expect("example")
);
serde_contract!(
    serde_convertible,
    finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond,
    finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond::example()
        .expect("example")
);
serde_contract!(
    serde_dollar_roll,
    finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll,
    finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll::example()
        .expect("example")
);
serde_contract!(
    serde_fi_trs,
    finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap,
    finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap::example()
        .expect("example")
);
serde_contract!(
    serde_inflation_linked_bond,
    finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond,
    finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond::example()
);
serde_contract!(
    serde_mbs_passthrough,
    finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough,
    finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough::example(
    )
    .expect("example")
);
serde_contract!(
    serde_revolving_credit,
    finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit,
    finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit::example()
        .expect("example")
);
serde_contract!(
    serde_structured_credit,
    finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit,
    finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit::example()
);
serde_contract!(
    serde_tba,
    finstack_quant_valuations::instruments::fixed_income::tba::AgencyTba,
    finstack_quant_valuations::instruments::fixed_income::tba::AgencyTba::example()
        .expect("example")
);
serde_contract!(
    serde_term_loan,
    finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan,
    finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan::example()
        .expect("example")
);

// ---------------------------------------------------------------------------
// Commodity
//
// All six commodity instrument types flatten a shared `CommodityUnderlyingParams`
// leg via `#[serde(flatten)]`, which serde does not support alongside its native
// `#[serde(deny_unknown_fields)]`. They instead enforce the "unknown fields
// denied" invariant with a trailing `UnknownFieldGuard` flatten field (see
// `common_impl::serde_guard`), so they carry the full contract like every other
// instrument while keeping the flat v1 wire format/schema unchanged.
// ---------------------------------------------------------------------------
serde_contract!(
    serde_commodity_forward,
    finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward,
    finstack_quant_valuations::instruments::commodity::commodity_forward::CommodityForward::example(
    )
);
serde_contract!(
    serde_commodity_option,
    finstack_quant_valuations::instruments::commodity::commodity_option::CommodityOption,
    finstack_quant_valuations::instruments::commodity::commodity_option::CommodityOption::example()
);
serde_contract!(
    serde_commodity_swap,
    finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap,
    finstack_quant_valuations::instruments::commodity::commodity_swap::CommoditySwap::example()
);
serde_contract!(
    serde_commodity_swaption,
    finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption,
    finstack_quant_valuations::instruments::commodity::commodity_swaption::CommoditySwaption::example()
);
serde_contract!(
    serde_commodity_asian_option,
    finstack_quant_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption,
    finstack_quant_valuations::instruments::commodity::commodity_asian_option::CommodityAsianOption::example()
);
serde_contract!(
    serde_commodity_spread_option,
    finstack_quant_valuations::instruments::commodity::commodity_spread_option::CommoditySpreadOption,
    finstack_quant_valuations::instruments::commodity::commodity_spread_option::CommoditySpreadOption::example()
        .expect("example")
);
