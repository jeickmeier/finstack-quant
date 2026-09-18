//! Typed loan-level collateral row (`PoolAsset`).

use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{enum_to_py_string, money_to_py};
use crate::errors::value_error;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetType, BalloonSpec, LiquidationSpec, PoolAsset, PrepaymentPenalty, SpecialServicingSpec,
};

use super::super::instruments::enum_from_str;

/// One loan-level collateral row of an [`AssetPool`](super::PyAssetPool):
/// the contractual terms (balance, coupon or index + spread, maturity, day
/// count, amortization type), the credit state (rating, default, recovery,
/// purchase price) and the behavioural overrides (SMM / MDR / recovery
/// overrides, delinquency buckets, commercial-mortgage balloon, prepayment
/// penalty, special servicing, NOI).
///
/// Percent fields (`market_price_pct`) are percent values; `rate`,
/// `smm_override`, `mdr_override` and `recovery_rate` are decimals;
/// `spread_bp` is in basis points.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import PoolAsset
/// >>> loan = PoolAsset(
/// ...     "LOAN-1",
/// ...     {"type": "first_lien_loan", "industry": "Software"},
/// ...     Money(10_000_000.0, Currency("USD")),
/// ...     0.08,
/// ...     datetime.date(2031, 1, 15),
/// ...     credit_quality="B",
/// ...     balloon={"extension_prob": 0.3, "extension_months": 24},
/// ... )
/// >>> loan.credit_quality, loan.balloon["extension_months"]
/// ('B', 24)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PoolAsset",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPoolAsset {
    /// Inner canonical Rust asset row.
    pub(crate) inner: PoolAsset,
}

/// Convert an optional Python `Money` reference to its Rust value.
fn opt_money(value: Option<PyRef<'_, PyMoney>>) -> Option<finstack_quant_core::money::Money> {
    value.map(|m| m.inner)
}

/// Convert an optional Python date to a Rust `Date`.
fn opt_date(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_core::dates::Date>> {
    value.map(extract_date).transpose()
}

/// Deserialize an optional dict / JSON string sub-spec.
fn opt_spec<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    label: &str,
) -> PyResult<Option<T>> {
    value
        .map(|obj| crate::bindings::module_utils::py_to_serde(py, obj, label))
        .transpose()
}

sc_wire_methods!(PyPoolAsset, PoolAsset, "PoolAsset");

