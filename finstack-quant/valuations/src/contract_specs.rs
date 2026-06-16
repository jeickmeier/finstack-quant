//! Embedded exchange contract-specification registry.

use crate::instruments::equity::equity_index_future::EquityFutureSpecs;
use crate::instruments::equity::vol_index_future::VolIndexContractSpecs;
use crate::instruments::equity::vol_index_option::VolIndexOptionSpecs;
use crate::instruments::fixed_income::bond_future::types::RepoDayCountBasis;
use crate::instruments::fixed_income::bond_future::BondFutureSpecs;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const EMBEDDED_CONTRACT_SPECS: &str = include_str!("../data/contract_specs/contract_specs.v1.json");

static EMBEDDED_REGISTRY: OnceLock<Result<ContractSpecRegistry>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContractSpecRegistry {
    schema_version: String,
    bond_futures: Vec<BondFutureSpecRecord>,
    equity_index_futures: Vec<EquityFutureSpecRecord>,
    vol_index_futures: Vec<VolIndexFutureSpecRecord>,
    vol_index_options: Vec<VolIndexOptionSpecRecord>,
    repo_defaults: Vec<RepoDefaultRecord>,
}

impl ContractSpecRegistry {
    pub(crate) fn bond_future_specs(&self, id: &str) -> Result<BondFutureSpecs> {
        let record = self
            .bond_futures
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("bond future contract spec", id))?;
        Ok(BondFutureSpecs {
            contract_size: record.contract_size,
            tick_size: record.tick_size,
            tick_value: record.tick_value,
            standard_coupon: record.standard_coupon,
            standard_maturity_years: record.standard_maturity_years,
            settlement_days: record.settlement_days,
            calendar_id: record.calendar_id.clone(),
            repo_day_count_basis: record.repo_day_count_basis,
        })
    }

    pub(crate) fn equity_index_future_specs(&self, id: &str) -> Result<EquityFutureSpecs> {
        let record = self
            .equity_index_futures
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("equity index future contract spec", id))?;
        Ok(EquityFutureSpecs {
            multiplier: record.multiplier,
            tick_size: record.tick_size,
            tick_value: record.tick_value,
            settlement_method: record.settlement_method.clone(),
        })
    }

    pub(crate) fn vol_index_future_specs(&self, id: &str) -> Result<VolIndexContractSpecs> {
        let record = self
            .vol_index_futures
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("volatility index future contract spec", id))?;
        Ok(VolIndexContractSpecs {
            multiplier: record.multiplier,
            tick_size: record.tick_size,
            tick_value: record.tick_value,
            index_id: record.index_id.clone(),
        })
    }

    pub(crate) fn vol_index_option_specs(&self, id: &str) -> Result<VolIndexOptionSpecs> {
        let record = self
            .vol_index_options
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("volatility index option contract spec", id))?;
        Ok(VolIndexOptionSpecs {
            multiplier: record.multiplier,
            index_id: record.index_id.clone(),
        })
    }

    pub(crate) fn repo_defaults(&self, id: &str) -> Result<RepoDefaultSpecs> {
        let record = self
            .repo_defaults
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("repo default spec", id))?;
        record.to_specs()
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != "finstack_quant.contract_specs/1" {
            return Err(Error::Validation(format!(
                "unsupported contract-spec registry schema version '{}'",
                self.schema_version
            )));
        }
        validate_ids(
            "bond future contract spec",
            self.bond_futures.iter().map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "equity index future contract spec",
            self.equity_index_futures
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "volatility index future contract spec",
            self.vol_index_futures
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "volatility index option contract spec",
            self.vol_index_options
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "repo default spec",
            self.repo_defaults
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        for record in &self.bond_futures {
            record.validate()?;
        }
        for record in &self.equity_index_futures {
            record.validate()?;
        }
        for record in &self.vol_index_futures {
            record.validate()?;
        }
        for record in &self.vol_index_options {
            record.validate()?;
        }
        for record in &self.repo_defaults {
            record.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RepoDefaultSpecs {
    pub(crate) haircut: f64,
    pub(crate) calendar_id: String,
    pub(crate) day_count: DayCount,
    pub(crate) business_day_convention: BusinessDayConvention,
    pub(crate) triparty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BondFutureSpecRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    contract_size: f64,
    tick_size: f64,
    tick_value: f64,
    standard_coupon: f64,
    standard_maturity_years: f64,
    settlement_days: u32,
    calendar_id: String,
    repo_day_count_basis: RepoDayCountBasis,
}

impl BondFutureSpecRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata(
            "bond future contract spec",
            &self.source,
            &self.source_version,
        )?;
        validate_nonblank("bond future effective date", &self.effective_date)?;
        validate_positive(self.contract_size, "bond future contract size")?;
        validate_positive(self.tick_size, "bond future tick size")?;
        validate_positive(self.tick_value, "bond future tick value")?;
        validate_unit_interval(self.standard_coupon, "bond future standard coupon")?;
        validate_positive(
            self.standard_maturity_years,
            "bond future standard maturity years",
        )?;
        if self.settlement_days == 0 {
            return Err(Error::Validation(
                "contract-spec registry bond future settlement days must be positive".to_string(),
            ));
        }
        validate_nonblank("bond future calendar id", &self.calendar_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EquityFutureSpecRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    multiplier: f64,
    tick_size: f64,
    tick_value: f64,
    settlement_method: String,
}

impl EquityFutureSpecRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata(
            "equity index future contract spec",
            &self.source,
            &self.source_version,
        )?;
        validate_nonblank("equity index future effective date", &self.effective_date)?;
        validate_positive(self.multiplier, "equity index future multiplier")?;
        validate_positive(self.tick_size, "equity index future tick size")?;
        validate_positive(self.tick_value, "equity index future tick value")?;
        validate_nonblank(
            "equity index future settlement method",
            &self.settlement_method,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VolIndexFutureSpecRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    multiplier: f64,
    tick_size: f64,
    tick_value: f64,
    index_id: String,
}

impl VolIndexFutureSpecRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata(
            "volatility index future contract spec",
            &self.source,
            &self.source_version,
        )?;
        validate_nonblank(
            "volatility index future effective date",
            &self.effective_date,
        )?;
        validate_positive(self.multiplier, "volatility index future multiplier")?;
        validate_positive(self.tick_size, "volatility index future tick size")?;
        validate_positive(self.tick_value, "volatility index future tick value")?;
        validate_nonblank("volatility index future index id", &self.index_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VolIndexOptionSpecRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    multiplier: f64,
    index_id: String,
}

impl VolIndexOptionSpecRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata(
            "volatility index option contract spec",
            &self.source,
            &self.source_version,
        )?;
        validate_nonblank(
            "volatility index option effective date",
            &self.effective_date,
        )?;
        validate_positive(self.multiplier, "volatility index option multiplier")?;
        validate_nonblank("volatility index option index id", &self.index_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepoDefaultRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    haircut: f64,
    calendar_id: String,
    day_count: String,
    business_day_convention: String,
    triparty: bool,
}

impl RepoDefaultRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata("repo default spec", &self.source, &self.source_version)?;
        validate_nonblank("repo default effective date", &self.effective_date)?;
        validate_unit_interval(self.haircut, "repo default haircut")?;
        validate_nonblank("repo default calendar id", &self.calendar_id)?;
        self.parse_day_count()?;
        self.parse_business_day_convention()?;
        Ok(())
    }

    fn to_specs(&self) -> Result<RepoDefaultSpecs> {
        Ok(RepoDefaultSpecs {
            haircut: self.haircut,
            calendar_id: self.calendar_id.clone(),
            day_count: self.parse_day_count()?,
            business_day_convention: self.parse_business_day_convention()?,
            triparty: self.triparty,
        })
    }

    fn parse_day_count(&self) -> Result<DayCount> {
        self.day_count.parse().map_err(|err| {
            Error::Validation(format!(
                "contract-spec registry has invalid repo default day_count '{}': {err}",
                self.day_count
            ))
        })
    }

    fn parse_business_day_convention(&self) -> Result<BusinessDayConvention> {
        self.business_day_convention.parse().map_err(|err| {
            Error::Validation(format!(
                "contract-spec registry has invalid repo default business_day_convention '{}': {err}",
                self.business_day_convention
            ))
        })
    }
}

