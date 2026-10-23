use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::{
    enum_to_py_string, money_from_py, money_to_py, typed_list_from_py,
};
use crate::bindings::valuations::instruments::{PyBond, PyTermLoan};
use crate::bindings::valuations::typed_revolving_credit::PyRevolvingCredit;
use crate::errors::serde_json_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, CallExercisePolicy, DealType, InstrumentCollateral, InstrumentExerciseOverride,
    PutExercisePolicy, ReinvestmentPeriod,
};

use super::super::convert::enum_from_str;
use super::{PyPoolAsset, PyRepLine};

/// Parse an internally tagged policy/destination enum from a bare variant
/// name (``"first_call"``), a ``dict`` in the serde shape, or a JSON ``str``.
fn tagged_enum_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    tag: &str,
    label: &str,
) -> PyResult<T> {
    if let Ok(text) = value.extract::<String>() {
        let trimmed = text.trim();
        let json = if trimmed.starts_with('{') {
            trimmed.to_string()
        } else {
            serde_json::json!({ tag: trimmed }).to_string()
        };
        return serde_json::from_str(&json)
            .map_err(|err| serde_json_to_py(err, &format!("invalid {label}")));
    }
    crate::bindings::module_utils::py_to_serde(py, value, label)
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetPool",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAssetPool {
    /// Inner canonical Rust asset pool.
    pub(crate) inner: AssetPool,
}