#[pymethods]
impl PyPoolAsset {
    /// Construct a collateral row from its contractual terms.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable asset identifier, unique within the pool.
    /// asset_type : dict | str
    ///     ``AssetType`` serde object such as ``{"type": "first_lien_loan",
    ///     "industry": None}`` or ``{"type": "high_yield_bond"}``; the type decides
    ///     whether the row amortizes (level pay) or pays as a bullet.
    /// balance : Money
    ///     Current principal balance in the pool currency.
    /// rate : float
    ///     Annual coupon as a decimal (``0.08`` = 8%). For floating rows the
    ///     engine adds ``spread_bp`` to the ``index_id`` projection.
    /// maturity : datetime.date
    ///     Contractual maturity (balloon date for commercial mortgages).
    /// day_count : DayCount, optional
    ///     Accrual convention; Act/360 when omitted.
    /// spread_bp : float, optional
    ///     Floating spread over ``index_id`` in basis points.
    /// index_id : str, optional
    ///     Forward-curve identifier of the floating index; ``None`` for a
    ///     fixed-rate row.
    /// index_floor : float, optional
    ///     Floor on the floating index as an annual decimal (``0.01`` = 1%),
    ///     applied before ``spread_bp`` is added; ignored on fixed-rate rows.
    /// credit_quality : str, optional
    ///     Credit rating (``"BB"``, ``"CCC"``, ``"NR"`` ...), used by the
    ///     coverage-test haircuts and the CCC bucket.
    /// industry : str, optional
    ///     Industry label for concentration reporting.
    /// obligor_id : str, optional
    ///     Obligor identifier for exposure aggregation.
    /// is_defaulted : bool, optional
    ///     ``True`` marks the row defaulted at closing; ``recovery_amount``
    ///     and ``default_date`` describe its state.
    /// recovery_amount : Money, optional
    ///     Recovery still expected on a defaulted row.
    /// default_date : datetime.date, optional
    ///     Date of default for a defaulted row.
    /// purchase_price : Money, optional
    ///     Price paid for the row (discount-obligation test).
    /// acquisition_date : datetime.date, optional
    ///     Date the row entered the pool (anchors non-performing-loan
    ///     timelines).
    /// origination_date : datetime.date, optional
    ///     Date the loan was originated; anchors the seasoning-dependent
    ///     prepayment/default curves (PSA, SDA, ABS, vector) and the
    ///     amortization schedule. Falls back to ``acquisition_date``, then to
    ///     the deal closing date.
    /// smm_override : float, optional
    ///     Row-level single-month mortality (decimal) overriding the deal
    ///     prepayment model.
    /// mdr_override : float, optional
    ///     Row-level monthly default rate (decimal) overriding the deal
    ///     default model.
    /// recovery_rate : float, optional
    ///     Row-level recovery rate (decimal) overriding the deal recovery
    ///     model.
    /// commitment : Money, optional
    ///     Total commitment for revolving rows (drawn balance is ``balance``).
    /// contractual_payment : Money, optional
    ///     Monthly level payment of an amortizing row; derived from the
    ///     terms when omitted.
    /// amortization_term_months : int, optional
    ///     Schedule length in months from origination (``origination_date``,
    ///     else ``acquisition_date``, else closing) for level-pay rows; the unamortized balance pays as
    ///     the balloon at maturity. ``None`` amortizes fully by maturity.
    /// io_months : int, optional
    ///     Interest-only window in months from origination: no scheduled
    ///     principal while the loan is younger than this.
    /// market_price_pct : float, optional
    ///     Market price in percent of par for market-value coverage rules.
    /// delinquency_buckets : list[Money], optional
    ///     Seeded delinquent balances per bucket (30/60/90 ...); requires
    ///     a ``credit_model.delinquency`` model on the deal.
    /// balloon : dict | str, optional
    ///     ``BalloonSpec`` (``extension_prob`` decimal, ``extension_months``,
    ///     optional ``extension_rate`` decimal, and ``loss_prob`` decimal,
    ///     ``severity_pct`` percent, ``workout_months`` for the share that
    ///     defaults at maturity) for balloon extension and workout.
    /// prepayment_penalty : dict | str, optional
    ///     ``PrepaymentPenalty``: ``{"kind": "lockout", "through":
    ///     "2025-12-31"}`` (no voluntary prepayment inside the window),
    ///     ``{"kind": "fixed", "pct": 3.0, "through": "2026-01-01"}``,
    ///     ``{"kind": "step_down", "schedule": [{"through": "2025-12-31",
    ///     "pct": 5.0}, ...]}`` or ``{"kind": "yield_maintenance",
    ///     "reinvestment_rate": 0.05, "discount_curve_id": "USD-OIS",
    ///     "floor_pct": 1.0, "through": None}`` (the curve discounts the lost
    ///     coupons and supplies the reinvestment rate when it is omitted).
    /// special_servicing : dict | str, optional
    ///     ``SpecialServicingSpec`` (``appraisal_reduction_pct`` percent).
    /// noi : Money, optional
    ///     Annual net operating income of the property for the CMBS DSCR.
    /// liquidation : dict | str, optional
    ///     ``LiquidationSpec`` for a non-performing loan
    ///     (``months_to_resolution``, ``proceeds_pct`` and ``carry_cost_pct``
    ///     percents, ``reperformance_prob`` decimal, optional
    ///     ``modified_rate`` decimal); leave ``is_defaulted`` false.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a sub-spec does not match its serde shape, a date is invalid
    ///     or ``credit_quality`` is not a known rating.
    #[new]
    #[pyo3(signature = (
        id, asset_type, balance, rate, maturity, *, day_count=None, spread_bp=None, index_id=None,
        index_floor=None, credit_quality=None, industry=None, obligor_id=None, is_defaulted=false,
        recovery_amount=None, default_date=None, purchase_price=None, acquisition_date=None,
        origination_date=None, smm_override=None, mdr_override=None, recovery_rate=None, commitment=None,
        contractual_payment=None, amortization_term_months=None, io_months=None,
        market_price_pct=None, delinquency_buckets=None, balloon=None,
        prepayment_penalty=None, special_servicing=None, noi=None, liquidation=None
    ))]
    #[pyo3(
        text_signature = "(id, asset_type, balance, rate, maturity, *, day_count=None, spread_bp=None, index_id=None, index_floor=None, credit_quality=None, industry=None, obligor_id=None, is_defaulted=False, recovery_amount=None, default_date=None, purchase_price=None, acquisition_date=None, origination_date=None, smm_override=None, mdr_override=None, recovery_rate=None, commitment=None, contractual_payment=None, amortization_term_months=None, io_months=None, market_price_pct=None, delinquency_buckets=None, balloon=None, prepayment_penalty=None, special_servicing=None, noi=None, liquidation=None)"
    )]
    // PyO3 binding: one keyword per public Rust field.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        id: &str,
        asset_type: &Bound<'_, PyAny>,
        balance: PyRef<'_, PyMoney>,
        rate: f64,
        maturity: &Bound<'_, PyAny>,
        day_count: Option<PyRef<'_, PyDayCount>>,
        spread_bp: Option<f64>,
        index_id: Option<String>,
        index_floor: Option<f64>,
        credit_quality: Option<&str>,
        industry: Option<String>,
        obligor_id: Option<String>,
        is_defaulted: bool,
        recovery_amount: Option<PyRef<'_, PyMoney>>,
        default_date: Option<&Bound<'_, PyAny>>,
        purchase_price: Option<PyRef<'_, PyMoney>>,
        acquisition_date: Option<&Bound<'_, PyAny>>,
        origination_date: Option<&Bound<'_, PyAny>>,
        smm_override: Option<f64>,
        mdr_override: Option<f64>,
        recovery_rate: Option<f64>,
        commitment: Option<PyRef<'_, PyMoney>>,
        contractual_payment: Option<PyRef<'_, PyMoney>>,
        amortization_term_months: Option<u32>,
        io_months: Option<u32>,
        market_price_pct: Option<f64>,
        delinquency_buckets: Option<Vec<PyRef<'_, PyMoney>>>,
        balloon: Option<&Bound<'_, PyAny>>,
        prepayment_penalty: Option<&Bound<'_, PyAny>>,
        special_servicing: Option<&Bound<'_, PyAny>>,
        noi: Option<PyRef<'_, PyMoney>>,
        liquidation: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let asset_type: AssetType =
            crate::bindings::module_utils::py_to_serde(py, asset_type, "asset_type")?;
        let credit_quality: Option<CreditRating> = credit_quality
            .map(|value| enum_from_str(value, "credit_quality"))
            .transpose()?;
        let balloon: Option<BalloonSpec> = opt_spec(py, balloon, "balloon")?;
        let prepayment_penalty: Option<PrepaymentPenalty> =
            opt_spec(py, prepayment_penalty, "prepayment_penalty")?;
        let special_servicing: Option<SpecialServicingSpec> =
            opt_spec(py, special_servicing, "special_servicing")?;
        let liquidation: Option<LiquidationSpec> = opt_spec(py, liquidation, "liquidation")?;
        let inner = PoolAsset {
            id: InstrumentId::new(id.to_string()),
            asset_type,
            balance: balance.inner,
            rate,
            spread_bp,
            index_id,
            index_floor,
            maturity: extract_date(maturity)?,
            credit_quality,
            industry,
            obligor_id,
            is_defaulted,
            recovery_amount: opt_money(recovery_amount),
            default_date: opt_date(default_date)?,
            purchase_price: opt_money(purchase_price),
            acquisition_date: opt_date(acquisition_date)?,
            origination_date: opt_date(origination_date)?,
            day_count: day_count.map_or(DayCount::Act360, |value| value.inner),
            smm_override,
            mdr_override,
            recovery_rate,
            commitment: opt_money(commitment),
            contractual_payment: opt_money(contractual_payment),
            amortization_term_months,
            io_months,
            market_price_pct,
            delinquency_buckets: delinquency_buckets
                .map(|buckets| buckets.into_iter().map(|m| m.inner).collect()),
            balloon,
            prepayment_penalty,
            special_servicing,
            noi: opt_money(noi),
            liquidation,
        };
        Ok(Self { inner })
    }

    /// Fixed-rate bullet bond row (mirrors Rust ``PoolAsset::fixed_rate_bond``).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable asset identifier.
    /// balance : Money
    ///     Current principal balance.
    /// rate : float
    ///     Annual fixed coupon as a decimal.
    /// maturity : datetime.date
    ///     Bullet maturity.
    /// day_count : DayCount
    ///     Accrual convention.
    ///
    /// Returns
    /// -------
    /// PoolAsset
    ///     A performing, unrated bond row.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``maturity`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import PoolAsset
    /// >>> bond = PoolAsset.fixed_rate_bond("B1", Money(1_000_000.0, Currency("USD")), 0.06, datetime.date(2030, 1, 1), DayCount.THIRTY_360)
    /// >>> bond.asset_type["type"], bond.rate
    /// ('high_yield_bond', 0.06)
    #[staticmethod]
    #[pyo3(text_signature = "(id, balance, rate, maturity, day_count)")]
    fn fixed_rate_bond(
        id: &str,
        balance: PyRef<'_, PyMoney>,
        rate: f64,
        maturity: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PoolAsset::fixed_rate_bond(
                id,
                balance.inner,
                rate,
                extract_date(maturity)?,
                day_count.inner,
            ),
        })
    }

    /// Floating-rate first-lien loan row (mirrors Rust
    /// ``PoolAsset::floating_rate_loan``).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable asset identifier.
    /// balance : Money
    ///     Current principal balance.
    /// index_id : str
    ///     Forward-curve identifier of the floating index (e.g.
    ///     ``"USD-SOFR-3M"``).
    /// spread_bp : float
    ///     Spread over the index in basis points.
    /// maturity : datetime.date
    ///     Loan maturity.
    /// day_count : DayCount
    ///     Accrual convention.
    ///
    /// Returns
    /// -------
    /// PoolAsset
    ///     A performing first-lien loan row.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``maturity`` is not a valid date.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.dates import DayCount
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import PoolAsset
    /// >>> loan = PoolAsset.floating_rate_loan("L1", Money(1_000_000.0, Currency("USD")), "USD-SOFR-3M", 350.0, datetime.date(2030, 1, 1), DayCount.ACT_360)
    /// >>> loan.index_id, loan.spread_bp
    /// ('USD-SOFR-3M', 350.0)
    #[staticmethod]
    #[pyo3(text_signature = "(id, balance, index_id, spread_bp, maturity, day_count)")]
    fn floating_rate_loan(
        id: &str,
        balance: PyRef<'_, PyMoney>,
        index_id: &str,
        spread_bp: f64,
        maturity: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PoolAsset::floating_rate_loan(
                id,
                balance.inner,
                index_id,
                spread_bp,
                extract_date(maturity)?,
                day_count.inner,
            ),
        })
    }

    /// Asset identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// ``AssetType`` serde object (``{"type": ..., ...}``).
    #[getter]
    fn asset_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.asset_type)
    }

    /// Current principal balance.
    #[getter]
    fn balance(&self) -> PyMoney {
        money_to_py(self.inner.balance)
    }

    /// Annual coupon as a decimal.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    /// Floating spread in basis points, or ``None`` for a fixed-rate row.
    #[getter]
    fn spread_bp(&self) -> Option<f64> {
        self.inner.spread_bp
    }

    /// Floating index curve identifier, or ``None``.
    #[getter]
    fn index_id(&self) -> Option<String> {
        self.inner.index_id.clone()
    }

    /// Floor on the floating index (annual decimal) applied before the
    /// spread, or ``None``.
    #[getter]
    fn index_floor(&self) -> Option<f64> {
        self.inner.index_floor
    }

    /// Contractual maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Credit rating string, or ``None`` when unrated.
    #[getter]
    fn credit_quality(&self) -> PyResult<Option<String>> {
        self.inner
            .credit_quality
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Industry label, or ``None``.
    #[getter]
    fn industry(&self) -> Option<String> {
        self.inner.industry.clone()
    }

    /// Obligor identifier, or ``None``.
    #[getter]
    fn obligor_id(&self) -> Option<String> {
        self.inner.obligor_id.clone()
    }

    /// ``True`` when the row is defaulted.
    #[getter]
    fn is_defaulted(&self) -> bool {
        self.inner.is_defaulted
    }

    /// Expected recovery on a defaulted row, or ``None``.
    #[getter]
    fn recovery_amount(&self) -> Option<PyMoney> {
        self.inner.recovery_amount.map(money_to_py)
    }

    /// Default date as ``datetime.date``, or ``None``.
    #[getter]
    fn default_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .default_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Purchase price, or ``None``.
    #[getter]
    fn purchase_price(&self) -> Option<PyMoney> {
        self.inner.purchase_price.map(money_to_py)
    }

    /// Acquisition date as ``datetime.date``, or ``None``.
    #[getter]
    fn acquisition_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .acquisition_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Origination date as ``datetime.date``, or ``None`` (the row then ages
    /// from ``acquisition_date``, else the closing date).
    #[getter]
    fn origination_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .origination_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    /// Accrual day count.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount {
            inner: self.inner.day_count,
        }
    }

    /// Row-level SMM override (decimal), or ``None``.
    #[getter]
    fn smm_override(&self) -> Option<f64> {
        self.inner.smm_override
    }

    /// Row-level MDR override (decimal), or ``None``.
    #[getter]
    fn mdr_override(&self) -> Option<f64> {
        self.inner.mdr_override
    }

    /// Row-level recovery rate (decimal), or ``None``.
    #[getter]
    fn recovery_rate(&self) -> Option<f64> {
        self.inner.recovery_rate
    }

    /// Total commitment of a revolving row, or ``None``.
    #[getter]
    fn commitment(&self) -> Option<PyMoney> {
        self.inner.commitment.map(money_to_py)
    }

    /// Monthly level payment, or ``None`` when derived from the terms.
    #[getter]
    fn contractual_payment(&self) -> Option<PyMoney> {
        self.inner.contractual_payment.map(money_to_py)
    }

    /// Amortization schedule length in months from origination, or ``None``.
    #[getter]
    fn amortization_term_months(&self) -> Option<u32> {
        self.inner.amortization_term_months
    }

    /// Interest-only window in months from origination, or ``None``.
    #[getter]
    fn io_months(&self) -> Option<u32> {
        self.inner.io_months
    }

    /// Market price in percent of par, or ``None``.
    #[getter]
    fn market_price_pct(&self) -> Option<f64> {
        self.inner.market_price_pct
    }

    /// Seeded delinquent balances per bucket, or ``None``.
    #[getter]
    fn delinquency_buckets(&self) -> Option<Vec<PyMoney>> {
        self.inner
            .delinquency_buckets
            .as_ref()
            .map(|buckets| buckets.iter().copied().map(money_to_py).collect())
    }

    /// ``BalloonSpec`` serde dict, or ``None``.
    #[getter]
    fn balloon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .balloon
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// ``PrepaymentPenalty`` serde dict, or ``None``.
    #[getter]
    fn prepayment_penalty<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .prepayment_penalty
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// ``SpecialServicingSpec`` serde dict, or ``None``.
    #[getter]
    fn special_servicing<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .special_servicing
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Annual net operating income, or ``None``.
    #[getter]
    fn noi(&self) -> Option<PyMoney> {
        self.inner.noi.map(money_to_py)
    }

    /// ``LiquidationSpec`` serde dict, or ``None`` for a performing loan.
    #[getter]
    fn liquidation<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .liquidation
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "PoolAsset(id={:?}, balance={}, rate={}, maturity={})",
            self.inner.id.as_str(),
            self.inner.balance.amount(),
            self.inner.rate,
            self.inner.maturity
        )
    }
}

/// Coerce ``Sequence[PoolAsset | dict] | str`` into Rust asset rows.
pub(crate) fn pool_assets_from_py(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<PoolAsset>> {
    if value.is_instance_of::<pyo3::types::PyString>() {
        return crate::bindings::module_utils::py_to_serde(py, value, "assets");
    }
    let items = value.try_iter().map_err(|_| {
        value_error(
            "assets: expected a sequence of PoolAsset objects or dicts, or a JSON array string",
        )
    })?;
    let mut assets = Vec::new();
    for item in items {
        let item = item?;
        if let Ok(asset) = item.cast::<PyPoolAsset>() {
            assets.push(asset.borrow().inner.clone());
        } else {
            assets.push(crate::bindings::module_utils::py_to_serde(
                py, &item, "assets",
            )?);
        }
    }
    Ok(assets)
}
