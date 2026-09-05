//! CDS index Python wrappers: `CDSIndex`, its fluent builder, the
//! `CDSIndexParams` preset descriptor and the `CDSIndexConstituent` row.

use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndexConstituent, CDSIndexParams, IndexPricing,
};
use finstack_quant_valuations::instruments::{CreditParams, Instrument, InstrumentJson};
use rust_decimal::prelude::ToPrimitive;

use super::super::convert::{
    attributes_from_py, bool_repr, bps_from_py, builder_repr, date_repr, enum_to_py_string,
    float_repr, money_from_py, money_repr, money_to_py, opt_repr,
};
use super::super::instruments::{enum_from_str, serialize_typed_instrument_json};
use super::super::typed_fx::{
    instrument_envelope_methods, instrument_pricing_methods, take_builder,
};
use super::super::typed_legs::{PyPremiumLegSpec, PyProtectionLegSpec};
use super::cds::cds_convention_from_str;

type CdsIndexBuilderInner =
    finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndexBuilder;

/// Preset descriptor for a standardized CDS index (typed wrapper for Rust ``CDSIndexParams``).
///
/// Captures only the index identity (name, series, version), the running
/// coupon and the regional convention. Trade state — notional, side, dates,
/// curves — lives on the ``CDSIndex`` built with ``CDSIndex.from_preset``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CDSIndexParams
/// >>> preset = CDSIndexParams.cdx_na_ig(42, 1, 100.0)
/// >>> (preset.index_name, preset.convention, preset.num_constituents)
/// ('CDX.NA.IG', 'isda_na', 125)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSIndexParams",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCDSIndexParams {
    /// Inner canonical Rust preset.
    pub(crate) inner: CDSIndexParams,
}