pub(crate) fn embedded_registry() -> Result<&'static ContractSpecRegistry> {
    match EMBEDDED_REGISTRY.get_or_init(|| parse_registry_json(EMBEDDED_CONTRACT_SPECS)) {
        Ok(registry) => Ok(registry),
        Err(err) => Err(err.clone()),
    }
}

fn parse_registry_json(raw: &str) -> Result<ContractSpecRegistry> {
    let registry = serde_json::from_str(raw).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded contract-spec registry: {err}"
        ))
    })?;
    validate_registry(registry)
}

fn validate_registry(registry: ContractSpecRegistry) -> Result<ContractSpecRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn validate_ids<'a>(kind: &str, records: impl Iterator<Item = &'a [String]>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for ids in records {
        if ids.is_empty() {
            return Err(Error::Validation(format!(
                "contract-spec registry contains {kind} without an id"
            )));
        }
        for id in ids {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                return Err(Error::Validation(format!(
                    "contract-spec registry contains blank {kind} id"
                )));
            }
            if !seen.insert(trimmed.to_string()) {
                return Err(Error::Validation(format!(
                    "contract-spec registry contains duplicate {kind} id '{trimmed}'"
                )));
            }
        }
    }
    Ok(())
}

fn validate_metadata(label: &str, source: &str, source_version: &str) -> Result<()> {
    validate_nonblank(label, source)?;
    validate_nonblank(label, source_version)
}

