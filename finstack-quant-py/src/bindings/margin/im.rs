//! Python bindings for direct initial-margin calculators.
//!
//! These bindings expose the explicit calculator helper methods that do not
//! require a Python representation of the Rust `Marginable` trait.

use super::calculators::{money_from_amount, PyImResult};
use super::types::{PyCollateralAssetClass, PyEligibleCollateralSchedule};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_margin as fm;
use pyo3::prelude::*;

fn parse_currency(code: &str) -> PyResult<Currency> {
    code.parse::<Currency>().map_err(display_to_py)
}

fn parse_date(year: i32, month: u8, day: u8) -> PyResult<Date> {
    let month = time::Month::try_from(month)
        .map_err(|e| crate::errors::value_error(format!("invalid month: {e}")))?;
    Date::from_calendar_date(year, month, day)
        .map_err(|e| crate::errors::value_error(format!("invalid date: {e}")))
}

fn parse_simm_version(version: &str) -> PyResult<fm::SimmVersion> {
    version
        .parse::<fm::SimmVersion>()
        .map_err(crate::errors::value_error)
}

fn parse_credit_sector(sector: &str) -> PyResult<fm::SimmCreditSector> {
    sector
        .parse::<fm::SimmCreditSector>()
        .map_err(crate::errors::value_error)
}

fn parse_risk_class(risk_class: &str) -> PyResult<fm::SimmRiskClass> {
    risk_class
        .parse::<fm::SimmRiskClass>()
        .map_err(crate::errors::value_error)
}

fn parse_schedule_asset_class(asset_class: &str) -> PyResult<fm::ScheduleAssetClass> {
    asset_class
        .parse::<fm::ScheduleAssetClass>()
        .map_err(crate::errors::value_error)
}

/// ISDA SIMM sensitivity portfolio.
#[pyclass(
    name = "SimmSensitivities",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimmSensitivities {
    inner: fm::SimmSensitivities,
}

#[pymethods]
impl PySimmSensitivities {
    /// Create an empty SIMM sensitivity container.
    #[new]
    #[pyo3(signature = (base_currency = "USD"))]
    fn new(base_currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: fm::SimmSensitivities::new(parse_currency(base_currency)?),
        })
    }

    /// Construct from a JSON serialization of `SimmSensitivities`.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = fm::SimmSensitivities::from_json(json).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        self.inner.to_json_pretty().map_err(core_to_py)
    }

    /// Add an interest-rate delta sensitivity (DV01-style currency amount).
    #[pyo3(signature = (currency, tenor, amount))]
    fn add_ir_delta(&mut self, currency: &str, tenor: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_ir_delta(parse_currency(currency)?, tenor, amount);
        Ok(())
    }

    /// Add an interest-rate vega sensitivity.
    #[pyo3(signature = (currency, tenor, amount))]
    fn add_ir_vega(&mut self, currency: &str, tenor: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_ir_vega(parse_currency(currency)?, tenor, amount);
        Ok(())
    }

    /// Add a credit delta sensitivity.
    #[pyo3(signature = (name, qualifying, tenor, amount))]
    fn add_credit_delta(&mut self, name: &str, qualifying: bool, tenor: &str, amount: f64) {
        self.inner.add_credit_delta(name, qualifying, tenor, amount);
    }

    /// Add a bucketed credit-qualifying delta sensitivity.
    #[pyo3(signature = (sector, name, tenor, amount))]
    fn add_credit_delta_bucketed(
        &mut self,
        sector: &str,
        name: &str,
        tenor: &str,
        amount: f64,
    ) -> PyResult<()> {
        self.inner
            .add_credit_delta_bucketed(parse_credit_sector(sector)?, name, tenor, amount);
        Ok(())
    }

    /// Add an equity delta sensitivity.
    #[pyo3(signature = (underlier, amount))]
    fn add_equity_delta(&mut self, underlier: &str, amount: f64) {
        self.inner.add_equity_delta(underlier, amount);
    }

    /// Add an equity vega sensitivity.
    #[pyo3(signature = (underlier, amount))]
    fn add_equity_vega(&mut self, underlier: &str, amount: f64) {
        self.inner.add_equity_vega(underlier, amount);
    }

    /// Add an FX delta sensitivity.
    #[pyo3(signature = (currency, amount))]
    fn add_fx_delta(&mut self, currency: &str, amount: f64) -> PyResult<()> {
        self.inner.add_fx_delta(parse_currency(currency)?, amount);
        Ok(())
    }

    /// Add an FX vega sensitivity for a currency pair.
    #[pyo3(signature = (ccy1, ccy2, amount))]
    fn add_fx_vega(&mut self, ccy1: &str, ccy2: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_fx_vega(parse_currency(ccy1)?, parse_currency(ccy2)?, amount);
        Ok(())
    }

    /// Add a commodity delta sensitivity bucket.
    #[pyo3(signature = (bucket, amount))]
    fn add_commodity_delta(&mut self, bucket: &str, amount: f64) {
        self.inner.add_commodity_delta(bucket, amount);
    }

    /// Add a curvature contribution for a SIMM risk class.
    #[pyo3(signature = (risk_class, amount))]
    fn add_curvature(&mut self, risk_class: &str, amount: f64) -> PyResult<()> {
        self.inner
            .add_curvature(parse_risk_class(risk_class)?, amount);
        Ok(())
    }

    /// Whether the sensitivity container has no populated buckets.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Base currency of the sensitivity set.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }
}