#[pymethods]
impl PyCDSIndexParams {
    /// Describe a standardized CDS index.
    ///
    /// Parameters
    /// ----------
    /// index_name : str
    ///     Index name, e.g. ``"CDX.NA.IG"`` or ``"iTraxx Europe"``.
    /// series : int
    ///     Series number (e.g. ``42``).
    /// version : int
    ///     Version within the series (e.g. ``1``).
    /// fixed_coupon_bp : float | Bps
    ///     Fixed running coupon in basis points (``100.0`` = 1%).
    /// convention : {"isda_na", "isda_eu", "isda_as", "custom"}
    ///     Regional ISDA convention; default ``"isda_na"`` (SNAC standard).
    /// num_constituents : int | None
    ///     Number of names in the pool, used by portfolio analytics when the
    ///     constituent list is empty.
    ///
    /// Returns
    /// -------
    /// CDSIndexParams
    ///     The preset.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``convention`` is not an accepted string.
    /// TypeError
    ///     If ``fixed_coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexParams
    /// >>> CDSIndexParams("CDX.NA.HY", 42, 1, 500.0).fixed_coupon_bp
    /// 500.0
    #[new]
    #[pyo3(signature = (index_name, series, version, fixed_coupon_bp, convention="isda_na", num_constituents=None))]
    #[pyo3(
        text_signature = "(index_name, series, version, fixed_coupon_bp, convention='isda_na', num_constituents=None)"
    )]
    fn new(
        index_name: &str,
        series: u16,
        version: u16,
        fixed_coupon_bp: &Bound<'_, PyAny>,
        convention: &str,
        num_constituents: Option<u32>,
    ) -> PyResult<Self> {
        let mut inner = CDSIndexParams::new(
            index_name,
            series,
            version,
            bps_from_py(fixed_coupon_bp, "fixed_coupon_bp")?,
            cds_convention_from_str(convention)?,
        );
        if let Some(n) = num_constituents {
            inner = inner.with_num_constituents(n);
        }
        Ok(Self { inner })
    }

    /// CDX North American Investment Grade preset (125 names, ``isda_na``).
    ///
    /// Parameters
    /// ----------
    /// series : int
    ///     Series number.
    /// version : int
    ///     Version within the series.
    /// fixed_coupon_bp : float | Bps
    ///     Fixed running coupon in basis points (``100.0`` for CDX.NA.IG).
    ///
    /// Returns
    /// -------
    /// CDSIndexParams
    ///     The preset.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``fixed_coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexParams
    /// >>> CDSIndexParams.cdx_na_ig(42, 1, 100.0).num_constituents
    /// 125
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, fixed_coupon_bp)")]
    fn cdx_na_ig(series: u16, version: u16, fixed_coupon_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CDSIndexParams::cdx_na_ig(
                series,
                version,
                bps_from_py(fixed_coupon_bp, "fixed_coupon_bp")?,
            ),
        })
    }

    /// CDX North American High Yield preset (100 names, ``isda_na``).
    ///
    /// Parameters
    /// ----------
    /// series : int
    ///     Series number.
    /// version : int
    ///     Version within the series.
    /// fixed_coupon_bp : float | Bps
    ///     Fixed running coupon in basis points (``500.0`` for CDX.NA.HY).
    ///
    /// Returns
    /// -------
    /// CDSIndexParams
    ///     The preset.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``fixed_coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexParams
    /// >>> CDSIndexParams.cdx_na_hy(42, 1, 500.0).num_constituents
    /// 100
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, fixed_coupon_bp)")]
    fn cdx_na_hy(series: u16, version: u16, fixed_coupon_bp: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CDSIndexParams::cdx_na_hy(
                series,
                version,
                bps_from_py(fixed_coupon_bp, "fixed_coupon_bp")?,
            ),
        })
    }

    /// iTraxx Europe Main preset (125 names, ``isda_eu``).
    ///
    /// Parameters
    /// ----------
    /// series : int
    ///     Series number.
    /// version : int
    ///     Version within the series.
    /// fixed_coupon_bp : float | Bps
    ///     Fixed running coupon in basis points (``100.0`` for iTraxx Europe).
    ///
    /// Returns
    /// -------
    /// CDSIndexParams
    ///     The preset.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``fixed_coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexParams
    /// >>> CDSIndexParams.itraxx_europe(41, 1, 100.0).convention
    /// 'isda_eu'
    #[staticmethod]
    #[pyo3(text_signature = "(series, version, fixed_coupon_bp)")]
    fn itraxx_europe(
        series: u16,
        version: u16,
        fixed_coupon_bp: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CDSIndexParams::itraxx_europe(
                series,
                version,
                bps_from_py(fixed_coupon_bp, "fixed_coupon_bp")?,
            ),
        })
    }

    /// Index name.
    #[getter]
    fn index_name(&self) -> String {
        self.inner.index_name.clone()
    }

    /// Series number.
    #[getter]
    fn series(&self) -> u16 {
        self.inner.series
    }

    /// Version within the series.
    #[getter]
    fn version(&self) -> u16 {
        self.inner.version
    }

    /// Fixed running coupon in basis points.
    #[getter]
    fn fixed_coupon_bp(&self) -> f64 {
        self.inner.fixed_coupon_bp
    }

    /// Regional ISDA convention (serde name).
    #[getter]
    fn convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.convention)
    }

    /// Number of names in the pool, if known.
    #[getter]
    fn num_constituents(&self) -> Option<u32> {
        self.inner.num_constituents
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CDSIndexParams(index_name={:?}, series={}, version={}, fixed_coupon_bp={}, convention={:?}, num_constituents={})",
            self.inner.index_name,
            self.inner.series,
            self.inner.version,
            float_repr(self.inner.fixed_coupon_bp),
            enum_to_py_string(&self.inner.convention).unwrap_or_default(),
            opt_repr(self.inner.num_constituents),
        )
    }
}

/// One reference entity in a CDS index (typed wrapper for Rust ``CDSIndexConstituent``).
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
/// >>> row = CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 1 / 125)
/// >>> (row.reference_entity, row.defaulted)
/// ('ACME-CORP', False)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSIndexConstituent",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCDSIndexConstituent {
    /// Inner canonical Rust constituent.
    pub(crate) inner: CDSIndexConstituent,
}

