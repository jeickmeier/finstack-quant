//! Shared polymorphic extraction helpers for PyO3 bindings.
//!
//! Each helper accepts a `&Bound<'_, PyAny>` and tries two paths:
//!
//! 1. **Typed fast path** — cast to the corresponding `#[pyclass]` wrapper
//!    and use the inner Rust value directly: the `*Access` enums borrow it
//!    (no clone), and [`extract_instrument`] clones it (no JSON).
//! 2. **Canonical JSON path** — extract a Python `str` (or envelope `dict`),
//!    then deserialize the same serde contract used by Rust.
//!
//! The `*Access` enums wrap both paths behind a `Deref<Target = T>` impl so
//! pipeline functions can accept `T | str` without branching.
//! [`extract_instrument_json`] is the exception: it serializes a typed
//! instrument to envelope text, for Rust APIs that take JSON.

use pyo3::prelude::*;

use crate::bindings::portfolio::container::PyPortfolio;
use crate::bindings::portfolio::valuation::PyPortfolioValuation;
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
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
use crate::errors::{display_to_py as to_py, portfolio_to_py};
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::pricer::ParsedInstrument;

// Instrument — typed-or-JSON extraction

/// A polymorphic instrument argument: a typed instance's native payload, or
/// canonical envelope JSON text from a `dict` or `str`.
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

/// Extract an instrument argument from a typed instrument object, an envelope
/// `dict`, or a pre-serialized envelope string.
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
    envelope_json_text(obj).map(InstrumentArg::Envelope).map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a canonical instrument envelope (dict or JSON string) or a typed instrument instance",
        )
    })
}

/// Extract canonical instrument-envelope JSON text from a typed instrument
/// object, an envelope `dict`, or a pre-serialized envelope string.
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
/// A `str` is passed through unchanged so the Rust parser reports its own
/// errors; a `dict` is serialized with `json.dumps`.
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

// Zero-clone access types (available for callers that only need &T)

/// Access to a [`FinancialModelSpec`] without cloning on the typed fast path.
///
/// When the caller passes a `FinancialModelSpec` Python object, the
/// `Borrowed` variant holds a `PyRef` guard — no clone occurs.  When the
/// caller passes a JSON string, the `Owned` variant holds the parsed value.
///
/// Use `Deref` (i.e. `&model`) for read-only access.  Call `.into_owned()`
/// only when ownership is truly needed (e.g. `goal_seek` which mutates).
pub enum ModelAccess<'py> {
    Borrowed(PyRef<'py, PyFinancialModelSpec>),
    Owned(Box<finstack_quant_statements::types::FinancialModelSpec>),
}

impl std::ops::Deref for ModelAccess<'_> {
    type Target = finstack_quant_statements::types::FinancialModelSpec;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(m) => m.as_ref(),
        }
    }
}

impl ModelAccess<'_> {
    /// Consume this access and produce an owned value, cloning only if
    /// the data was borrowed from a Python object.
    pub fn into_owned(self) -> finstack_quant_statements::types::FinancialModelSpec {
        match self {
            Self::Borrowed(r) => r.inner.clone(),
            Self::Owned(m) => *m,
        }
    }
}

/// Access to a [`StatementResult`] without cloning on the typed fast path.
pub enum ResultAccess<'py> {
    Borrowed(PyRef<'py, PyStatementResult>),
    Owned(Box<finstack_quant_statements::evaluator::StatementResult>),
}

impl std::ops::Deref for ResultAccess<'_> {
    type Target = finstack_quant_statements::evaluator::StatementResult;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(r) => r.as_ref(),
        }
    }
}

impl ResultAccess<'_> {
    pub fn into_owned(self) -> finstack_quant_statements::evaluator::StatementResult {
        match self {
            Self::Borrowed(r) => r.inner.clone(),
            Self::Owned(r) => *r,
        }
    }
}

/// Extract a [`FinancialModelSpec`] without cloning when a typed Python
/// object is passed.  Returns [`ModelAccess`] which dereferences to
/// `&FinancialModelSpec`.
pub fn extract_model_ref<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ModelAccess<'py>> {
    if let Ok(spec) = obj.cast::<PyFinancialModelSpec>() {
        return Ok(ModelAccess::Borrowed(spec.borrow()));
    }
    let json: String = obj.extract()?;
    let inner = finstack_quant_statements::types::FinancialModelSpec::from_json(&json)
        .map_err(|error| crate::errors::contract_to_py(obj.py(), error))?;
    Ok(ModelAccess::Owned(Box::new(inner)))
}

