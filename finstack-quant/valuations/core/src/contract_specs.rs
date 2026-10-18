//! Embedded exchange contract-specification registry.

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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ContractSpecRegistry {
    schema: String,
    bond_futures: Vec<BondFutureSpecRecord>,
    repo_defaults: Vec<RepoDefaultRecord>,
}

impl ContractSpecRegistry {
    pub fn bond_future_specs(&self, id: &str) -> Result<BondFutureSpecs> {
        let record = self
            .bond_futures
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("bond future contract spec", id))?;
        Ok(BondFutureSpecs {
            standard_coupon: record.standard_coupon,
            standard_maturity_years: record.standard_maturity_years,
            repo_day_count: record.repo_day_count,
        })
    }

    pub fn repo_defaults(&self, id: &str) -> Result<RepoDefaultSpecs> {
        let record = self
            .repo_defaults
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("repo default spec", id))?;
        Ok(record.to_specs())
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
            "repo default spec",
            self.repo_defaults
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        for record in &self.bond_futures {
            record.validate()?;
        }
        for record in &self.repo_defaults {
            record.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RepoDefaultSpecs {
    pub haircut: f64,
    pub calendar_id: String,
    pub day_count: DayCount,
    pub business_day_convention: BusinessDayConvention,
    pub triparty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct BondFutureSpecRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RepoDefaultRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    haircut: f64,
    calendar_id: String,
    day_count: DayCount,
    business_day_convention: BusinessDayConvention,
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
        Ok(())
    }

    fn to_specs(&self) -> RepoDefaultSpecs {
        RepoDefaultSpecs {
            haircut: self.haircut,
            calendar_id: self.calendar_id.clone(),
            day_count: self.day_count,
            business_day_convention: self.business_day_convention,
            triparty: self.triparty,
        }
    }
}

pub fn embedded_registry() -> Result<&'static ContractSpecRegistry> {
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

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum RepoDayCountWire {
    #[serde(rename = "act_360")]
    Act360,
    #[serde(rename = "act_365f")]
    Act365F,
}

impl From<RepoDayCountWire> for DayCount {
    fn from(value: RepoDayCountWire) -> Self {
        match value {
            RepoDayCountWire::Act360 => Self::Act360,
            RepoDayCountWire::Act365F => Self::Act365F,
        }
    }
}

impl TryFrom<DayCount> for RepoDayCountWire {
    type Error = finstack_quant_core::Error;

    fn try_from(value: DayCount) -> std::result::Result<Self, Self::Error> {
        match value {
            DayCount::Act360 => Ok(Self::Act360),
            DayCount::Act365F => Ok(Self::Act365F),
            unsupported => Err(finstack_quant_core::Error::Validation(format!(
                "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
            ))),
        }
    }
}

mod repo_day_count_wire {
    use super::{DayCount, RepoDayCountWire};
    use serde::{Deserialize, Serialize};

    pub(super) fn serialize<S>(
        value: &DayCount,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        RepoDayCountWire::try_from(*value)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> std::result::Result<DayCount, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        RepoDayCountWire::deserialize(deserializer).map(Into::into)
    }
}

/// Contract specifications for bond futures.
///
/// Defines the notional bond parameters used for conversion factor
/// calculations and the implied-repo day count. Contract size lives on the
/// future's `terms`: `terms.multiplier` is the currency value of one full
/// price point, i.e. one hundredth of the per-contract face (1,000 for a
/// $100,000 UST 10Y contract).
///
/// Delivery timing is carried by the future's explicit `delivery_start` /
/// `terms.settlement_date`, so the spec holds no settlement lag or holiday
/// calendar.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
///
/// // UST 10-year contract specs
/// let specs = BondFutureSpecs::default(); // UST 10Y defaults
/// assert_eq!(specs.standard_coupon, 0.06);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BondFutureSpecs {
    /// Standard coupon rate for conversion factor calculation (e.g., 0.06 for 6%)
    pub standard_coupon: f64,
    /// Standard maturity in years for conversion factor calculation
    pub standard_maturity_years: f64,
    /// Day-count convention for implied repo rate annualization.
    ///
    /// Bond-future repo supports `act_360` and `act_365f`.
    #[serde(default = "default_repo_day_count", with = "repo_day_count_wire")]
    #[cfg_attr(feature = "json-schema", schemars(with = "RepoDayCountWire"))]
    pub repo_day_count: DayCount,
}

fn default_repo_day_count() -> DayCount {
    DayCount::Act360
}

pub fn repo_annualization_denominator(day_count: DayCount) -> finstack_quant_core::Result<f64> {
    match day_count {
        DayCount::Act360 => Ok(360.0),
        DayCount::Act365F => Ok(365.0),
        unsupported => Err(finstack_quant_core::Error::Validation(format!(
            "bond-future repo_day_count must be act_360 or act_365f, got {unsupported:?}"
        ))),
    }
}