#[pymethods]
impl PyCDSIndexConstituent {
    /// Describe one index constituent.
    ///
    /// Parameters
    /// ----------
    /// reference_entity : str
    ///     Issuer / reference-entity name.
    /// recovery_rate : float
    ///     Assumed recovery as a fraction (``0.4`` = 40%).
    /// credit_curve_id : str
    ///     Hazard curve identifier for the issuer.
    /// weight : float
    ///     Weight of the issuer in the index notional (``1/125`` for CDX IG).
    /// defaulted : bool
    ///     Whether the name has defaulted; defaulted names drop out of the
    ///     premium leg (their settlement is reflected in ``index_factor``).
    ///
    /// Returns
    /// -------
    /// CDSIndexConstituent
    ///     The constituent row.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
    /// >>> CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008).weight
    /// 0.008
    #[new]
    #[pyo3(signature = (reference_entity, recovery_rate, credit_curve_id, weight, defaulted=false))]
    #[pyo3(
        text_signature = "(reference_entity, recovery_rate, credit_curve_id, weight, defaulted=False)"
    )]
    fn new(
        reference_entity: &str,
        recovery_rate: f64,
        credit_curve_id: &str,
        weight: f64,
        defaulted: bool,
    ) -> Self {
        Self {
            inner: CDSIndexConstituent {
                credit: CreditParams::new(
                    reference_entity,
                    recovery_rate,
                    CurveId::new(credit_curve_id.to_string()),
                ),
                weight,
                defaulted,
            },
        }
    }

    /// Deserialize from the canonical JSON shape.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with ``credit`` (``reference_entity``, ``recovery_rate``,
    ///     ``credit_curve_id``), ``weight`` and optional ``defaulted``.
    ///
    /// Returns
    /// -------
    /// CDSIndexConstituent
    ///     The parsed constituent.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import CDSIndexConstituent
    /// >>> row = CDSIndexConstituent("ACME-CORP", 0.4, "ACME-HZD", 0.008)
    /// >>> CDSIndexConstituent.from_json(row.to_json()).credit_curve_id
    /// 'ACME-HZD'
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| serde_json_to_py(e, "invalid CDSIndexConstituent JSON"))
    }

    /// Serialize to the canonical JSON shape.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON accepted by ``from_json`` and by ``CDSIndexBuilder.constituents``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CDSIndexConstituent"))
    }

    /// Support `pickle` via the JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Issuer / reference-entity name.
    #[getter]
    fn reference_entity(&self) -> String {
        self.inner.credit.reference_entity.clone()
    }

    /// Assumed recovery as a fraction.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.credit.recovery_rate
    }

    /// Hazard curve identifier for the issuer.
    #[getter]
    fn credit_curve_id(&self) -> String {
        self.inner.credit.credit_curve_id.to_string()
    }

    /// Weight of the issuer in the index notional.
    #[getter]
    fn weight(&self) -> f64 {
        self.inner.weight
    }

    /// Whether the name has defaulted.
    #[getter]
    fn defaulted(&self) -> bool {
        self.inner.defaulted
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CDSIndexConstituent(reference_entity={:?}, recovery_rate={}, credit_curve_id={:?}, weight={}, defaulted={})",
            self.inner.credit.reference_entity,
            float_repr(self.inner.credit.recovery_rate),
            self.inner.credit.credit_curve_id.as_str(),
            float_repr(self.inner.weight),
            bool_repr(self.inner.defaulted),
        )
    }
}

