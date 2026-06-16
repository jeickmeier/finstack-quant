//! DataFrame export helpers for valuation results.
//!
//! Provides row-oriented helpers to convert valuation results into
//! flat, generic structures suitable for Polars/Pandas DataFrame
//! construction. No metric is special-cased: every entry in
//! [`ValuationResult::measures`] becomes a named column.

use super::ValuationResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Flat row representation of a [`ValuationResult`] for DataFrame export.
///
/// Contains the identifying fields plus every measure keyed by its
/// string `MetricId`. When serialized, measures are flattened into the
/// top-level object alongside the identifying fields.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ValuationRow {
    /// Instrument identifier.
    pub instrument_id: String,
    /// Valuation date (ISO 8601 format).
    pub as_of_date: String,
    /// Present value amount (in `currency`).
    pub pv: f64,
    /// Currency code.
    pub currency: String,
    /// All computed measures, keyed by metric id string.
    #[serde(flatten)]
    pub measures: IndexMap<String, f64>,
}

impl ValuationResult {
    /// Convert this result to a flat row for DataFrame export.
    ///
    /// All measures are emitted as named columns; no metric is
    /// hard-coded or renamed.
    pub fn to_row(&self) -> ValuationRow {
        let measures = self
            .measures
            .iter()
            .map(|(id, &v)| (id.to_string(), v))
            .collect();
        ValuationRow {
            instrument_id: self.instrument_id.clone(),
            as_of_date: self.as_of.to_string(),
            pv: self.value.amount(),
            currency: self.value.currency().to_string(),
            measures,
        }
    }
}

/// Convert multiple valuation results to rows for DataFrame construction.
pub fn results_to_rows(results: &[ValuationResult]) -> Vec<ValuationRow> {
    results.iter().map(ValuationResult::to_row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;

    fn jan15() -> Date {
        Date::from_calendar_date(2025, time::Month::January, 15).expect("valid date")
    }

    #[test]
    fn to_row_carries_identity_and_all_measures() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::Dv01, 1250.0);
        measures.insert(MetricId::Convexity, 125.5);
        measures.insert(MetricId::Ytm, 0.0425);

        let result =
            ValuationResult::stamped("BOND-001", jan15(), Money::new(1_042_315.67, Currency::USD))
                .with_measures(measures);

        let row = result.to_row();

        assert_eq!(row.instrument_id, "BOND-001");
        assert_eq!(row.currency, "USD");
        assert_eq!(row.as_of_date, "2025-01-15");
        assert!((row.pv - 1_042_315.67).abs() < 0.01);
        assert_eq!(row.measures.get("dv01"), Some(&1250.0));
        assert_eq!(row.measures.get("convexity"), Some(&125.5));
        assert_eq!(row.measures.get("ytm"), Some(&0.0425));
        assert!(row.measures.get("duration_mod").is_none());
    }

    #[test]
    fn to_row_preserves_measure_insertion_order() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::Ytm, 0.05);
        measures.insert(MetricId::Dv01, 100.0);
        measures.insert(MetricId::Convexity, 12.3);

        let result =
            ValuationResult::stamped("BOND-ORDER", jan15(), Money::new(1.0, Currency::USD))
                .with_measures(measures);

        let row = result.to_row();
        let keys: Vec<&str> = row.measures.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["ytm", "dv01", "convexity"]);
    }

    #[test]
    fn results_to_rows_batch() {
        let result1 =
            ValuationResult::stamped("BOND-001", jan15(), Money::new(1_000_000.0, Currency::USD));
        let result2 =
            ValuationResult::stamped("BOND-002", jan15(), Money::new(500_000.0, Currency::EUR));

        let rows = results_to_rows(&[result1, result2]);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].instrument_id, "BOND-001");
        assert_eq!(rows[1].currency, "EUR");
    }

    #[test]
    fn row_serde_flattens_measures() {
        let mut measures = IndexMap::new();
        measures.insert(MetricId::Dv01, 500.0);

        let row = ValuationResult::stamped(
            "BOND-SERDE",
            jan15(),
            Money::new(1_000_000.0, Currency::USD),
        )
        .with_measures(measures)
        .to_row();

        let json = serde_json::to_string(&row).expect("should serialize");
        assert!(json.contains("BOND-SERDE"));
        assert!(json.contains("USD"));
        assert!(json.contains("\"dv01\":500"));

        let back: ValuationRow = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(back.instrument_id, "BOND-SERDE");
        assert_eq!(back.measures.get("dv01"), Some(&500.0));
    }
}
