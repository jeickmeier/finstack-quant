//! Shared argument extraction helpers for PyO3 bindings.
//!
//! A Python `str` argument is a value (a label, a code, a date), never a JSON
//! document. A parameter that takes a typed object is declared with its
//! `#[pyclass]` type, so no helper is needed for it; JSON text reaches such a
//! parameter through the class's own `from_json`.
//!
//! What remains here is the one input with two structured forms and the unit
//! coercers:
//!
//! - [`extract_instrument`]: a typed instrument instance (cloned, no JSON) or
//!   a canonical envelope `dict`. An envelope dict is the only form for an
//!   instrument that has no typed class.
//! - [`extract_instrument_json`]: the same input as envelope text, for Rust
//!   APIs that take text.
//! - [`envelope_json_text`]: envelope `dict` or text, for the validators whose
//!   subject is the envelope document itself.

use pyo3::prelude::*;

use crate::bindings::valuations::composite::PyCompositeInstrument;
use crate::bindings::valuations::convert::serialize_typed_instrument_json;
use crate::bindings::valuations::instruments::{PyBond, PyTermLoan};
use crate::bindings::valuations::typed_agency_cmo::PyAgencyCmo;
use crate::bindings::valuations::typed_agency_mbs_passthrough::PyAgencyMbsPassthrough;
use crate::bindings::valuations::typed_agency_tba::PyAgencyTba;
use crate::bindings::valuations::typed_asian_option::PyAsianOption;
use crate::bindings::valuations::typed_asset_backed_facility::PyAssetBackedFacility;
use crate::bindings::valuations::typed_autocallable::PyAutocallable;
use crate::bindings::valuations::typed_barrier_option::PyBarrierOption;
use crate::bindings::valuations::typed_basis_swap::PyBasisSwap;
use crate::bindings::valuations::typed_basket::PyBasket;
use crate::bindings::valuations::typed_bermudan_swaption::PyBermudanSwaption;
use crate::bindings::valuations::typed_bond_future::PyBondFuture;
use crate::bindings::valuations::typed_callable_range_accrual::PyCallableRangeAccrual;
use crate::bindings::valuations::typed_cliquet_option::PyCliquetOption;
use crate::bindings::valuations::typed_cms_option::PyCmsOption;
use crate::bindings::valuations::typed_cms_spread_option::PyCmsSpreadOption;
use crate::bindings::valuations::typed_cms_swap::PyCmsSwap;
use crate::bindings::valuations::typed_commodity_asian_option::PyCommodityAsianOption;
use crate::bindings::valuations::typed_commodity_forward::PyCommodityForward;
use crate::bindings::valuations::typed_commodity_future::PyCommodityFuture;
use crate::bindings::valuations::typed_commodity_future_option::PyCommodityFutureOption;
use crate::bindings::valuations::typed_commodity_spot_option::PyCommoditySpotOption;
use crate::bindings::valuations::typed_commodity_spread_option::PyCommoditySpreadOption;
use crate::bindings::valuations::typed_commodity_swap::PyCommoditySwap;
use crate::bindings::valuations::typed_commodity_swaption::PyCommoditySwaption;
use crate::bindings::valuations::typed_convertible::PyConvertibleBond;
use crate::bindings::valuations::typed_credit::{
    PyCdsIndex, PyCdsOption, PyCdsTranche, PyCreditDefaultSwap,
};
use crate::bindings::valuations::typed_deposit::PyDeposit;
use crate::bindings::valuations::typed_dollar_roll::PyDollarRoll;
use crate::bindings::valuations::typed_equity::PyEquityOption;
use crate::bindings::valuations::typed_equity_future::PyEquityFuture;
use crate::bindings::valuations::typed_equity_future_option::PyEquityFutureOption;
use crate::bindings::valuations::typed_equity_spot::PyEquity;
use crate::bindings::valuations::typed_equity_total_return_future::PyEquityTotalReturnFuture;
use crate::bindings::valuations::typed_equity_total_return_swap::PyEquityTotalReturnSwap;
use crate::bindings::valuations::typed_fi_index_total_return_swap::PyFiIndexTotalReturnSwap;
use crate::bindings::valuations::typed_fra::PyForwardRateAgreement;
use crate::bindings::valuations::typed_fx::{PyFxForward, PyFxOption};
use crate::bindings::valuations::typed_fx_barrier_option::PyFxBarrierOption;
use crate::bindings::valuations::typed_fx_digital_option::PyFxDigitalOption;
use crate::bindings::valuations::typed_fx_future::PyFxFuture;
use crate::bindings::valuations::typed_fx_future_option::PyFxFutureOption;
use crate::bindings::valuations::typed_fx_spot::PyFxSpot;
use crate::bindings::valuations::typed_fx_swap::PyFxSwap;
use crate::bindings::valuations::typed_fx_touch_option::PyFxTouchOption;
use crate::bindings::valuations::typed_fx_variance_swap::PyFxVarianceSwap;
use crate::bindings::valuations::typed_inflation_bond::PyInflationLinkedBond;
use crate::bindings::valuations::typed_inflation_cap_floor::PyInflationCapFloor;
use crate::bindings::valuations::typed_inflation_swap::PyInflationSwap;
use crate::bindings::valuations::typed_interest_rate_future::PyInterestRateFuture;
use crate::bindings::valuations::typed_interest_rate_future_option::PyInterestRateFutureOption;
use crate::bindings::valuations::typed_levered_real_estate_equity::PyLeveredRealEstateEquity;
use crate::bindings::valuations::typed_lookback_option::PyLookbackOption;
use crate::bindings::valuations::typed_ndf::PyNdf;
use crate::bindings::valuations::typed_private_markets_fund::PyPrivateMarketsFund;
use crate::bindings::valuations::typed_quanto_option::PyQuantoOption;
use crate::bindings::valuations::typed_range_accrual::PyRangeAccrual;
use crate::bindings::valuations::typed_rates::{PyCapFloor, PyInterestRateSwap, PySwaption};
use crate::bindings::valuations::typed_real_estate_asset::PyRealEstateAsset;
use crate::bindings::valuations::typed_repo::PyRepo;
use crate::bindings::valuations::typed_revolving_credit::PyRevolvingCredit;
use crate::bindings::valuations::typed_snowball::PySnowball;
use crate::bindings::valuations::typed_structured_credit::{
    PyStructuredCredit, PyStructuredCreditTranche,
};
use crate::bindings::valuations::typed_tarn::PyTarn;
use crate::bindings::valuations::typed_variance_swap::PyVarianceSwap;
use crate::bindings::valuations::typed_volatility_index_future::PyVolatilityIndexFuture;
use crate::bindings::valuations::typed_volatility_index_future_option::PyVolatilityIndexFutureOption;
use crate::bindings::valuations::typed_xccy_swap::PyXccySwap;
use crate::bindings::valuations::typed_yoy_inflation_swap::PyYoYInflationSwap;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::pricer::ParsedInstrument;