/// Coerce `list[CDSIndexConstituent | dict] | str` to Rust constituents.
fn constituents_from_py(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<CDSIndexConstituent>> {
    if let Ok(json) = value.extract::<std::borrow::Cow<'_, str>>() {
        return serde_json::from_str(&json)
            .map_err(|e| serde_json_to_py(e, "invalid constituents JSON"));
    }
    let Ok(items) = value.cast::<PyList>() else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "constituents: expected a list of CDSIndexConstituent / dict or a JSON string, got {}",
            value.get_type().name()?
        )));
    };
    items
        .iter()
        .map(|item| {
            if let Ok(row) = item.cast::<PyCDSIndexConstituent>() {
                Ok(row.borrow().inner.clone())
            } else {
                crate::bindings::module_utils::py_to_serde(py, &item, "constituents entry")
            }
        })
        .collect()
}

/// Credit index (CDX / iTraxx) trade (typed wrapper for Rust ``CDSIndex``).
///
/// Priced either against a single index hazard curve (``pricing="single_curve"``,
/// a synthetic CDS) or by expanding into weighted constituents
/// (``pricing="constituents"``). ``index_factor`` scales the surviving
/// notional after defaults.
///
/// Build with ``CDSIndex.builder()`` or ``CDSIndex.from_preset(...)``; start
/// from ``CDSIndex.example()``. Instances are accepted directly by
/// ``price_instrument`` and expose ``price`` / ``metric`` / ``par_spread`` /
/// ``risky_pv01`` / ``cs01`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CDSIndex
/// >>> idx = CDSIndex.example()
/// >>> (idx.index_name, idx.series, idx.pricing, idx.num_constituents)
/// ('CDX.NA.IG', 42, 'single_curve', 125)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSIndex",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCDSIndex {
    /// Inner canonical Rust CDS index.
    pub(crate) inner: finstack_quant_valuations::instruments::CDSIndex,
}

impl PyCDSIndex {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(InstrumentJson::CDSIndex(self.inner.clone()), "CDSIndex")
    }
}

instrument_envelope_methods!(
    PyCDSIndex,
    CDSIndex,
    "cds_index",
    PyCDSIndexBuilder,
    finstack_quant_valuations::instruments::CDSIndex::builder().constituents(Vec::new())
);
instrument_pricing_methods!(PyCDSIndex);