#[pymethods]
impl PyAssetPool {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.__init__")]
    #[new]
    #[pyo3(text_signature = "(id, deal_type, currency)")]
    fn new(id: &str, deal_type: &str, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let deal_type: DealType = enum_from_str(deal_type, "deal_type")?;
        let currency = crate::bindings::core::currency::extract_currency(currency)?;
        let inner = AssetPool::new(id, deal_type, currency);
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_rep_lines")]
    #[pyo3(text_signature = "($self, rep_lines)")]
    fn with_rep_lines(&self, rep_lines: Vec<PyRef<'_, PyRepLine>>) -> Self {
        let mut inner = self.inner.clone();
        inner.rep_lines = Some(rep_lines.iter().map(|line| line.inner.clone()).collect());
        Self { inner }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_assets")]
    #[pyo3(text_signature = "($self, value)")]
    fn with_assets(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let assets = typed_list_from_py(py, value, "assets", |asset: &PyPoolAsset| {
            asset.inner.clone()
        })?;
        let mut inner = self.inner.clone();
        inner.assets = assets;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_instruments")]
    #[pyo3(signature = (bonds=None, term_loans=None, revolvers=None, call_exercise=None, put_exercise=None, overrides=None))]
    #[pyo3(
        text_signature = "($self, bonds=None, term_loans=None, revolvers=None, call_exercise=None, put_exercise=None, overrides=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn with_instruments(
        &self,
        py: Python<'_>,
        bonds: Option<Vec<PyRef<'_, PyBond>>>,
        term_loans: Option<Vec<PyRef<'_, PyTermLoan>>>,
        revolvers: Option<Vec<PyRef<'_, PyRevolvingCredit>>>,
        call_exercise: Option<&Bound<'_, PyAny>>,
        put_exercise: Option<&Bound<'_, PyAny>>,
        overrides: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let call_exercise: CallExercisePolicy = match call_exercise {
            Some(value) => tagged_enum_from_py(py, value, "policy", "call_exercise")?,
            None => CallExercisePolicy::default(),
        };
        let put_exercise: PutExercisePolicy = match put_exercise {
            Some(value) => tagged_enum_from_py(py, value, "policy", "put_exercise")?,
            None => PutExercisePolicy::default(),
        };
        let overrides: Vec<InstrumentExerciseOverride> = match overrides {
            Some(value) => crate::bindings::module_utils::py_to_serde(py, value, "overrides")?,
            None => Vec::new(),
        };
        let collateral = InstrumentCollateral {
            bonds: bonds
                .unwrap_or_default()
                .iter()
                .map(|bond| bond.inner.clone())
                .collect(),
            term_loans: term_loans
                .unwrap_or_default()
                .iter()
                .map(|loan| loan.inner.clone())
                .collect(),
            revolvers: revolvers
                .unwrap_or_default()
                .iter()
                .map(|facility| facility.inner.clone())
                .collect(),
            call_exercise,
            put_exercise,
            overrides,
        };
        let mut inner = self.inner.clone();
        inner.instruments = Some(collateral);
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_reserve")]
    #[pyo3(signature = (reserve_account, reserve_account_rate=0.0, reserve_target=None, reserve_interest_destination=None, currency=None))]
    #[pyo3(
        text_signature = "($self, reserve_account, reserve_account_rate=0.0, reserve_target=None, reserve_interest_destination=None, currency=None)"
    )]
    fn with_reserve(
        &self,
        py: Python<'_>,
        reserve_account: &Bound<'_, PyAny>,
        reserve_account_rate: f64,
        reserve_target: Option<&Bound<'_, PyAny>>,
        reserve_interest_destination: Option<&Bound<'_, PyAny>>,
        currency: Option<&str>,
    ) -> PyResult<Self> {
        let inner = self.inner.clone().with_reserve(
            money_from_py(reserve_account, currency, "reserve_account")?,
            reserve_account_rate,
            reserve_target
                .map(|value| money_from_py(value, currency, "reserve_target"))
                .transpose()?,
            reserve_interest_destination
                .map(|value| tagged_enum_from_py(py, value, "kind", "reserve_interest_destination"))
                .transpose()?,
        );
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_reinvestment_period")]
    #[pyo3(text_signature = "($self, value)")]
    fn with_reinvestment_period(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let period: ReinvestmentPeriod = if let Ok(text) = value.extract::<String>() {
            serde_json::from_str(&text)
                .map_err(|err| serde_json_to_py(err, "invalid reinvestment_period"))?
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "reinvestment_period")?
        };
        let mut inner = self.inner.clone();
        inner.reinvestment_period = Some(period);
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.with_accounts")]
    #[pyo3(signature = (*, cumulative_defaults=None, cumulative_recoveries=None, cumulative_prepayments=None, cumulative_scheduled_amortization=None, collection_account=None, excess_spread_account=None, original_balance=None))]
    #[pyo3(
        text_signature = "($self, *, cumulative_defaults=None, cumulative_recoveries=None, cumulative_prepayments=None, cumulative_scheduled_amortization=None, collection_account=None, excess_spread_account=None, original_balance=None)"
    )]
    // PyO3 binding: one keyword per pool account field.
    #[allow(clippy::too_many_arguments)]
    fn with_accounts(
        &self,
        cumulative_defaults: Option<PyRef<'_, PyMoney>>,
        cumulative_recoveries: Option<PyRef<'_, PyMoney>>,
        cumulative_prepayments: Option<PyRef<'_, PyMoney>>,
        cumulative_scheduled_amortization: Option<PyRef<'_, PyMoney>>,
        collection_account: Option<PyRef<'_, PyMoney>>,
        excess_spread_account: Option<PyRef<'_, PyMoney>>,
        original_balance: Option<PyRef<'_, PyMoney>>,
    ) -> Self {
        let mut inner = self.inner.clone();
        if let Some(value) = original_balance {
            inner.original_balance = Some(value.inner);
        }
        if let Some(value) = cumulative_defaults {
            inner.cumulative_defaults = value.inner;
        }
        if let Some(value) = cumulative_recoveries {
            inner.cumulative_recoveries = value.inner;
        }
        if let Some(value) = cumulative_prepayments {
            inner.cumulative_prepayments = value.inner;
        }
        if let Some(value) = cumulative_scheduled_amortization {
            inner.cumulative_scheduled_amortization = value.inner;
        }
        if let Some(value) = collection_account {
            inner.collection_account = value.inner;
        }
        if let Some(value) = excess_spread_account {
            inner.excess_spread_account = value.inner;
        }
        Self { inner }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid AssetPool JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.asset_records")]
    #[getter]
    fn asset_records<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.assets)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.assets")]
    #[getter]
    fn assets(&self) -> Vec<PyPoolAsset> {
        self.inner
            .assets
            .iter()
            .map(|asset| PyPoolAsset {
                inner: asset.clone(),
            })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.rep_lines")]
    #[getter]
    fn rep_lines(&self) -> Option<Vec<PyRepLine>> {
        self.inner.rep_lines.as_ref().map(|lines| {
            lines
                .iter()
                .map(|line| PyRepLine {
                    inner: line.clone(),
                })
                .collect()
        })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.reserve_interest_destination")]
    #[getter]
    fn reserve_interest_destination<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.reserve_interest_destination)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.instruments")]
    #[getter]
    fn instruments<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .instruments
            .as_ref()
            .map(|collateral| crate::bindings::pandas_utils::serde_to_py(py, collateral))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.reinvestment_period")]
    #[getter]
    fn reinvestment_period<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .reinvestment_period
            .as_ref()
            .map(|period| crate::bindings::pandas_utils::serde_to_py(py, period))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.total_balance")]
    #[pyo3(text_signature = "($self)")]
    fn total_balance(&self) -> PyResult<PyMoney> {
        self.inner
            .total_balance()
            .map(money_to_py)
            .map_err(crate::errors::core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.performing_balance")]
    #[pyo3(text_signature = "($self)")]
    fn performing_balance(&self) -> PyResult<PyMoney> {
        self.inner
            .performing_balance()
            .map(money_to_py)
            .map_err(crate::errors::core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.wac")]
    #[pyo3(text_signature = "($self)")]
    fn wac(&self) -> f64 {
        self.inner.wac()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.weighted_avg_spread_bp")]
    #[pyo3(text_signature = "($self)")]
    fn weighted_avg_spread_bp(&self) -> f64 {
        self.inner.weighted_avg_spread_bp()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.weighted_avg_maturity")]
    #[pyo3(text_signature = "($self, as_of)")]
    fn weighted_avg_maturity(&self, as_of: &Bound<'_, PyAny>) -> PyResult<f64> {
        let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
        self.inner
            .weighted_avg_maturity(as_of)
            .map_err(crate::errors::core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.diversity_score")]
    #[pyo3(text_signature = "($self)")]
    fn diversity_score(&self) -> f64 {
        self.inner.diversity_score()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.assets_by_obligor")]
    #[pyo3(text_signature = "($self, obligor_id)")]
    fn assets_by_obligor(&self, obligor_id: &str) -> Vec<PyPoolAsset> {
        self.inner
            .assets_by_obligor(obligor_id)
            .into_iter()
            .map(|asset| PyPoolAsset {
                inner: asset.clone(),
            })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.assets_by_industry")]
    #[pyo3(text_signature = "($self, industry)")]
    fn assets_by_industry(&self, industry: &str) -> Vec<PyPoolAsset> {
        self.inner
            .assets_by_industry(industry)
            .into_iter()
            .map(|asset| PyPoolAsset {
                inner: asset.clone(),
            })
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetPool(id='{}', deal_type='{}', currency='{}', assets={}, rep_lines={}, instruments={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.deal_type).unwrap_or_default(),
            self.inner.currency,
            self.inner.assets.len(),
            self.inner
                .rep_lines
                .as_ref()
                .map(|lines| lines.len().to_string())
                .unwrap_or_else(|| "None".to_string()),
            self.inner
                .instruments
                .as_ref()
                .map(|collateral| collateral.len().to_string())
                .unwrap_or_else(|| "None".to_string()),
        )
    }
}

crate::bindings::macros::py_getters!(PyAssetPool |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.id")]
    id -> String = s.inner.id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.deal_type")]
    deal_type -> PyResult<String> = enum_to_py_string(&s.inner.deal_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.cumulative_defaults")]
    cumulative_defaults -> PyMoney = money_to_py(s.inner.cumulative_defaults);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.cumulative_recoveries")]
    cumulative_recoveries -> PyMoney = money_to_py(s.inner.cumulative_recoveries);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.cumulative_prepayments")]
    cumulative_prepayments -> PyMoney = money_to_py(s.inner.cumulative_prepayments);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.cumulative_scheduled_amortization")]
    cumulative_scheduled_amortization -> PyMoney =
        money_to_py(s.inner.cumulative_scheduled_amortization);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.original_balance")]
    original_balance -> Option<PyMoney> = s.inner.original_balance.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.collection_account")]
    collection_account -> PyMoney = money_to_py(s.inner.collection_account);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.reserve_account")]
    reserve_account -> PyMoney = money_to_py(s.inner.reserve_account);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.reserve_account_rate")]
    reserve_account_rate -> f64 = s.inner.reserve_account_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.reserve_target")]
    reserve_target -> Option<PyMoney> = s.inner.reserve_target.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "AssetPool.excess_spread_account")]
    excess_spread_account -> PyMoney = money_to_py(s.inner.excess_spread_account);
});