// Instrument — typed-or-JSON extraction

/// An instrument argument: a typed instance's native payload, or canonical
/// envelope JSON text serialized from an envelope `dict`.
///
/// The typed arm skips the JSON round trip; the envelope arm parses through
/// the Rust envelope loader. Both prepare instruments identically: overrides
/// merge before validation, and validation runs once.
pub enum InstrumentArg {
    /// Clone of a typed instance's inner Rust instrument, not yet validated.
    Typed(Box<InstrumentJson>),
    /// Canonical `finstack_quant.instrument/1` envelope JSON text.
    Envelope(String),
}

impl InstrumentArg {
    /// Merge optional metric pricing overrides, validate, and box the
    /// instrument for pricing dispatch.
    ///
    /// # Arguments
    ///
    /// * `metric_pricing_overrides` - Optional `MetricPricingOverrides` JSON
    ///   patch merged before validation; `None` keeps stored controls.
    /// * `instrument_pricing_overrides` - Optional patch for instrument-owned
    ///   quotes and models, applied to the owned pricing copy.
    pub fn into_parsed(
        self,
        metric_pricing_overrides: Option<&str>,
        instrument_pricing_overrides: Option<&serde_json::Value>,
    ) -> finstack_quant_core::Result<ParsedInstrument> {
        match self {
            Self::Typed(instrument) => ParsedInstrument::from_instrument_json(
                *instrument,
                metric_pricing_overrides,
                instrument_pricing_overrides,
            ),
            Self::Envelope(json) => {
                finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
                    &json,
                    metric_pricing_overrides,
                    instrument_pricing_overrides,
                )
            }
        }
    }

    /// Validate the instrument and return its tagged payload.
    pub fn into_instrument_json(self) -> finstack_quant_core::Result<InstrumentJson> {
        match self {
            Self::Typed(instrument) => {
                instrument.validate_for_pricing()?;
                Ok(*instrument)
            }
            Self::Envelope(json) => {
                finstack_quant_valuations::pricer::parse_instrument_from_json(&json)
            }
        }
    }
}