/// Extract a [`StatementResult`] without cloning when a typed Python
/// object is passed.
pub fn extract_results_ref<'py>(obj: &Bound<'py, PyAny>) -> PyResult<ResultAccess<'py>> {
    if let Ok(result) = obj.cast::<PyStatementResult>() {
        return Ok(ResultAccess::Borrowed(result.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_statements::evaluator::StatementResult =
        serde_json::from_str(&json).map_err(to_py)?;
    Ok(ResultAccess::Owned(Box::new(inner)))
}

// Portfolio — borrow-preferring access

/// Access to a [`Portfolio`] without rebuilding from spec on the typed path.
///
/// Portfolio construction parses positions and rebuilds the position index +
/// dependency index; doing it once and reusing the typed object across
/// pipeline calls (value, cashflows, metrics, scenario) is a major win.
pub enum PortfolioAccess<'py> {
    Borrowed(PyRef<'py, PyPortfolio>),
    Owned(Box<finstack_quant_portfolio::Portfolio>),
}

impl std::ops::Deref for PortfolioAccess<'_> {
    type Target = finstack_quant_portfolio::Portfolio;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => r.inner.as_ref(),
            Self::Owned(p) => p.as_ref(),
        }
    }
}

/// Extract a [`Portfolio`] from a `Portfolio` Python object (fast path) or
/// build one from a JSON spec string (fallback). The JSON path pays the full
/// `Portfolio::from_spec` cost, which includes position materialization,
/// index construction, and validation; both stages release the GIL.
pub fn extract_portfolio_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<PortfolioAccess<'py>> {
    if let Ok(p) = obj.cast::<PyPortfolio>() {
        return Ok(PortfolioAccess::Borrowed(p.borrow()));
    }
    let json: String = obj.extract()?;
    let spec: finstack_quant_portfolio::PortfolioSpec = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    let portfolio = py
        .detach(move || finstack_quant_portfolio::Portfolio::from_spec(spec))
        .map_err(portfolio_to_py)?;
    Ok(PortfolioAccess::Owned(Box::new(portfolio)))
}

// PortfolioValuation — borrow-preferring access

/// Access to a [`PortfolioValuation`] without re-parsing JSON when a typed
/// Python object is passed.
pub enum ValuationAccess<'py> {
    Borrowed(PyRef<'py, PyPortfolioValuation>),
    Owned(Box<finstack_quant_portfolio::valuation::PortfolioValuation>),
}

impl std::ops::Deref for ValuationAccess<'_> {
    type Target = finstack_quant_portfolio::valuation::PortfolioValuation;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(v) => v.as_ref(),
        }
    }
}

/// Extract a [`PortfolioValuation`] from a typed Python object or a JSON string.
pub fn extract_valuation_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<ValuationAccess<'py>> {
    if let Ok(v) = obj.cast::<PyPortfolioValuation>() {
        return Ok(ValuationAccess::Borrowed(v.borrow()));
    }
    let json: String = obj.extract()?;
    let inner: finstack_quant_portfolio::valuation::PortfolioValuation = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(ValuationAccess::Owned(Box::new(inner)))
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

// PortfolioCashflows — borrow-preferring access

/// Access to a [`finstack_quant_portfolio::cashflows::PortfolioCashflows`]
/// ladder without re-parsing JSON when a typed Python object is passed.
pub enum CashflowsAccess<'py> {
    Borrowed(PyRef<'py, crate::bindings::portfolio::cashflows::PyPortfolioCashflows>),
    Owned(Box<finstack_quant_portfolio::cashflows::PortfolioCashflows>),
}

impl std::ops::Deref for CashflowsAccess<'_> {
    type Target = finstack_quant_portfolio::cashflows::PortfolioCashflows;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(r) => &r.inner,
            Self::Owned(c) => c.as_ref(),
        }
    }
}

/// Extract a `PortfolioCashflows` ladder from a typed Python object or a
/// full cashflow-ladder JSON string.
pub fn extract_cashflows_ref<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<CashflowsAccess<'py>> {
    if let Ok(c) = obj.cast::<crate::bindings::portfolio::cashflows::PyPortfolioCashflows>() {
        return Ok(CashflowsAccess::Borrowed(c.borrow()));
    }
    let json: String = obj.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected a PortfolioCashflows instance or a cashflow-ladder JSON string",
        )
    })?;
    let inner: finstack_quant_portfolio::cashflows::PortfolioCashflows = py
        .detach(move || serde_json::from_str(&json))
        .map_err(to_py)?;
    Ok(CashflowsAccess::Owned(Box::new(inner)))
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
