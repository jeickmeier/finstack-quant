//! Typed loan-level collateral row (`PoolAsset`).

use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::module_utils::opt_spec_from_py;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{enum_to_py_string, money_to_py};
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetType, BalloonSpec, LiquidationSpec, PoolAsset, PrepaymentPenalty, SpecialServicingSpec,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset")]
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
    value.map(py_to_date).transpose()
}

crate::bindings::macros::wire_methods!(
    PyPoolAsset,
    PoolAsset,
    "PoolAsset",
    module = "finstack_quant.valuations.instruments",
    to_dict
);

#[pymethods]
impl PyPoolAsset {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.__init__")]
    #[new]
    #[pyo3(signature = (
        id, asset_type, balance, rate, maturity, *, day_count, spread_bp=None, forward_curve_id=None,
        index_floor_bp=None, rating=None, industry=None, obligor_id=None, defaulted=false,
        recovery_amount=None, default_date=None, purchase_price=None, acquisition_date=None,
        origination_date=None, smm_override=None, mdr_override=None, recovery_rate=None, commitment=None,
        contractual_payment=None, amortization_term_months=None, io_months=None,
        market_price_pct=None, delinquency_buckets=None, balloon=None,
        prepayment_penalty=None, special_servicing=None, noi=None, liquidation=None
    ))]
    #[pyo3(
        text_signature = "(id, asset_type, balance, rate, maturity, *, day_count, spread_bp=None, forward_curve_id=None, index_floor_bp=None, rating=None, industry=None, obligor_id=None, defaulted=False, recovery_amount=None, default_date=None, purchase_price=None, acquisition_date=None, origination_date=None, smm_override=None, mdr_override=None, recovery_rate=None, commitment=None, contractual_payment=None, amortization_term_months=None, io_months=None, market_price_pct=None, delinquency_buckets=None, balloon=None, prepayment_penalty=None, special_servicing=None, noi=None, liquidation=None)"
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
        day_count: PyRef<'_, PyDayCount>,
        spread_bp: Option<f64>,
        forward_curve_id: Option<String>,
        index_floor_bp: Option<f64>,
        rating: Option<&str>,
        industry: Option<String>,
        obligor_id: Option<String>,
        defaulted: bool,
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
        let rating: Option<CreditRating> = rating
            .map(|value| value.parse().map_err(crate::errors::core_to_py))
            .transpose()?;
        let balloon: Option<BalloonSpec> = opt_spec_from_py(py, balloon, "balloon")?;
        let prepayment_penalty: Option<PrepaymentPenalty> =
            opt_spec_from_py(py, prepayment_penalty, "prepayment_penalty")?;
        let special_servicing: Option<SpecialServicingSpec> =
            opt_spec_from_py(py, special_servicing, "special_servicing")?;
        let liquidation: Option<LiquidationSpec> =
            opt_spec_from_py(py, liquidation, "liquidation")?;
        let inner = PoolAsset {
            id: InstrumentId::new(id.to_string()),
            asset_type,
            balance: balance.inner,
            rate,
            spread_bp,
            forward_curve_id,
            index_floor_bp,
            maturity: py_to_date(maturity)?,
            rating,
            industry,
            obligor_id,
            defaulted,
            recovery_amount: opt_money(recovery_amount),
            default_date: opt_date(default_date)?,
            purchase_price: opt_money(purchase_price),
            acquisition_date: opt_date(acquisition_date)?,
            origination_date: opt_date(origination_date)?,
            day_count: day_count.inner,
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.fixed_rate_bond")]
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
                py_to_date(maturity)?,
                day_count.inner,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.floating_rate_loan")]
    #[staticmethod]
    #[pyo3(text_signature = "(id, balance, forward_curve_id, spread_bp, maturity, day_count)")]
    fn floating_rate_loan(
        id: &str,
        balance: PyRef<'_, PyMoney>,
        forward_curve_id: &str,
        spread_bp: f64,
        maturity: &Bound<'_, PyAny>,
        day_count: PyRef<'_, PyDayCount>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: PoolAsset::floating_rate_loan(
                id,
                balance.inner,
                forward_curve_id,
                spread_bp,
                py_to_date(maturity)?,
                day_count.inner,
            ),
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.asset_type")]
    #[getter]
    fn asset_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.asset_type)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.rating")]
    #[getter]
    fn rating(&self) -> PyResult<Option<String>> {
        self.inner
            .rating
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.default_date")]
    #[getter]
    fn default_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .default_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.acquisition_date")]
    #[getter]
    fn acquisition_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .acquisition_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.origination_date")]
    #[getter]
    fn origination_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .origination_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.balloon")]
    #[getter]
    fn balloon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .balloon
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.prepayment_penalty")]
    #[getter]
    fn prepayment_penalty<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .prepayment_penalty
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.special_servicing")]
    #[getter]
    fn special_servicing<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .special_servicing
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.liquidation")]
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

crate::bindings::macros::py_getters!(PyPoolAsset |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.id")]
    id -> String = s.inner.id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.balance")]
    balance -> PyMoney = money_to_py(s.inner.balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.rate")]
    rate -> f64 = s.inner.rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.spread_bp")]
    spread_bp -> Option<f64> = s.inner.spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.forward_curve_id")]
    forward_curve_id -> Option<String> = s.inner.forward_curve_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.index_floor_bp")]
    index_floor_bp -> Option<f64> = s.inner.index_floor_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.industry")]
    industry -> Option<String> = s.inner.industry.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.obligor_id")]
    obligor_id -> Option<String> = s.inner.obligor_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.defaulted")]
    defaulted -> bool = s.inner.defaulted;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.recovery_amount")]
    recovery_amount -> Option<PyMoney> = s.inner.recovery_amount.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.purchase_price")]
    purchase_price -> Option<PyMoney> = s.inner.purchase_price.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.day_count")]
    day_count -> PyDayCount = PyDayCount { inner: s.inner.day_count };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.smm_override")]
    smm_override -> Option<f64> = s.inner.smm_override;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.mdr_override")]
    mdr_override -> Option<f64> = s.inner.mdr_override;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.recovery_rate")]
    recovery_rate -> Option<f64> = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.commitment")]
    commitment -> Option<PyMoney> = s.inner.commitment.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.contractual_payment")]
    contractual_payment -> Option<PyMoney> = s.inner.contractual_payment.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.amortization_term_months")]
    amortization_term_months -> Option<u32> = s.inner.amortization_term_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.io_months")]
    io_months -> Option<u32> = s.inner.io_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.market_price_pct")]
    market_price_pct -> Option<f64> = s.inner.market_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.delinquency_buckets")]
    delinquency_buckets -> Option<Vec<PyMoney>> = s.inner
        .delinquency_buckets
        .as_ref()
        .map(|buckets| buckets.iter().copied().map(money_to_py).collect());

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "PoolAsset.noi")]
    noi -> Option<PyMoney> = s.inner.noi.map(money_to_py);
});