/// Extract an instrument argument from a typed instrument object or an
/// envelope `dict`. A `str` is refused: envelope text loads with
/// `json.loads`, or with the instrument class's `from_json`.
///
/// # Extending this function
///
/// Every typed instrument class is wired in here by adding exactly **one**
/// cast arm (its `inner` converts through `InstrumentJson::from`), ordered
/// before the final envelope fallback. Update the fallback
/// error message below to name each newly landed class.
///
/// Currently wired: `Bond`, `TermLoan`, `InterestRateSwap`, `Swaption`,
/// `CapFloor`, `CreditDefaultSwap`, `CdsIndex`, `FxForward`, `FxOption`,
/// `CdsTranche`, `CdsOption`, `ConvertibleBond`, `EquityOption`, `StructuredCredit`,
/// `RevolvingCredit`, `AssetBackedFacility`, `CompositeInstrument`.
pub fn extract_instrument(obj: &Bound<'_, PyAny>) -> PyResult<InstrumentArg> {
    macro_rules! typed {
        ($($py:ty),+ $(,)?) => {
            $(
                if let Ok(typed) = obj.cast::<$py>() {
                    let inner = typed.borrow().inner.clone();
                    return Ok(InstrumentArg::Typed(Box::new(InstrumentJson::from(inner))));
                }
            )+
        };
    }
    typed!(
        PyCompositeInstrument,
        PyBond,
        PyTermLoan,
        PyRevolvingCredit,
        PyAssetBackedFacility,
        PyInterestRateSwap,
        PyDeposit,
        PyInflationLinkedBond,
        PyBondFuture,
        PyInterestRateFuture,
        PyEquityFuture,
        PyFxFuture,
        PyVolatilityIndexFuture,
        PyInterestRateFutureOption,
        PyEquityFutureOption,
        PyFxFutureOption,
        PyVolatilityIndexFutureOption,
        PyForwardRateAgreement,
        PySwaption,
        PyCapFloor,
        PyCreditDefaultSwap,
        PyCdsIndex,
        PyFxForward,
        PyFxOption,
        PyCdsOption,
        PyCdsTranche,
        PyConvertibleBond,
        PyEquity,
        PyEquityOption,
        PyAutocallable,
        PyCliquetOption,
        PyPrivateMarketsFund,
        PyAsianOption,
        PyBarrierOption,
        PyLookbackOption,
        PyRangeAccrual,
        PyTarn,
        PySnowball,
        PyCallableRangeAccrual,
        PyAgencyTba,
        PyAgencyMbsPassthrough,
        PyDollarRoll,
        PyAgencyCmo,
        PyBasket,
        PyRealEstateAsset,
        PyLeveredRealEstateEquity,
        PyBasisSwap,
        PyXccySwap,
        PyCmsSwap,
        PyCmsOption,
        PyCmsSpreadOption,
        PyInflationSwap,
        PyYoYInflationSwap,
        PyInflationCapFloor,
        PyBermudanSwaption,
        PyCommodityFutureOption,
        PyFxSpot,
        PyCommodityForward,
        PyCommodityFuture,
        PyCommodityAsianOption,
        PyCommoditySpotOption,
        PyCommoditySpreadOption,
        PyCommoditySwap,
        PyCommoditySwaption,
        PyFxVarianceSwap,
        PyVarianceSwap,
        PyEquityTotalReturnSwap,
        PyEquityTotalReturnFuture,
        PyFiIndexTotalReturnSwap,
        PyRepo,
        PyFxSwap,
        PyNdf,
        PyQuantoOption,
        PyFxDigitalOption,
        PyFxTouchOption,
        PyFxBarrierOption,
        PyStructuredCredit,
        PyStructuredCreditTranche,
    );
    if obj.is_instance_of::<pyo3::types::PyDict>() {
        return crate::bindings::module_utils::py_to_json_string(obj.py(), obj, "instrument")
            .map(InstrumentArg::Envelope);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected a typed instrument instance or a canonical instrument envelope dict",
    ))
}

/// Extract canonical instrument-envelope JSON text from a typed instrument
/// object or an envelope `dict`.
///
/// Typed instances are serialized to the same `InstrumentEnvelope` the JSON
/// loader parses. Use this only where the Rust API takes envelope text;
/// callers that need the instrument itself use [`extract_instrument`], which
/// skips the serialize-and-parse round trip.
pub fn extract_instrument_json(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    match extract_instrument(obj)? {
        InstrumentArg::Typed(instrument) => {
            let what = instrument.type_tag();
            serialize_typed_instrument_json(*instrument, what)
        }
        InstrumentArg::Envelope(json) => Ok(json),
    }
}

/// Canonical instrument-envelope JSON text from an envelope `dict` (as
/// returned by `instrument_envelope_from_spec`) or pre-serialized JSON `str`.
///
/// Only for the validators whose subject is the envelope document itself
/// (`validate_instrument_json` and its siblings). A `str` is passed through
/// unchanged so the Rust parser reports its own errors; a `dict` is serialized
/// with `json.dumps`.
///
/// # Errors
///
/// Raises `TypeError` for any other Python type and `ValueError` when a
/// `dict` is not JSON-serializable.
pub fn envelope_json_text(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = obj.extract::<String>() {
        return Ok(text);
    }
    if obj.is_instance_of::<pyo3::types::PyDict>() {
        return crate::bindings::module_utils::py_to_json_string(obj.py(), obj, "instrument");
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected a canonical instrument envelope as a dict or JSON string",
    ))
}