/// ISDA SIMM initial-margin calculator.
#[pyclass(
    name = "SimmCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimmCalculator {
    inner: fm::SimmCalculator,
}

#[pymethods]
impl PySimmCalculator {
    /// Create a SIMM calculator from the embedded margin registry.
    #[new]
    #[pyo3(signature = (version = "v2_6", mpor_days = None))]
    fn new(version: &str, mpor_days: Option<u32>) -> PyResult<Self> {
        let mut inner =
            fm::SimmCalculator::new(parse_simm_version(version)?).map_err(core_to_py)?;
        if let Some(days) = mpor_days {
            inner = inner.with_mpor(days);
        }
        Ok(Self { inner })
    }

    /// SIMM version label (`"v2_5"` or `"v2_6"`).
    #[getter]
    fn version(&self) -> &'static str {
        self.inner.version().as_str()
    }

    /// Margin period of risk in calendar days.
    #[getter]
    fn mpor_days(&self) -> u32 {
        self.inner.mpor_days()
    }

    /// Calculate SIMM from explicit sensitivities.
    #[pyo3(signature = (sensitivities, currency, year, month, day))]
    fn calculate_from_sensitivities(
        &self,
        py: Python<'_>,
        sensitivities: &PySimmSensitivities,
        currency: &str,
        year: i32,
        month: u8,
        day: u8,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = parse_date(year, month, day)?;
        let inner = py.detach(|| {
            self.inner
                .calculate_from_sensitivities_result(&sensitivities.inner, ccy, as_of)
        });
        Ok(PyImResult::from_inner(inner))
    }
}

/// BCBS-IOSCO regulatory schedule initial-margin calculator.
#[pyclass(
    name = "ScheduleImCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScheduleImCalculator {
    inner: fm::ScheduleImCalculator,
}

#[pymethods]
impl PyScheduleImCalculator {
    /// Create a schedule calculator from the embedded BCBS-IOSCO grid.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        Ok(Self {
            inner: fm::ScheduleImCalculator::bcbs_standard().map_err(core_to_py)?,
        })
    }

    /// Create a schedule calculator from a registry id.
    #[staticmethod]
    fn from_registry_id(schedule_id: &str) -> PyResult<Self> {
        Ok(Self {
            inner: fm::ScheduleImCalculator::from_registry_id(schedule_id).map_err(core_to_py)?,
        })
    }

    /// Set the default asset class used by trait-based calculations.
    fn with_asset_class(&self, asset_class: &str) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_asset_class(parse_schedule_asset_class(asset_class)?),
        })
    }

    /// Set the default maturity used by trait-based calculations.
    fn with_maturity(&self, years: f64) -> Self {
        Self {
            inner: self.inner.clone().with_maturity(years),
        }
    }

    /// Lookup the schedule IM rate for an asset class and maturity.
    fn rate(&self, asset_class: &str, maturity_years: f64) -> PyResult<f64> {
        Ok(self
            .inner
            .rate(parse_schedule_asset_class(asset_class)?, maturity_years))
    }

    /// Calculate gross schedule IM from an explicit notional amount.
    #[pyo3(signature = (notional, currency, asset_class, maturity_years, year, month, day))]
    #[allow(clippy::too_many_arguments)]
    fn calculate_for_notional(
        &self,
        notional: f64,
        currency: &str,
        asset_class: &str,
        maturity_years: f64,
        year: i32,
        month: u8,
        day: u8,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = parse_date(year, month, day)?;
        let asset_class = parse_schedule_asset_class(asset_class)?;
        Ok(PyImResult::from_inner(
            self.inner.calculate_for_notional_result(
                money_from_amount(notional, ccy)?,
                asset_class,
                maturity_years,
                as_of,
            ),
        ))
    }

    /// Calculate schedule IM for a single-asset-class netting set using NGR.
    #[pyo3(signature = (positions, currency, asset_class, maturity_years, year, month, day))]
    #[allow(clippy::too_many_arguments)]
    fn calculate_netting_set_with_ngr(
        &self,
        positions: Vec<(f64, f64)>,
        currency: &str,
        asset_class: &str,
        maturity_years: f64,
        year: i32,
        month: u8,
        day: u8,
    ) -> PyResult<Option<PyImResult>> {
        let ccy = parse_currency(currency)?;
        let as_of = parse_date(year, month, day)?;
        let asset_class = parse_schedule_asset_class(asset_class)?;
        let money_positions: Vec<_> = positions
            .into_iter()
            .map(|(mtm, notional)| {
                Ok((
                    money_from_amount(mtm, ccy)?,
                    money_from_amount(notional, ccy)?,
                ))
            })
            .collect::<PyResult<_>>()?;
        Ok(self
            .inner
            .calculate_netting_set_with_ngr_result(
                &money_positions,
                asset_class,
                maturity_years,
                as_of,
            )
            .map(PyImResult::from_inner))
    }
}

