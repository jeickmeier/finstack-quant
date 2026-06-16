//! Loader for CDS conventions embedded in JSON registries.

use super::json::{normalize_registry_id, RegistryFile};
use crate::market::conventions::defs::CdsConventions;
use crate::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;
use std::str::FromStr;
use strum::IntoEnumIterator;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CdsConventionsRecord {
    doc_clause: CdsDocClause,
    day_count: DayCount,
    payment_frequency: String,
    bdc: BusinessDayConvention,
    #[serde(rename = "stub_convention")]
    _stub_convention: String,
    settlement_days: i32,
    calendar_id: String,
}

impl CdsConventionsRecord {
    fn into_conventions(self) -> Result<CdsConventions, Error> {
        let payment_frequency = Tenor::parse(&self.payment_frequency).map_err(|e| {
            Error::Validation(format!(
                "Invalid `payment_frequency` in CDS conventions registry: '{}': {}",
                self.payment_frequency, e
            ))
        })?;

        Ok(CdsConventions {
            calendar_id: self.calendar_id,
            day_count: self.day_count,
            bdc: self.bdc,
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
/// This loader expands `ANY:<Clause>` IDs across all ISO currencies, allowing the embedded
/// registry to define catch-all conventions that apply to any currency not explicitly
/// overridden. Explicit currency IDs (e.g., `USD:IsdaNa`) take precedence over expanded
/// `ANY` entries.
pub(crate) fn load_registry() -> Result<HashMap<CdsConventionKey, CdsConventions>, Error> {
    let json = include_str!("../../../../data/conventions/cds_conventions.json");
    load_registry_from_str(json)
}

fn load_registry_from_str(json: &str) -> Result<HashMap<CdsConventionKey, CdsConventions>, Error> {
    let file: RegistryFile<CdsConventionsRecord> = serde_json::from_str(json).map_err(|e| {
        Error::Validation(format!(
            "Failed to parse embedded CDS conventions registry JSON: {e}"
        ))
    })?;
    file.validate_metadata("CDS")?;

    // Two-pass approach:
    // 1. Collect explicit (Currency, Clause) keys first - these take precedence
    // 2. Collect ANY:<Clause> entries and expand them to all currencies not already present

    let mut final_map: HashMap<CdsConventionKey, CdsConventions> = HashMap::default();
    let mut any_clauses: Vec<(CdsDocClause, CdsConventions)> = Vec::new();
    let mut seen_ids: HashMap<String, ()> = HashMap::default();

    for entry in file.entries {
        let conventions = entry.record.clone().into_conventions()?;
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
                any_clauses.push((clause, conventions.clone()));
            } else if let Ok(currency) = prefix.parse::<Currency>() {
                let clause = parse_doc_clause(clause_str)?;
                if clause != entry.record.doc_clause {
                    return Err(Error::Validation(format!(
                        "CDS convention registry id '{}' doc clause does not match record doc_clause {:?}",
                        key_str, entry.record.doc_clause
                    )));
                }
                let key = CdsConventionKey {
                    currency,
                    doc_clause: clause,
                };
                final_map.insert(key, conventions.clone());
            } else {
                return Err(Error::Validation(format!(
                    "Invalid CDS convention registry id '{}': unknown currency or prefix '{}'",
                    key_str, prefix
                )));
            }
        }
    }

    // Second pass: expand ANY entries to all currencies not already present
    for (clause, conventions) in any_clauses {
        for currency in Currency::iter() {
            let key = CdsConventionKey {
                currency,
                doc_clause: clause,
            };
            // Only insert if not already present (explicit entries take precedence)
            final_map.entry(key).or_insert_with(|| conventions.clone());
        }
    }

    Ok(final_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_asian_currency_overrides_take_precedence_over_any_defaults() {
        let registry = load_registry().expect("cds registry");

        let aud = registry
            .get(&CdsConventionKey {
                currency: Currency::AUD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("AUD IsdaAs");
        assert_eq!(aud.calendar_id, "auce");

        let nzd = registry
            .get(&CdsConventionKey {
                currency: Currency::NZD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("NZD IsdaAs");
        assert_eq!(nzd.calendar_id, "nzau");

        let hkd = registry
            .get(&CdsConventionKey {
                currency: Currency::HKD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("HKD IsdaAs");
        assert_eq!(hkd.calendar_id, "hkhk");

        let sgd = registry
            .get(&CdsConventionKey {
                currency: Currency::SGD,
                doc_clause: CdsDocClause::IsdaAs,
            })
            .expect("SGD IsdaAs");
        assert_eq!(sgd.calendar_id, "sgsi");
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
                        "day_count": "Act360",
                        "payment_frequency": "3M",
                        "bdc": "modified_following",
                        "stub_convention": "ShortFront",
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
                        "day_count": "Act360",
                        "payment_frequency": "3M",
                        "bdc": "modified_following",
                        "stub_convention": "ShortFront",
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