// Rate / spread — typed-or-float unit extraction

/// Extract a decimal rate from a `float | int | Rate | Bps`.
///
/// The one rate coercer of the bindings. A `Rate` wrapper contributes
/// `Rate::as_decimal()`; `Bps` is converted through `as_decimal()`. A bare
/// number is taken as an already-decimal rate (`0.05` for 5%). No percent/bp
/// coercion is applied to bare numbers — a caller holding basis points should
/// pass a `Bps`. Percent quotes should be converted with `Rate::from_percent`
/// before extraction.
///
/// # Arguments
///
/// * `obj` - A Python `float`/`int` in decimal form, a
///   `finstack_quant.core.types.Rate` or a `finstack_quant.core.types.Bps`.
/// * `what` - Parameter name used in the `TypeError` message.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither numeric nor a `Rate`/`Bps`.
pub fn extract_rate_decimal(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if let Ok(rate) = obj.cast::<crate::bindings::core::types::PyRate>() {
        return Ok(rate.borrow().inner.as_decimal());
    }
    if let Ok(bps) = obj.cast::<crate::bindings::core::types::PyBps>() {
        return Ok(bps.borrow().inner.as_decimal());
    }
    obj.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{what}: expected a decimal rate (float, e.g. 0.05 for 5%) or a Rate/Bps \
             instance, got {}",
            obj.get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ))
    })
}

/// Extract a basis-point spread from a `float | int | Bps | Rate`.
///
/// The one basis-point coercer of the bindings. A `Bps` wrapper contributes
/// its integer basis points as `f64`; a bare number is taken as already in
/// basis points (`25.0` for 25 bp). A `Rate` reports its fractional basis
/// points through the Rust `as_bp_f64` accessor, so a caller holding a decimal
/// rate never has to rescale by hand.
///
/// # Arguments
///
/// * `obj` - A Python `float`/`int` in basis points, a
///   `finstack_quant.core.types.Bps` or a `finstack_quant.core.types.Rate`.
/// * `what` - Parameter name used in the `TypeError` message.
///
/// # Errors
///
/// Returns `TypeError` when `obj` is neither numeric nor a `Bps`/`Rate`.
pub fn extract_basis_points(obj: &Bound<'_, PyAny>, what: &str) -> PyResult<f64> {
    if let Ok(bps) = obj.cast::<crate::bindings::core::types::PyBps>() {
        return Ok(f64::from(bps.borrow().inner.as_bp()));
    }
    if let Ok(rate) = obj.cast::<crate::bindings::core::types::PyRate>() {
        return rate
            .borrow()
            .inner
            .as_bp()
            .map_err(crate::errors::core_to_py);
    }
    obj.extract::<f64>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{what}: expected a spread in basis points (float, e.g. 25.0) or a Bps/Rate \
             instance, got {}",
            obj.get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ))
    })
}

/// Parse a canonical serde wire label (e.g. `"senior"`, `"low"`) into `T`.
///
/// # Errors
///
/// Returns `ValueError` when `label` is not one of `T`'s serde labels.
pub(crate) fn parse_label<T: serde::de::DeserializeOwned>(label: &str) -> PyResult<T> {
    finstack_quant_core::wire::serde_parse(label).map_err(crate::errors::core_to_py)
}

/// Render `value` as its canonical serde wire label.
///
/// # Arguments
///
/// * `value` - Rust value whose canonical serde representation must be a string.
///
/// # Errors
///
/// Returns `RuntimeError` when `value` does not serialize to a string label.
pub(crate) fn label<T: serde::Serialize>(value: &T) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(value).map_err(crate::errors::core_to_py)
}

// Records — JSON string | dict | list | pandas.DataFrame → JSON string

/// Turn a JSON-shaped Python input into its compact JSON string.
///
/// Accepts a pre-serialized JSON `str` (passed through unchanged), any
/// `json.dumps`-able object (`dict`, `list`, tuples of scalars), typed
/// result/spec wrappers standalone or nested in lists and dicts (encoded
/// through their `to_dict` / `to_json`), or a `pandas.DataFrame`, which is
/// converted through `to_dict("records")` so each row becomes one JSON object.
///
/// # Errors
///
/// Returns `ValueError` when the object cannot be JSON-encoded.
pub fn extract_records_json(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<String> {
    if let Ok(json) = obj.extract::<String>() {
        return Ok(json);
    }
    if let Some(records) = crate::bindings::dataframe_utils::table_to_py_records(py, obj)? {
        return crate::bindings::module_utils::py_to_json_string(py, &records, label);
    }
    crate::bindings::module_utils::py_to_json_string(py, obj, label)
}
