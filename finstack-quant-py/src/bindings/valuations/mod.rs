//! Python bindings for the `finstack-quant-valuations` crate.
//!
//! Exposes the [`PyValuationResult`] envelope for pricing output,
//! JSON-based instrument loading and the standard pricer pipeline.

mod bond_breakeven;
mod bond_quotes;
mod bond_workout;
pub(crate) mod composite;
pub(crate) mod convert;
mod convertible_spot_ladder;
pub(crate) mod instruments;
mod margin;
pub(crate) mod market;
mod merton_mc;
mod pricing;
mod schema;
pub(crate) mod typed_asset_backed_facility;
pub(crate) mod typed_bond_future;
pub(crate) mod typed_equity_future;
pub(crate) mod typed_equity_future_option;
pub(crate) mod typed_fx_future;
pub(crate) mod typed_fx_future_option;
pub(crate) mod typed_interest_rate_future;
pub(crate) mod typed_interest_rate_future_option;
pub(crate) mod typed_macros;
pub(crate) mod typed_volatility_index_future;
pub(crate) mod typed_volatility_index_future_option;

pub(crate) mod typed_agency_cmo;
pub(crate) mod typed_agency_mbs_passthrough;
pub(crate) mod typed_agency_tba;
pub(crate) mod typed_asian_option;
pub(crate) mod typed_autocallable;
pub(crate) mod typed_barrier_option;
pub(crate) mod typed_basis_swap;
pub(crate) mod typed_basket;
pub(crate) mod typed_bermudan_swaption;
pub(crate) mod typed_callable_range_accrual;
pub(crate) mod typed_cliquet_option;
pub(crate) mod typed_cms_option;
pub(crate) mod typed_cms_spread_option;
pub(crate) mod typed_cms_swap;
pub(crate) mod typed_commodity_asian_option;
pub(crate) mod typed_commodity_forward;
pub(crate) mod typed_commodity_future;
pub(crate) mod typed_commodity_future_option;
pub(crate) mod typed_commodity_spot_option;
pub(crate) mod typed_commodity_spread_option;
pub(crate) mod typed_commodity_swap;
pub(crate) mod typed_commodity_swaption;
pub(crate) mod typed_convertible;
pub(crate) mod typed_credit;
pub(crate) mod typed_deposit;
pub(crate) mod typed_dollar_roll;
pub(crate) mod typed_equity;
pub(crate) mod typed_equity_spot;
pub(crate) mod typed_equity_total_return_future;
pub(crate) mod typed_equity_total_return_swap;
pub(crate) mod typed_fi_index_total_return_swap;
pub(crate) mod typed_fra;
pub(crate) mod typed_fx;
pub(crate) mod typed_fx_barrier_option;
pub(crate) mod typed_fx_digital_option;
pub(crate) mod typed_fx_spot;
pub(crate) mod typed_fx_swap;
pub(crate) mod typed_fx_touch_option;
pub(crate) mod typed_fx_variance_swap;
pub(crate) mod typed_inflation_bond;
pub(crate) mod typed_inflation_cap_floor;
pub(crate) mod typed_inflation_swap;
mod typed_legs;
pub(crate) mod typed_levered_real_estate_equity;
pub(crate) mod typed_listed;
pub(crate) mod typed_lookback_option;
pub(crate) mod typed_ndf;
pub(crate) mod typed_private_markets_fund;
pub(crate) mod typed_quanto_option;
pub(crate) mod typed_range_accrual;
pub(crate) mod typed_rates;
pub(crate) mod typed_real_estate_asset;
pub(crate) mod typed_repo;
pub(crate) mod typed_revolving_credit;
pub(crate) mod typed_snowball;
pub(crate) mod typed_structured_credit;
pub(crate) mod typed_tarn;
pub(crate) mod typed_variance_swap;
pub(crate) mod typed_xccy_swap;
pub(crate) mod typed_yoy_inflation_swap;
mod var;

mod json_validation;
mod result;

