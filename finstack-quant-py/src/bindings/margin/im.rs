//! Python bindings for direct initial-margin calculators.
//!
//! These bindings expose the explicit calculator helper methods that do not
//! require a Python representation of the Rust `Marginable` trait.

use super::calculators::{
    imresult_from_amount, imresult_from_parts, money_from_amount, PyImResult,
};
use super::types::{PyCollateralAssetClass, PyEligibleCollateralSchedule};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_margin as fm;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SimmSensitivitiesJson {
    base_currency: Currency,
    #[serde(default)]
    ir_delta: Vec<(Currency, String, f64)>,
    #[serde(default)]
    ir_vega: Vec<(Currency, String, f64)>,
    #[serde(default)]
    credit_qualifying_delta: Vec<(String, String, f64)>,
    #[serde(default)]
    credit_non_qualifying_delta: Vec<(String, String, f64)>,
    #[serde(default)]
    equity_delta: Vec<(String, f64)>,
    #[serde(default)]
    equity_vega: Vec<(String, f64)>,
    #[serde(default)]
    fx_delta: Vec<(Currency, f64)>,
    #[serde(default)]
    fx_vega: Vec<(Currency, Currency, f64)>,
    #[serde(default)]
    commodity_delta: Vec<(String, f64)>,
    #[serde(default)]
    curvature: Vec<(fm::SimmRiskClass, f64)>,
    #[serde(default)]
    credit_qualifying_delta_bucketed: Vec<(fm::SimmCreditSector, String, String, f64)>,
}

impl From<&fm::SimmSensitivities> for SimmSensitivitiesJson {
    fn from(sens: &fm::SimmSensitivities) -> Self {
        Self {
            base_currency: sens.base_currency,
            ir_delta: sens
                .ir_delta
                .iter()
                .map(|((currency, tenor), amount)| (*currency, tenor.clone(), *amount))
                .collect(),
            ir_vega: sens
                .ir_vega
                .iter()
                .map(|((currency, tenor), amount)| (*currency, tenor.clone(), *amount))
                .collect(),
            credit_qualifying_delta: sens
                .credit_qualifying_delta
                .iter()
                .map(|((name, tenor), amount)| (name.clone(), tenor.clone(), *amount))
                .collect(),
            credit_non_qualifying_delta: sens
                .credit_non_qualifying_delta
                .iter()
                .map(|((name, tenor), amount)| (name.clone(), tenor.clone(), *amount))
                .collect(),
            equity_delta: sens
                .equity_delta
                .iter()
                .map(|(underlier, amount)| (underlier.clone(), *amount))
                .collect(),
            equity_vega: sens
                .equity_vega
                .iter()
                .map(|(underlier, amount)| (underlier.clone(), *amount))
                .collect(),
            fx_delta: sens
                .fx_delta
                .iter()
                .map(|(currency, amount)| (*currency, *amount))
                .collect(),
            fx_vega: sens
                .fx_vega
                .iter()
                .map(|((ccy1, ccy2), amount)| (*ccy1, *ccy2, *amount))
                .collect(),
            commodity_delta: sens
                .commodity_delta
                .iter()
                .map(|(bucket, amount)| (bucket.clone(), *amount))
                .collect(),
            curvature: sens
                .curvature
                .iter()
                .map(|(risk_class, amount)| (*risk_class, *amount))
                .collect(),
            credit_qualifying_delta_bucketed: sens
                .credit_qualifying_delta_bucketed
                .iter()
                .map(|((sector, name, tenor), amount)| {
                    (*sector, name.clone(), tenor.clone(), *amount)
                })
                .collect(),
        }
    }
}