#[pymethods]
impl PyCDSIndex {
    /// Canonical example: CDX.NA.IG series 42, USD 10,000,000 payer.
    ///
    /// Mirrors Rust ``CDSIndex::example()``: 60bp running spread,
    /// ``single_curve`` pricing off ``CDX.NA.IG.HAZARD`` discounted on
    /// ``USD-OIS``, premium 2024-03-20 to 2029-12-20, 125 names.
    ///
    /// Returns
    /// -------
    /// CDSIndex
    ///     The example index trade.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> Self {
        Self {
            inner: finstack_quant_valuations::instruments::CDSIndex::example(),
        }
    }

    /// Build an index trade from a standardized preset.
    ///
    /// Mirrors Rust ``CDSIndex::from_preset``: the premium leg takes the
    /// preset's fixed coupon and regional convention (day count, frequency,
    /// business-day rule, calendar, stub), pricing is ``"single_curve"``,
    /// ``index_factor`` is ``1.0`` and the constituent list is empty.
    ///
    /// Parameters
    /// ----------
    /// preset : CDSIndexParams
    ///     Index identity, coupon and convention (e.g. ``CDSIndexParams.cdx_na_ig``).
    /// id : str
    ///     Unique instrument identifier for the trade.
    /// notional : Money
    ///     Index notional.
    /// side : {"pay", "receive"}
    ///     ``"pay"`` buys protection, ``"receive"`` sells protection.
    /// start : datetime.date | str
    ///     Premium accrual start (typically the last IMM roll).
    /// end : datetime.date | str
    ///     Scheduled maturity (an IMM date).
    /// recovery_rate : float
    ///     Assumed recovery as a fraction (``0.4`` = 40%).
    /// discount_curve_id : str
    ///     Discount curve identifier.
    /// credit_curve_id : str
    ///     Index hazard curve identifier.
    ///
    /// Returns
    /// -------
    /// CDSIndex
    ///     The index trade.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``side`` is unknown or the preset coupon is not representable.
    #[staticmethod]
    #[pyo3(
        text_signature = "(preset, id, notional, side, start, end, recovery_rate, discount_curve_id, credit_curve_id)"
    )]
    // PyO3 binding: the argument list mirrors the Rust constructor one-for-one.
    #[allow(clippy::too_many_arguments)]
    fn from_preset(
        preset: PyRef<'_, PyCDSIndexParams>,
        id: &str,
        notional: &Bound<'_, PyAny>,
        side: &str,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        recovery_rate: f64,
        discount_curve_id: &str,
        credit_curve_id: &str,
    ) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::CDSIndex::from_preset(
            &preset.inner,
            InstrumentId::new(id.to_string()),
            money_from_py(notional, None, "notional")?,
            enum_from_str(side, "side")?,
            extract_date(start)?,
            extract_date(end)?,
            recovery_rate,
            CurveId::new(discount_curve_id.to_string()),
            CurveId::new(credit_curve_id.to_string()),
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Par spread of the index in basis points.
    ///
    /// Mirrors Rust ``CDSIndex::par_spread`` (risky-annuity denominator in
    /// ``single_curve`` mode; weighted constituents otherwise).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the discount and hazard curves the index names.
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Par spread in basis points.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve is missing from ``market``.
    /// ValueError
    ///     If ``as_of`` or the market JSON is invalid.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn par_spread(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner.par_spread(&market, as_of).map_err(core_to_py)
    }

    /// Risky PV01 (risky annuity) of the index premium leg.
    ///
    /// Mirrors Rust ``CDSIndex::risky_pv01``: PV of 1bp running on the
    /// surviving notional.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the discount and hazard curves the index names.
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Risky PV01 in notional currency units per basis point.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve is missing from ``market``.
    /// ValueError
    ///     If ``as_of`` or the market JSON is invalid.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn risky_pv01(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner.risky_pv01(&market, as_of).map_err(core_to_py)
    }

    /// Credit spread sensitivity (CS01) of the index.
    ///
    /// Mirrors Rust ``CDSIndex::cs01`` using the cached recalibration
    /// provider: the hazard curve(s) are rebootstrapped after a 1bp parallel
    /// spread bump. Hazard curves built by hand without a lossless calibration
    /// recipe raise.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the discount and hazard curves the index names.
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     PV change for a +1bp spread move, in notional currency units.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve is missing from ``market``.
    /// ValueError
    ///     If the hazard curve carries no lossless calibration recipe.
    /// RuntimeError
    ///     If the recalibration fails.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn cs01(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let provider =
            finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new();
        self.inner
            .cs01(&market, as_of, &provider)
            .map_err(core_to_py)
    }

    /// Index name.
    #[getter]
    fn index_name(&self) -> String {
        self.inner.index_name.clone()
    }

    /// Series number.
    #[getter]
    fn series(&self) -> u16 {
        self.inner.series
    }

    /// Version within the series.
    #[getter]
    fn version(&self) -> u16 {
        self.inner.version
    }

    /// Index notional.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Fraction of surviving notional (``1.0`` = no defaults since inception).
    #[getter]
    fn index_factor(&self) -> f64 {
        self.inner.index_factor
    }

    /// ``"pay"`` (buy protection) or ``"receive"`` (sell protection).
    #[getter]
    fn side(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.side)
    }

    /// Regional ISDA convention (serde name).
    #[getter]
    fn convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.convention)
    }

    /// Premium leg specification.
    #[getter]
    fn premium(&self) -> PyPremiumLegSpec {
        PyPremiumLegSpec {
            inner: self.inner.premium.clone(),
        }
    }

    /// Protection leg specification.
    #[getter]
    fn protection(&self) -> PyProtectionLegSpec {
        PyProtectionLegSpec {
            inner: self.inner.protection.clone(),
        }
    }

    /// Pricing mode: ``"single_curve"`` or ``"constituents"``.
    #[getter]
    fn pricing(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.pricing)
    }

    /// Constituent rows (empty in ``single_curve`` mode).
    #[getter]
    fn constituents(&self) -> Vec<PyCDSIndexConstituent> {
        self.inner
            .constituents
            .iter()
            .map(|row| PyCDSIndexConstituent { inner: row.clone() })
            .collect()
    }

    /// Number of names in the pool, if set.
    #[getter]
    fn num_constituents(&self) -> Option<u32> {
        self.inner.num_constituents
    }

    /// OTC margin specification as a dict, or ``None`` for unmargined trades.
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .margin_spec
            .as_ref()
            .map(|spec| crate::bindings::pandas_utils::serde_to_py(py, spec))
            .transpose()
    }

    /// Premium-leg end date (the pricer's notion of expiry), or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CDSIndex(id={:?}, index_name={:?}, series={}, side={:?}, notional={}, spread_bp={}, end={}, pricing={:?})",
            self.inner.id.as_str(),
            self.inner.index_name,
            self.inner.series,
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            opt_repr(self.inner.premium.spread_bp.to_f64()),
            date_repr(self.inner.premium.end),
            enum_to_py_string(&self.inner.pricing).unwrap_or_default(),
        )
    }
}