use crate::bindings::pydoc::pydoc;
use json_validation::{
    instrument_envelope_from_spec, pretty_instrument_json, validate_instrument_json,
    validate_typed_instrument_json,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
pub(crate) use result::PyValuationResult;

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "valuations")?;
    m.setattr("__doc__", pydoc!("finstack_quant.valuations", "__module__"))?;

    m.add_class::<PyValuationResult>()?;
    margin::register(&m)?;
    composite::register(py, &m)?;
    schema::register(py, &m)?;
    register_instruments(py, &m)?;
    register_market(py, &m)?;

    let all = PyList::new(
        py,
        [
            "ValuationResult",
            "calculate_simm",
            "composite",
            "instruments",
            "market",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}

fn register_instruments(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "instruments")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.instruments", "__module__"),
    )?;

    m.add_function(wrap_pyfunction!(validate_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(validate_typed_instrument_json, &m)?)?;
    m.add_function(wrap_pyfunction!(instrument_envelope_from_spec, &m)?)?;
    m.add_function(wrap_pyfunction!(pretty_instrument_json, &m)?)?;
    instruments::register(py, &m)?;
    merton_mc::register(py, &m)?;
    bond_quotes::register(&m)?;
    bond_breakeven::register(&m)?;
    bond_workout::register(&m)?;
    typed_legs::register(py, &m)?;
    typed_rates::register(py, &m)?;
    typed_deposit::register(&m)?;
    typed_fra::register(&m)?;
    typed_inflation_bond::register(&m)?;
    typed_bond_future::register(&m)?;
    typed_interest_rate_future::register(&m)?;
    typed_equity_future::register(&m)?;
    typed_fx_future::register(&m)?;
    typed_volatility_index_future::register(&m)?;
    m.add_class::<typed_listed::PyListedFutureTerms>()?;
    m.add_class::<typed_listed::PyFutureOptionTerms>()?;
    typed_interest_rate_future_option::register(&m)?;
    typed_equity_future_option::register(&m)?;
    typed_fx_future_option::register(&m)?;
    typed_volatility_index_future_option::register(&m)?;

    typed_credit::register(py, &m)?;
    typed_convertible::register(py, &m)?;
    convertible_spot_ladder::register(&m)?;
    typed_equity::register(py, &m)?;
    typed_autocallable::register(&m)?;
    typed_cliquet_option::register(&m)?;
    typed_private_markets_fund::register(&m)?;
    typed_asian_option::register(&m)?;
    typed_barrier_option::register(&m)?;
    typed_lookback_option::register(&m)?;
    typed_range_accrual::register(&m)?;
    typed_tarn::register(&m)?;
    typed_snowball::register(&m)?;
    typed_callable_range_accrual::register(&m)?;
    typed_agency_tba::register(&m)?;
    typed_agency_mbs_passthrough::register(&m)?;
    typed_dollar_roll::register(&m)?;
    typed_agency_cmo::register(&m)?;
    typed_basket::register(&m)?;
    typed_real_estate_asset::register(&m)?;
    typed_levered_real_estate_equity::register(&m)?;
    typed_basis_swap::register(&m)?;
    typed_xccy_swap::register(&m)?;
    typed_cms_swap::register(&m)?;
    typed_cms_option::register(&m)?;
    typed_cms_spread_option::register(&m)?;
    typed_inflation_swap::register(&m)?;
    typed_yoy_inflation_swap::register(&m)?;
    typed_inflation_cap_floor::register(&m)?;
    typed_bermudan_swaption::register(&m)?;
    typed_commodity_future_option::register(&m)?;
    typed_fx_spot::register(&m)?;
    typed_commodity_forward::register(&m)?;
    typed_commodity_future::register(&m)?;
    typed_commodity_asian_option::register(&m)?;
    typed_commodity_spot_option::register(&m)?;
    typed_commodity_spread_option::register(&m)?;
    typed_commodity_swap::register(&m)?;
    typed_commodity_swaption::register(&m)?;
    typed_fx_variance_swap::register(&m)?;
    typed_variance_swap::register(&m)?;
    typed_equity_total_return_swap::register(&m)?;
    typed_equity_total_return_future::register(&m)?;
    typed_fi_index_total_return_swap::register(&m)?;
    typed_equity_spot::register(&m)?;
    typed_repo::register(&m)?;
    typed_fx_swap::register(&m)?;
    typed_ndf::register(&m)?;
    typed_quanto_option::register(&m)?;
    typed_fx_digital_option::register(&m)?;
    typed_fx_touch_option::register(&m)?;
    typed_fx_barrier_option::register(&m)?;
    typed_fx::register(py, &m)?;
    typed_structured_credit::register(py, &m)?;
    typed_revolving_credit::register(py, &m)?;
    typed_asset_backed_facility::register(py, &m)?;
    pricing::register(py, &m)?;
    var::register(&m)?;
    typed_structured_credit::tranche_analytics::register(&m)?;
    let mut exports = vec![
        "validate_instrument_json",
        "validate_typed_instrument_json",
        "instrument_envelope_from_spec",
        "pretty_instrument_json",
    ];
    exports.extend_from_slice(merton_mc::EXPORTS);
    exports.extend_from_slice(bond_quotes::EXPORTS);
    exports.extend_from_slice(bond_breakeven::EXPORTS);
    exports.extend_from_slice(bond_workout::EXPORTS);
    exports.extend_from_slice(typed_structured_credit::tranche_analytics::EXPORTS);
    exports.extend_from_slice(instruments::EXPORTS);
    exports.extend_from_slice(typed_legs::EXPORTS);
    exports.extend_from_slice(typed_rates::EXPORTS);
    exports.extend([
        "Deposit",
        "DepositBuilder",
        "InflationLinkedBond",
        "InflationLinkedBondBuilder",
        "BondFuture",
        "BondFutureBuilder",
        "InterestRateFuture",
        "InterestRateFutureBuilder",
        "ListedFutureTerms",
        "FutureOptionTerms",
        "ForwardRateAgreement",
        "ForwardRateAgreementBuilder",
    ]);
    exports.extend_from_slice(typed_equity_future::EXPORTS);
    exports.extend_from_slice(typed_volatility_index_future::EXPORTS);
    exports.extend_from_slice(typed_interest_rate_future_option::EXPORTS);
    exports.extend_from_slice(typed_equity_future_option::EXPORTS);
    exports.extend_from_slice(typed_volatility_index_future_option::EXPORTS);
    exports.extend_from_slice(typed_fx_future::EXPORTS);
    exports.extend_from_slice(typed_fx_future_option::EXPORTS);
    exports.extend_from_slice(typed_credit::EXPORTS);
    exports.extend_from_slice(typed_convertible::EXPORTS);
    exports.extend_from_slice(convertible_spot_ladder::EXPORTS);
    exports.extend_from_slice(typed_equity::EXPORTS);
    exports.extend_from_slice(typed_autocallable::EXPORTS);
    exports.extend_from_slice(typed_cliquet_option::EXPORTS);
    exports.extend_from_slice(typed_private_markets_fund::EXPORTS);
    exports.extend_from_slice(typed_asian_option::EXPORTS);
    exports.extend_from_slice(typed_barrier_option::EXPORTS);
    exports.extend_from_slice(typed_lookback_option::EXPORTS);
    exports.extend_from_slice(typed_range_accrual::EXPORTS);
    exports.extend_from_slice(typed_tarn::EXPORTS);
    exports.extend_from_slice(typed_snowball::EXPORTS);
    exports.extend_from_slice(typed_callable_range_accrual::EXPORTS);
    exports.extend_from_slice(typed_basket::EXPORTS);
    exports.extend_from_slice(typed_real_estate_asset::EXPORTS);
    exports.extend_from_slice(typed_levered_real_estate_equity::EXPORTS);
    exports.extend_from_slice(&[
        "AgencyTba",
        "AgencyTbaBuilder",
        "AgencyMbsPassthrough",
        "AgencyMbsPassthroughBuilder",
        "DollarRoll",
        "DollarRollBuilder",
        "AgencyCmo",
        "AgencyCmoBuilder",
    ]);
    exports.extend_from_slice(&[
        "BasisSwap",
        "BasisSwapBuilder",
        "XccySwap",
        "XccySwapBuilder",
        "CmsSwap",
        "CmsSwapBuilder",
        "CmsOption",
        "CmsOptionBuilder",
        "CmsSpreadOption",
        "CmsSpreadOptionBuilder",
        "InflationSwap",
        "InflationSwapBuilder",
        "YoYInflationSwap",
        "YoYInflationSwapBuilder",
        "InflationCapFloor",
        "InflationCapFloorBuilder",
        "BermudanSwaption",
        "BermudanSwaptionBuilder",
    ]);
    exports.extend_from_slice(typed_commodity_future_option::EXPORTS);
    exports.extend_from_slice(typed_commodity_forward::EXPORTS);
    exports.extend_from_slice(typed_commodity_future::EXPORTS);
    exports.extend_from_slice(typed_commodity_asian_option::EXPORTS);
    exports.extend_from_slice(typed_commodity_spot_option::EXPORTS);
    exports.extend_from_slice(typed_commodity_spread_option::EXPORTS);
    exports.extend_from_slice(typed_commodity_swap::EXPORTS);
    exports.extend_from_slice(typed_commodity_swaption::EXPORTS);
    exports.extend_from_slice(typed_variance_swap::EXPORTS);
    exports.extend_from_slice(typed_equity_spot::EXPORTS);
    exports.extend_from_slice(&[
        "EquityTotalReturnSwap",
        "EquityTotalReturnSwapBuilder",
        "EquityTotalReturnFuture",
        "EquityTotalReturnFutureBuilder",
        "FiIndexTotalReturnSwap",
        "FiIndexTotalReturnSwapBuilder",
    ]);
    exports.extend_from_slice(&["Repo", "RepoBuilder"]);
    exports.extend_from_slice(typed_fx_spot::EXPORTS);
    exports.extend_from_slice(typed_fx_swap::EXPORTS);
    exports.extend_from_slice(typed_ndf::EXPORTS);
    exports.extend_from_slice(typed_fx_variance_swap::EXPORTS);
    exports.extend_from_slice(typed_quanto_option::EXPORTS);
    exports.extend_from_slice(typed_fx_digital_option::EXPORTS);
    exports.extend_from_slice(typed_fx_touch_option::EXPORTS);
    exports.extend_from_slice(typed_fx_barrier_option::EXPORTS);
    exports.extend_from_slice(typed_fx::EXPORTS);
    exports.extend_from_slice(typed_structured_credit::EXPORTS);
    exports.extend_from_slice(typed_revolving_credit::EXPORTS);
    exports.extend_from_slice(typed_asset_backed_facility::EXPORTS);
    exports.extend_from_slice(pricing::EXPORTS);
    exports.extend_from_slice(var::EXPORTS);
    exports.sort_unstable();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}

fn register_market(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "market")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.valuations.market", "__module__"),
    )?;

    market::register(py, &m)?;
    let mut exports = market::EXPORTS.to_vec();
    exports.sort_unstable();
    let all = PyList::new(py, exports)?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