impl From<SimmSensitivitiesJson> for fm::SimmSensitivities {
    fn from(value: SimmSensitivitiesJson) -> Self {
        let mut sens = fm::SimmSensitivities::new(value.base_currency);
        for (currency, tenor, amount) in value.ir_delta {
            sens.add_ir_delta(currency, tenor, amount);
        }
        for (currency, tenor, amount) in value.ir_vega {
            sens.add_ir_vega(currency, tenor, amount);
        }
        for (name, tenor, amount) in value.credit_qualifying_delta {
            sens.add_credit_delta(name, true, tenor, amount);
        }
        for (name, tenor, amount) in value.credit_non_qualifying_delta {
            sens.add_credit_delta(name, false, tenor, amount);
        }
        for (underlier, amount) in value.equity_delta {
            sens.add_equity_delta(underlier, amount);
        }
        for (underlier, amount) in value.equity_vega {
            sens.add_equity_vega(underlier, amount);
        }
        for (currency, amount) in value.fx_delta {
            sens.add_fx_delta(currency, amount);
        }
        for (ccy1, ccy2, amount) in value.fx_vega {
            *sens.fx_vega.entry((ccy1, ccy2)).or_insert(0.0) += amount;
        }
        for (bucket, amount) in value.commodity_delta {
            *sens.commodity_delta.entry(bucket).or_insert(0.0) += amount;
        }
        for (risk_class, amount) in value.curvature {
            *sens.curvature.entry(risk_class).or_insert(0.0) += amount;
        }
        for (sector, name, tenor, amount) in value.credit_qualifying_delta_bucketed {
            sens.add_credit_delta_bucketed(sector, name, tenor, amount);
        }
        sens
    }
}

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
    match version
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-', '.'], "_")
        .as_str()
    {
        "v2_5" | "2_5" | "simm_v2_5" | "simm2_5" => Ok(fm::SimmVersion::V2_5),
        "v2_6" | "2_6" | "simm_v2_6" | "simm2_6" => Ok(fm::SimmVersion::V2_6),
        other => Err(crate::errors::value_error(format!(
            "unknown SIMM version '{other}' (expected 'v2_5' or 'v2_6')"
        ))),
    }
}

fn simm_version_label(version: fm::SimmVersion) -> &'static str {
    match version {
        fm::SimmVersion::V2_5 => "v2_5",
        fm::SimmVersion::V2_6 => "v2_6",
        _ => "unknown",
    }
}

fn parse_credit_sector(sector: &str) -> PyResult<fm::SimmCreditSector> {
    match sector
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "sovereign" | "ig_sovereign" => Ok(fm::SimmCreditSector::Sovereign),
        "financial" | "ig_financial" => Ok(fm::SimmCreditSector::Financial),
        "basic_materials" | "energy_industrials" | "ig_basic_materials" => {
            Ok(fm::SimmCreditSector::BasicMaterials)
        }
        "consumer_goods" | "ig_consumer_goods" => Ok(fm::SimmCreditSector::ConsumerGoods),
        "technology_media" | "technology" | "telecom" | "ig_technology_media" => {
            Ok(fm::SimmCreditSector::TechnologyMedia)
        }
        "health_care" | "healthcare" | "utilities" | "ig_health_care" => {
            Ok(fm::SimmCreditSector::HealthCare)
        }
        "high_yield_sovereign" | "hy_sovereign" => Ok(fm::SimmCreditSector::HighYieldSovereign),
        "high_yield_financial" | "hy_financial" => Ok(fm::SimmCreditSector::HighYieldFinancial),
        "high_yield_basic_materials" | "hy_basic_materials" => {
            Ok(fm::SimmCreditSector::HighYieldBasicMaterials)
        }
        "high_yield_consumer_goods" | "hy_consumer_goods" => {
            Ok(fm::SimmCreditSector::HighYieldConsumerGoods)
        }
        "high_yield_technology_media" | "hy_technology_media" => {
            Ok(fm::SimmCreditSector::HighYieldTechnologyMedia)
        }
        "high_yield_health_care" | "hy_health_care" | "hy_healthcare" => {
            Ok(fm::SimmCreditSector::HighYieldHealthCare)
        }
        "index" => Ok(fm::SimmCreditSector::Index),
        "securitized" | "securitised" => Ok(fm::SimmCreditSector::Securitized),
        "residual" | "other" => Ok(fm::SimmCreditSector::Residual),
        other => Err(crate::errors::value_error(format!(
            "unknown SIMM credit sector '{other}'"
        ))),
    }
}