/// Fluent builder for ``CDSIndex``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// The builder pre-seeds an empty ``constituents`` list so ``build()``
/// succeeds without calling ``constituents`` in ``"single_curve"`` mode.
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSIndexBuilder",
    skip_from_py_object
)]
pub struct PyCDSIndexBuilder {
    inner: Option<CdsIndexBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! cds_index_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyCDSIndexBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the index trade.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(slf, id, format!("{value:?}"), |b: CdsIndexBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the index name.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx Europe"``.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn index_name<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(
            slf,
            index_name,
            format!("{value:?}"),
            |b: CdsIndexBuilderInner| b.index_name(value.to_string())
        )
    }

    /// Set the series number.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Series number, e.g. ``42``.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn series<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(slf, series, value.to_string(), |b: CdsIndexBuilderInner| b
            .series(value))
    }

    /// Set the version number within the series.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Version number, e.g. ``1``.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn version<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(
            slf,
            version,
            value.to_string(),
            |b: CdsIndexBuilderInner| b.version(value)
        )
    }

    /// Set the notional amount of the index.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount of the index.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        cds_index_set!(
            slf,
            notional,
            money_repr(money),
            |b: CdsIndexBuilderInner| b.notional(money)
        )
    }

    /// Set the index factor (fraction of surviving notional).
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Index factor in ``[0.0, 1.0]``. ``1.0`` means no constituent has
    ///     defaulted since series inception.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn index_factor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(
            slf,
            index_factor,
            float_repr(value),
            |b: CdsIndexBuilderInner| b.index_factor(value)
        )
    }

    /// Set the protection buyer/seller perspective.
    ///
    /// Parameters
    /// ----------
    /// value : {"pay", "receive"}
    ///     ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell
    ///     protection (receive premium).
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized side.
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        cds_index_set!(
            slf,
            side,
            format!("{value:?}"),
            |b: CdsIndexBuilderInner| b.side(side)
        )
    }

    /// Set the ISDA regional convention.
    ///
    /// Parameters
    /// ----------
    /// value : {"isda_na", "isda_eu", "isda_as", "custom"}
    ///     ``"isda_na"`` is the SNAC / post-Big-Bang North American standard;
    ///     ``"isda_eu"`` European; ``"isda_as"`` Asian; ``"custom"`` for a
    ///     manually configured convention.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the accepted strings; the message lists them.
    #[pyo3(text_signature = "($self, value)")]
    fn convention<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let convention = cds_convention_from_str(value)?;
        cds_index_set!(
            slf,
            convention,
            format!("{value:?}"),
            |b: CdsIndexBuilderInner| b.convention(convention)
        )
    }

    /// Set the premium leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : PremiumLegSpec
    ///     Premium leg specification (coupon schedule and discounting).
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn premium<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyPremiumLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "PremiumLegSpec(spread_bp={}, start={}, end={})",
            leg.spread_bp,
            date_repr(leg.start),
            date_repr(leg.end)
        );
        cds_index_set!(slf, premium, shown, |b: CdsIndexBuilderInner| b
            .premium(leg))
    }

    /// Set the protection leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : ProtectionLegSpec
    ///     Protection leg specification (credit curve and settlement).
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn protection<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyProtectionLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "ProtectionLegSpec(credit_curve_id={:?}, recovery_rate={})",
            leg.credit_curve_id.as_str(),
            leg.recovery_rate
        );
        cds_index_set!(slf, protection, shown, |b: CdsIndexBuilderInner| b
            .protection(leg))
    }

    /// Set the pricing aggregation mode.
    ///
    /// Parameters
    /// ----------
    /// value : {"single_curve", "constituents"}
    ///     ``"single_curve"`` prices the index against a single index hazard
    ///     curve (synthetic CDS). ``"constituents"`` prices each issuer
    ///     separately and aggregates by weight; requires ``constituents``
    ///     to be set.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized pricing mode.
    #[pyo3(text_signature = "($self, value)")]
    fn pricing<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let pricing: IndexPricing = enum_from_str(value, "pricing")?;
        cds_index_set!(
            slf,
            pricing,
            format!("{value:?}"),
            |b: CdsIndexBuilderInner| b.pricing(pricing)
        )
    }

    /// Set the index constituents.
    ///
    /// Parameters
    /// ----------
    /// value : list[CDSIndexConstituent | dict] | str
    ///     Constituent rows as typed ``CDSIndexConstituent`` objects, dicts
    ///     with ``credit`` (``reference_entity``, ``recovery_rate``,
    ///     ``credit_curve_id``), ``weight`` and optional ``defaulted``, or a
    ///     JSON array of the same shape.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a dict/JSON entry does not match the constituent shape.
    /// TypeError
    ///     If ``value`` is neither a list nor a string.
    #[pyo3(text_signature = "($self, value)")]
    fn constituents<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let constituents = constituents_from_py(py, value)?;
        let shown = format!("<{} constituents>", constituents.len());
        cds_index_set!(slf, constituents, shown, |b: CdsIndexBuilderInner| b
            .constituents(constituents))
    }

    /// Set the number of reference entities in the index pool.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Number of names in the index pool, e.g. ``125`` for CDX.NA.IG.
    ///     Required for portfolio-level analytics (e.g. jump-to-default)
    ///     when ``constituents`` is empty.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn num_constituents<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        cds_index_set!(
            slf,
            num_constituents,
            value.to_string(),
            |b: CdsIndexBuilderInner| b.num_constituents(value)
        )
    }

    /// Set the OTC margin specification for VM/IM.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``OtcMarginSpec`` as a dict or JSON string (cleared: CCP +
    ///     currency; bilateral: SIMM with a credit classification).
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``OtcMarginSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::module_utils::py_to_serde(py, value, "margin_spec")?;
        let shown = value.repr()?.to_string();
        cds_index_set!(slf, margin_spec, shown, |b: CdsIndexBuilderInner| b
            .margin_spec(spec))
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute bag; a dict populates metadata, with an optional
    ///     ``"tags"`` list entry populating tags.
    ///
    /// Returns
    /// -------
    /// CDSIndexBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a dict, nor ``None``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        cds_index_set!(slf, attributes, shown, |b: CdsIndexBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated CDS index.
    ///
    /// Validation is the Rust ``CDSIndex::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// CDSIndex
    ///     The validated CDS index.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed CDS index fails validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyCDSIndex> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyCDSIndex { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("CDSIndexBuilder", &self.fields)
    }
}

/// Register the CDS-index helper classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCDSIndexParams>()?;
    m.add_class::<PyCDSIndexConstituent>()?;
    Ok(())
}