#[allow(clippy::expect_used)]
fn contract_spec_registry() -> &'static ContractSpecRegistry {
    embedded_registry().expect("embedded contract-spec registry should load")
}

#[allow(clippy::expect_used)]
fn bond_future_specs_from_registry(id: &str) -> BondFutureSpecs {
    contract_spec_registry()
        .bond_future_specs(id)
        .expect("embedded bond future contract spec should exist")
}

impl Default for BondFutureSpecs {
    /// Default specifications for UST 10-year futures.
    ///
    /// Standard parameters:
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% (0.06)
    /// - Standard maturity: 10 years
    fn default() -> Self {
        Self::ust_10y()
    }
}

impl BondFutureSpecs {
    /// UST 10-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 10-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 6.5 years remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_10y();
    /// assert_eq!(specs.standard_coupon, 0.06);
    /// ```
    pub fn ust_10y() -> Self {
        bond_future_specs_from_registry("cme.ust_10y")
    }

    /// UST 5-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 5-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract face: $100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 5 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 4 years, 2 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_5y();
    /// assert_eq!(specs.standard_maturity_years, 5.0);
    /// ```
    pub fn ust_5y() -> Self {
        bond_future_specs_from_registry("cme.ust_5y")
    }

    /// UST 2-year futures contract specifications.
    ///
    /// **Market**: U.S. Treasury
    /// **Exchange**: Chicago Board of Trade (CBOT)
    /// **Contract**: 2-Year T-Note Futures
    ///
    /// # Specifications
    ///
    /// - Contract face: $200,000 (`terms.multiplier` = 2,000 per point; double the 5Y/10Y contracts)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 2 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: U.S. Treasury notes with at least 1 year, 9 months remaining maturity
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::ust_2y();
    /// assert_eq!(specs.standard_maturity_years, 2.0);
    /// ```
    pub fn ust_2y() -> Self {
        bond_future_specs_from_registry("cme.ust_2y")
    }

    /// German Bund futures contract specifications.
    ///
    /// **Market**: Germany (Eurex)
    /// **Exchange**: Eurex Exchange
    /// **Contract**: Euro-Bund Futures
    ///
    /// # Specifications
    ///
    /// - Contract face: €100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 6% annual
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: German Federal bonds with 8.5 to 10.5 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::bund();
    /// assert_eq!(specs.standard_coupon, 0.06);
    /// ```
    pub fn bund() -> Self {
        bond_future_specs_from_registry("eurex.bund")
    }

    /// UK Gilt futures contract specifications.
    ///
    /// **Market**: United Kingdom
    /// **Exchange**: ICE Futures Europe (LIFFE)
    /// **Contract**: Long Gilt Futures
    ///
    /// # Specifications
    ///
    /// - Contract face: £100,000 (`terms.multiplier` = 1,000 per point)
    /// - Standard coupon: 4% annual (note: different from UST/Bund 6%)
    /// - Standard maturity: 10 years
    /// - Day count: Actual/Actual (ISDA)
    /// - Deliverable: UK Gilts with 8.75 to 13 years remaining maturity
    ///
    /// # Notes
    ///
    /// - Quoted in percentage points (e.g., 125.50 = 125.50%)
    /// - Standard coupon is 4%, not 6% like other major markets
    /// - Long Gilt contract covers 8.75-13 year maturity range
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations_core::contract_specs::BondFutureSpecs;
    ///
    /// let specs = BondFutureSpecs::gilt();
    /// assert_eq!(specs.standard_coupon, 0.04);  // 4%, not 6%
    /// ```
    pub fn gilt() -> Self {
        bond_future_specs_from_registry("ice.gilt")
    }
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
        assert_eq!(ust_10y.standard_coupon, 0.06);

        let gilt = registry.bond_future_specs("gilt").expect("gilt spec");
        assert_eq!(gilt.standard_coupon, 0.04);
        assert_eq!(gilt.repo_day_count, DayCount::Act365F);
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

    #[test]
    fn repo_day_count_wire_accepts_only_supported_canonical_values() {
        for value in ["act_360", "act_365f"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_ok(),
                "{value} must be accepted"
            );
        }

        for value in ["act360", "act365", "30_360", "act_act"] {
            let mut json = serde_json::to_value(BondFutureSpecs::default())
                .expect("serialize bond future specs");
            json["repo_day_count"] = serde_json::json!(value);
            assert!(
                serde_json::from_value::<BondFutureSpecs>(json).is_err(),
                "{value} must be rejected"
            );
        }
    }
}