fn parse_risk_class(risk_class: &str) -> PyResult<fm::SimmRiskClass> {
    match risk_class
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "interest_rate" | "ir" | "rates" => Ok(fm::SimmRiskClass::InterestRate),
        "credit_qualifying" | "credit_qual" | "cq" => Ok(fm::SimmRiskClass::CreditQualifying),
        "credit_non_qualifying" | "credit_nonqual" | "cnq" => {
            Ok(fm::SimmRiskClass::CreditNonQualifying)
        }
        "equity" | "eq" => Ok(fm::SimmRiskClass::Equity),
        "commodity" | "comm" => Ok(fm::SimmRiskClass::Commodity),
        "fx" | "foreign_exchange" => Ok(fm::SimmRiskClass::Fx),
        other => Err(crate::errors::value_error(format!(
            "unknown SIMM risk class '{other}'"
        ))),
    }
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
        let dto: SimmSensitivitiesJson = serde_json::from_str(json).map_err(display_to_py)?;
        let inner = fm::SimmSensitivities::from(dto);
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&SimmSensitivitiesJson::from(&self.inner))
            .map_err(display_to_py)
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
        let key = (parse_currency(ccy1)?, parse_currency(ccy2)?);
        *self.inner.fx_vega.entry(key).or_insert(0.0) += amount;
        Ok(())
    }

    /// Add a commodity delta sensitivity bucket.
    #[pyo3(signature = (bucket, amount))]
    fn add_commodity_delta(&mut self, bucket: &str, amount: f64) {
        *self
            .inner
            .commodity_delta
            .entry(bucket.to_string())
            .or_insert(0.0) += amount;
    }

    /// Add a curvature contribution for a SIMM risk class.
    #[pyo3(signature = (risk_class, amount))]
    fn add_curvature(&mut self, risk_class: &str, amount: f64) -> PyResult<()> {
        *self
            .inner
            .curvature
            .entry(parse_risk_class(risk_class)?)
            .or_insert(0.0) += amount;
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
        simm_version_label(self.inner.version())
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
        sensitivities: &PySimmSensitivities,
        currency: &str,
        year: i32,
        month: u8,
        day: u8,
    ) -> PyResult<PyImResult> {
        let ccy = parse_currency(currency)?;
        let as_of = parse_date(year, month, day)?;
        let (amount, breakdown) = self
            .inner
            .calculate_from_sensitivities(&sensitivities.inner, ccy);
        let amount = money_from_amount(amount, ccy)?;
        Ok(imresult_from_parts(
            amount,
            fm::ImMethodology::Simm,
            as_of,
            self.inner.mpor_days(),
            breakdown,
        ))
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
        let amount = self.inner.calculate_for_notional(
            money_from_amount(notional, ccy)?,
            asset_class.clone(),
            maturity_years,
        );
        Ok(imresult_from_amount(
            amount,
            fm::ImMethodology::Schedule,
            as_of,
            self.inner.mpor_days,
            asset_class.to_string(),
        ))
    }

    /// Calculate schedule IM for a single-asset-class netting set using NGR.
    #[pyo3(signature = (positions, currency, asset_class, maturity_years, year, month, day))]
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
            .calculate_netting_set_with_ngr(&money_positions, asset_class.clone(), maturity_years)
            .map(|amount| {
                imresult_from_amount(
                    amount,
                    fm::ImMethodology::Schedule,
                    as_of,
                    self.inner.mpor_days,
                    format!("{}_ngr", asset_class),
                )
            }))
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
        let amount = self
            .inner
            .calculate_for_collateral(
                money_from_amount(collateral_value, ccy)?,
                &asset_class.inner,
                currency_mismatch,
            )
            .map_err(core_to_py)?;
        Ok(imresult_from_amount(
            amount,
            fm::ImMethodology::Haircut,
            as_of,
            2,
            asset_class.inner.to_string(),
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
