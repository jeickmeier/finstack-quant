//! JSON position parsing helpers for factor-model bindings.

use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope};
use serde::Deserialize;

/// A parsed factor-model position ready for repricing or sensitivity analysis.
pub(crate) struct ParsedPosition {
    /// Position identifier.
    pub id: String,
    /// Boxed instrument parsed from its canonical v1 envelope.
    pub instrument: Box<dyn Instrument>,
    /// Position weight or notional multiplier.
    pub weight: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PositionInput {
    id: String,
    instrument: InstrumentEnvelope,
    weight: f64,
}

/// Parse factor-model position JSON into boxed instruments using the shared
/// instrument JSON pipeline.
///
/// Each element must contain an ID, a canonical v1 instrument envelope, and a
/// weight/notional multiplier. Instrument decoding is delegated to the
/// valuations crate so its canonical instrument schema remains authoritative.
///
/// # Arguments
///
/// * `positions_json` - UTF-8 JSON array of position inputs; every entry has a
///   user ID, an instrument envelope, and a risk weight multiplier.
///
/// # Errors
///
/// Returns validation errors for malformed position or instrument JSON, and
/// propagates unsupported-instrument and instrument-construction errors.
pub(crate) fn parse_positions_json(
    positions_json: &str,
) -> finstack_quant_core::Result<Vec<ParsedPosition>> {
    let specs: Vec<PositionInput> = serde_json::from_str(positions_json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("invalid factor-model positions JSON: {e}"))
    })?;

    specs
        .into_iter()
        .map(|spec| {
            let instrument = spec.instrument.into_boxed()?;
            Ok(ParsedPosition {
                id: spec.id,
                instrument,
                weight: spec.weight,
            })
        })
        .collect()
}

/// Convert parsed positions into the borrowed tuple form required by the
/// factor-model engines.
///
/// # Arguments
///
/// * `positions` - Parsed positions whose owned instruments remain borrowed by
///   the returned engine tuples; callers must keep this slice alive.
pub(crate) fn pricing_positions(
    positions: &[ParsedPosition],
) -> Vec<(String, &dyn Instrument, f64)> {
    positions
        .iter()
        .map(|position| {
            (
                position.id.clone(),
                position.instrument.as_ref() as &dyn Instrument,
                position.weight,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};

    #[test]
    fn parse_positions_json_builds_boxed_instruments() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let positions_json = serde_json::json!([
            {
                "id": "POS-1",
                "instrument": InstrumentEnvelope::new(InstrumentJson::Bond(bond)),
                "weight": 2.0
            }
        ])
        .to_string();

        let positions = parse_positions_json(&positions_json).expect("positions");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].id, "POS-1");
        assert_eq!(positions[0].weight, 2.0);
        assert_eq!(positions[0].instrument.id(), "TEST-BOND");
    }

    #[test]
    fn parse_positions_json_rejects_bare_instruments() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let positions_json = serde_json::json!([
            {
                "id": "POS-1",
                "instrument": InstrumentJson::Bond(bond),
                "weight": 2.0
            }
        ])
        .to_string();

        // schema-rejection-test: sensitivity positions require an envelope.
        let result = parse_positions_json(&positions_json);
        assert!(result.is_err(), "bare instrument JSON must be rejected");
        let error = result.err().expect("error should be present");
        assert!(error.to_string().contains("factor-model positions JSON"));
    }

    #[test]
    fn pricing_positions_preserves_order_and_weights() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let positions_json = serde_json::json!([
            {
                "id": "POS-1",
                "instrument": InstrumentEnvelope::new(InstrumentJson::Bond(bond)),
                "weight": 2.0
            }
        ])
        .to_string();

        let parsed = parse_positions_json(&positions_json).expect("positions");
        let positions = pricing_positions(&parsed);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].0, "POS-1");
        assert_eq!(positions[0].1.id(), "TEST-BOND");
        assert_eq!(positions[0].2, 2.0);
    }
}