/// Haircut-based initial-margin calculator.
#[pyclass(
    name = "HaircutImCalculator",
    module = "finstack_quant.margin",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHaircutImCalculator {
    inner: fm::HaircutImCalculator,
}

#[pymethods]
impl PyHaircutImCalculator {
    /// Create a haircut calculator with the BCBS-IOSCO schedule.
    #[staticmethod]
    fn bcbs_standard() -> PyResult<Self> {
        Ok(Self {
            inner: fm::HaircutImCalculator::bcbs_standard().map_err(core_to_py)?,
        })
    }

    /// Create a haircut calculator with the US Treasuries schedule.
    #[staticmethod]
    fn us_treasuries() -> PyResult<Self> {
        Ok(Self {
            inner: fm::HaircutImCalculator::us_treasuries().map_err(core_to_py)?,
        })
    }

    /// Create a haircut calculator from an eligible-collateral schedule.
    #[staticmethod]
    fn from_schedule(schedule: &PyEligibleCollateralSchedule) -> Self {
        Self {
            inner: fm::HaircutImCalculator::new(schedule.inner.clone()),
        }
    }

    /// Return a copy configured with a default asset class.
    fn with_default_asset_class(&self, asset_class: &PyCollateralAssetClass) -> Self {
        Self {
            inner: self
                .inner
                .clone()
                .with_default_asset_class(asset_class.inner.clone()),
        }
    }

    /// Return a copy configured with a posted-collateral currency.
    fn with_posted_collateral_currency(&self, currency: &str) -> PyResult<Self> {
        Ok(Self {
            inner: self
                .inner
                .clone()
                .with_posted_collateral_currency(parse_currency(currency)?),
        })
    }

    /// Lookup the haircut for a collateral asset class.
    fn haircut_for(&self, asset_class: &PyCollateralAssetClass) -> PyResult<f64> {
        self.inner
            .haircut_for(&asset_class.inner)
            .map_err(core_to_py)
    }

    /// Calculate haircut IM from explicit collateral value and asset class.
    #[pyo3(signature = (collateral_value, currency, asset_class, currency_mismatch, year, month, day))]
    #[allow(clippy::too_many_arguments)]
    fn calculate_for_collateral(
        &self,
        collateral_value: f64,
        currency: &str,
        asset_class: &PyCollateralAssetClass,
        currency_mismatch: bool,
        year: i32,
        month: u8,
        day: u8,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = parse_date(year, month, day)?;
        Ok(PyImResult::from_inner(
            self.inner
                .calculate_for_collateral_result(
                    money_from_amount(collateral_value, ccy)?,
                    &asset_class.inner,
                    currency_mismatch,
                    as_of,
                )
                .map_err(core_to_py)?,
        ))
    }
}

/// Register direct IM calculator bindings.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySimmSensitivities>()?;
    m.add_class::<PySimmCalculator>()?;
    m.add_class::<PyScheduleImCalculator>()?;
    m.add_class::<PyHaircutImCalculator>()?;
    Ok(())
}