fn validate_nonblank(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::Validation(format!(
            "contract-spec registry has blank {label}"
        )))
    } else {
        Ok(())
    }
}

fn validate_positive(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "contract-spec registry has invalid {label} {value}"
        )))
    }
}

fn validate_unit_interval(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "contract-spec registry has invalid {label} {value}"
        )))
    }
}

fn has_id(ids: &[String], id: &str) -> bool {
    ids.iter().any(|candidate| candidate == id)
}

fn not_found(kind: &str, id: &str) -> Error {
    Error::Validation(format!(
        "contract-spec registry does not contain {kind} '{id}'"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_preserves_bond_future_specs() {
        let registry = embedded_registry().expect("registry should load");
        let ust_10y = registry
            .bond_future_specs("cme.ust_10y")
            .expect("UST 10Y spec");
        assert_eq!(ust_10y.contract_size, 100_000.0);
        assert_eq!(ust_10y.tick_size, 1.0 / 64.0);
        assert_eq!(ust_10y.tick_value, 15.625);
        assert_eq!(ust_10y.standard_coupon, 0.06);

        let gilt = registry.bond_future_specs("gilt").expect("gilt spec");
        assert_eq!(gilt.standard_coupon, 0.04);
        assert_eq!(gilt.repo_day_count_basis, RepoDayCountBasis::Act365);
    }

    #[test]
    fn embedded_registry_preserves_equity_and_vol_specs() {
        let registry = embedded_registry().expect("registry should load");
        let es = registry
            .equity_index_future_specs("cme.es")
            .expect("ES spec");
        assert_eq!(es.multiplier, 50.0);
        assert_eq!(es.tick_value, 12.5);

        let vix_future = registry
            .vol_index_future_specs("cboe.vix_future")
            .expect("VIX future spec");
        assert_eq!(vix_future.multiplier, 1000.0);
        assert_eq!(vix_future.index_id, "VIX");

        let vix_option = registry
            .vol_index_option_specs("cboe.vix_option")
            .expect("VIX option spec");
        assert_eq!(vix_option.multiplier, 100.0);
        assert_eq!(vix_option.index_id, "VIX");
    }

    #[test]
    fn embedded_registry_preserves_repo_defaults() {
        let registry = embedded_registry().expect("registry should load");
        let repo = registry
            .repo_defaults("repo.usd_general_collateral")
            .expect("repo default spec");

        assert_eq!(repo.haircut, 0.02);
        assert_eq!(repo.calendar_id, "usny");
        assert_eq!(repo.day_count, DayCount::Act360);
        assert_eq!(
            repo.business_day_convention,
            BusinessDayConvention::Following
        );
        assert!(!repo.triparty);
    }
}
