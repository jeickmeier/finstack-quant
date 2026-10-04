//! Loader for CDS conventions embedded in JSON registries.

use super::json::{normalize_registry_id, RegistryFile};
use crate::market::conventions::defs::{CdsConvention, CdsConventionSpec};
use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;
use std::str::FromStr;

/// Parsed CDS convention tables used to build [`ConventionRegistry`].
#[derive(Debug)]
pub(crate) struct CdsRegistryTables {
    /// Explicit `{currency}:{clause}` rows.
    pub entries: HashMap<CdsConventionKey, CdsConventionSpec>,
    /// `ANY:{family}` fallback rows resolved by the registry.
    pub any: HashMap<CdsConvention, CdsConventionSpec>,
    /// Explicit regional family per currency (`isda_na` / `isda_eu` / `isda_as`).
    pub primary_family: HashMap<Currency, CdsConvention>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CdsConventionsRecord {
    doc_clause: CdsDocClause,
    day_count: DayCount,
    payment_frequency: String,
    business_day_convention: BusinessDayConvention,
    stub_convention: StubKind,
    settlement_days: u16,
    calendar_id: String,
}

impl CdsConventionsRecord {
    fn into_spec(self) -> Result<CdsConventionSpec, Error> {
        let family = CdsConvention::family_from_doc_clause(self.doc_clause).ok_or_else(|| {
            Error::Validation(format!(
                "CDS convention registry record doc_clause {:?} is not a regional family",
                self.doc_clause
            ))
        })?;
        let payment_frequency = Tenor::parse(&self.payment_frequency).map_err(|e| {
            Error::Validation(format!(
                "Invalid `payment_frequency` in CDS conventions registry: '{}': {}",
                self.payment_frequency, e
            ))
        })?;

        Ok(CdsConventionSpec {
            family,
            calendar_id: self.calendar_id,
            day_count: self.day_count,
            business_day_convention: self.business_day_convention,
            stub: self.stub_convention,
            settlement_days: self.settlement_days,
            frequency: payment_frequency,
        })
    }
}

fn parse_doc_clause(clause_str: &str) -> Result<CdsDocClause, Error> {
    CdsDocClause::from_str(clause_str).map_err(Error::Validation)
}

/// Load the CDS conventions from the embedded JSON registry.
///
/// This loader stores `ANY:<Clause>` fallbacks separately, allowing the embedded
/// registry to define catch-all conventions that apply to any currency not explicitly
/// overridden. Explicit currency IDs (e.g., `USD:isda_na`) take precedence over resolved
/// `ANY` entries and become that currency's primary schedule family.
pub(crate) fn load_registry() -> Result<CdsRegistryTables, Error> {
    let json = include_str!("../../../../data/conventions/cds_conventions.json");
    load_registry_from_str(json)
}

fn load_registry_from_str(json: &str) -> Result<CdsRegistryTables, Error> {
    let file: RegistryFile<CdsConventionsRecord> = serde_json::from_str(json).map_err(|e| {
        Error::Validation(format!(
            "Failed to parse embedded CDS conventions registry JSON: {e}"
        ))
    })?;
    file.validate_metadata("CDS")?;

    let mut entries: HashMap<CdsConventionKey, CdsConventionSpec> = HashMap::default();
    let mut any: HashMap<CdsConvention, CdsConventionSpec> = HashMap::default();
    let mut primary_family: HashMap<Currency, CdsConvention> = HashMap::default();
    let mut seen_ids: HashMap<String, ()> = HashMap::default();

    for entry in file.entries {
        let spec = entry.record.clone().into_spec()?;
        for id in entry.ids {
            let key_str = normalize_registry_id(&id);
            if seen_ids.insert(key_str.clone(), ()).is_some() {
                return Err(Error::Validation(format!(
                    "Duplicate registry id after normalization: '{}' (from '{}')",
                    key_str, id
                )));
            }

            let (prefix, clause_str) = key_str.split_once(':').ok_or_else(|| {
                Error::Validation(format!(
                    "Invalid CDS convention registry id '{}': expected '<Currency>:<DocClause>' or 'ANY:<DocClause>'",
                    key_str
                ))
            })?;
            if clause_str.contains(':') {
                return Err(Error::Validation(format!(
                    "Invalid CDS convention registry id '{}': expected exactly one ':' separator",
                    key_str
                )));
            }

            if prefix.eq_ignore_ascii_case("ANY") {
                let clause = parse_doc_clause(clause_str)?;
                if clause != entry.record.doc_clause {
                    return Err(Error::Validation(format!(
                        "CDS convention registry id '{}' doc clause does not match record doc_clause {:?}",
                        key_str, entry.record.doc_clause
                    )));
                }
                any.insert(spec.family, spec.clone());
            } else if let Ok(currency) = prefix.parse::<Currency>() {
                let clause = parse_doc_clause(clause_str)?;
                if clause != entry.record.doc_clause {
                    return Err(Error::Validation(format!(
                        "CDS convention registry id '{}' doc clause does not match record doc_clause {:?}",
                        key_str, entry.record.doc_clause
                    )));
                }
                if spec.family != CdsConvention::Custom {
                    primary_family.entry(currency).or_insert(spec.family);
                }
                let key = CdsConventionKey {
                    currency,
                    doc_clause: clause,
                };
                entries.insert(key, spec.clone());
            } else {
                return Err(Error::Validation(format!(
                    "Invalid CDS convention registry id '{}': unknown currency or prefix '{}'",
                    key_str, prefix
                )));
            }
        }
    }

    Ok(CdsRegistryTables {
        entries,
        any,
        primary_family,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_asian_currency_overrides_take_precedence_over_any_defaults() {
        let registry = load_registry().expect("cds registry");

        let aud = registry
            .entries
            .get(&CdsConventionKey {
                currency: Currency::AUD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("AUD IsdaAs");
        assert_eq!(aud.calendar_id, "auce");
        assert_eq!(aud.stub, StubKind::ShortFront);
        assert_eq!(aud.family, CdsConvention::IsdaAs);

        let nzd = registry
            .entries
            .get(&CdsConventionKey {
                currency: Currency::NZD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("NZD IsdaAs");
        assert_eq!(nzd.calendar_id, "nzau");

        let hkd = registry
            .entries
            .get(&CdsConventionKey {
                currency: Currency::HKD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("HKD IsdaAs");
        assert_eq!(hkd.calendar_id, "hkhk");

        let sgd = registry
            .entries
            .get(&CdsConventionKey {
                currency: Currency::SGD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("SGD IsdaAs");
        assert_eq!(sgd.calendar_id, "sgsi");

        assert_eq!(
            registry.primary_family.get(&Currency::AUD),
            Some(&CdsConvention::IsdaAs)
        );
        assert_eq!(
            registry.primary_family.get(&Currency::SEK),
            Some(&CdsConvention::IsdaEu)
        );
    }

    #[test]
    fn malformed_registry_id_errors() {
        let json = r#"{
            "schema": "finstack_quant.instruments.cds.conventions.registry.v2",
            "namespace": "instruments.cds.market_conventions",
            "version": 1,
            "entries": [
                {
                    "ids": ["USD-isda_na"],
                    "record": {
                        "doc_clause": "isda_na",
                        "day_count": "act_360",
                        "payment_frequency": "3M",
                        "business_day_convention": "modified_following",
                        "stub_convention": "short_front",
                        "settlement_days": 3,
                        "calendar_id": "nyse"
                    }
                }
            ]
        }"#;

        let err = load_registry_from_str(json).expect_err("malformed key should fail");
        assert!(
            err.to_string()
                .contains("expected '<Currency>:<DocClause>'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn id_doc_clause_must_match_record_doc_clause() {
        let json = r#"{
            "schema": "finstack_quant.instruments.cds.conventions.registry.v2",
            "namespace": "instruments.cds.market_conventions",
            "version": 1,
            "entries": [
                {
                    "ids": ["USD:isda_eu"],
                    "record": {
                        "doc_clause": "isda_na",
                        "day_count": "act_360",
                        "payment_frequency": "3M",
                        "business_day_convention": "modified_following",
                        "stub_convention": "short_front",
                        "settlement_days": 3,
                        "calendar_id": "nyse"
                    }
                }
            ]
        }"#;

        let err = load_registry_from_str(json).expect_err("mismatched clause should fail");
        assert!(
            err.to_string().contains("does not match record doc_clause"),
            "unexpected error: {err}"
        );
    }
}
