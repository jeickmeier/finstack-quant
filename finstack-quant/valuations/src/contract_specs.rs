//! Embedded exchange contract-specification registry.

use crate::instruments::equity::vol_index_future::VolIndexContractSpecs;
use crate::instruments::fixed_income::bond_future::BondFutureSpecs;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<ContractSpecRegistry> = EmbeddedJsonRegistry::new(
    include_str!("../data/contract_specs/contract_specs.v1.json"),
    None,
    "contract-spec",
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContractSpecRegistry {
    schema: String,
    bond_futures: Vec<BondFutureSpecRecord>,
    vol_index_futures: Vec<VolIndexFutureSpecRecord>,
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
            standard_coupon: record.standard_coupon,
            standard_maturity_years: record.standard_maturity_years,
            repo_day_count: record.repo_day_count,
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

    pub(crate) fn repo_defaults(&self, id: &str) -> Result<RepoDefaultSpecs> {
        let record = self
            .repo_defaults
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("repo default spec", id))?;
        record.to_specs()
    }

    fn validate(&self) -> Result<()> {
        if self.schema != "finstack_quant.contract_specs/1" {
            return Err(Error::Validation(format!(
                "unsupported contract-spec registry schema version '{}'",
                self.schema
            )));
        }
        finstack_quant_core::validation::validate_unique_ids(
            "contract-spec registry",
            "bond future contract spec",
            self.bond_futures.iter().map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "contract-spec registry",
            "volatility index future contract spec",
            self.vol_index_futures
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "contract-spec registry",
            "repo default spec",
            self.repo_defaults
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        for record in &self.bond_futures {
            record.validate()?;
        }
        for record in &self.vol_index_futures {
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
    standard_coupon: f64,
    standard_maturity_years: f64,
    repo_day_count: DayCount,
}

impl BondFutureSpecRecord {
    fn validate(&self) -> Result<()> {
        finstack_quant_core::validation::validate_source_metadata(
            "bond future contract spec",
            &self.source,
            &self.source_version,
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.effective_date,
            "bond future effective date",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.contract_size,
            "bond future contract size",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.standard_coupon,
            "bond future standard coupon",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.standard_maturity_years,
            "bond future standard maturity years",
        )?;
        if !matches!(self.repo_day_count, DayCount::Act360 | DayCount::Act365F) {
            return Err(Error::Validation(format!(
                "bond future repo_day_count must be act_360 or act_365f, got {:?}",
                self.repo_day_count
            )));
        }
        Ok(())
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
        finstack_quant_core::validation::validate_source_metadata(
            "volatility index future contract spec",
            &self.source,
            &self.source_version,
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.effective_date,
            "volatility index future effective date",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.multiplier,
            "volatility index future multiplier",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.tick_size,
            "volatility index future tick size",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.tick_value,
            "volatility index future tick value",
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.index_id,
            "volatility index future index id",
        )
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
        finstack_quant_core::validation::validate_source_metadata(
            "repo default spec",
            &self.source,
            &self.source_version,
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.effective_date,
            "repo default effective date",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.haircut,
            "repo default haircut",
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.calendar_id,
            "repo default calendar id",
        )?;
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
    EMBEDDED_REGISTRY.load(validate_registry)
}

fn validate_registry(registry: ContractSpecRegistry) -> Result<ContractSpecRegistry> {
    registry.validate()?;
    Ok(registry)
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
        assert_eq!(ust_10y.standard_coupon, 0.06);

        let gilt = registry.bond_future_specs("gilt").expect("gilt spec");
        assert_eq!(gilt.standard_coupon, 0.04);
        assert_eq!(gilt.repo_day_count, DayCount::Act365F);
    }

    #[test]
    fn embedded_registry_preserves_vol_future_specs() {
        let registry = embedded_registry().expect("registry should load");
        let vix_future = registry
            .vol_index_future_specs("cboe.vix_future")
            .expect("VIX future spec");
        assert_eq!(vix_future.multiplier, 1000.0);
        assert_eq!(vix_future.index_id, "VIX");
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
