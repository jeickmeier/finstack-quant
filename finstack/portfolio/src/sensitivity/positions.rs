//! JSON position parsing helpers for factor-model bindings.

use finstack_valuations::instruments::{DynInstrument, Instrument};
use serde::Deserialize;

/// A parsed factor-model position ready for repricing or sensitivity analysis.
pub struct ParsedPosition {
    /// Position identifier.
    pub id: String,
    /// Boxed instrument parsed from tagged JSON.
    pub instrument: Box<DynInstrument>,
    /// Position weight or notional multiplier.
    pub weight: f64,
}

#[derive(Deserialize)]
struct PositionInput {
    id: String,
    instrument: serde_json::Value,
    weight: f64,
}

/// Parse factor-model position JSON into boxed instruments using the shared
/// instrument JSON pipeline.
pub fn parse_positions_json(positions_json: &str) -> finstack_core::Result<Vec<ParsedPosition>> {
    let specs: Vec<PositionInput> = serde_json::from_str(positions_json).map_err(|e| {
        finstack_core::Error::Validation(format!("invalid factor-model positions JSON: {e}"))
    })?;

    specs
        .into_iter()
        .map(|spec| {
            let instrument_json = serde_json::to_string(&spec.instrument).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "invalid factor-model instrument JSON for position '{}': {e}",
                    spec.id
                ))
            })?;
            let instrument =
                finstack_valuations::pricer::parse_boxed_instrument_json(&instrument_json, None)?;
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
pub fn pricing_positions(positions: &[ParsedPosition]) -> Vec<(String, &dyn Instrument, f64)> {
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
    use finstack_core::currency::Currency;
    use finstack_core::money::Money;
    use finstack_valuations::instruments::fixed_income::bond::Bond;
    use finstack_valuations::instruments::InstrumentJson;

    #[test]
    fn parse_positions_json_builds_boxed_instruments() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
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

        let positions = parse_positions_json(&positions_json).expect("positions");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].id, "POS-1");
        assert_eq!(positions[0].weight, 2.0);
        assert_eq!(positions[0].instrument.id(), "TEST-BOND");
    }

    #[test]
    fn pricing_positions_preserves_order_and_weights() {
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            time::Date::from_calendar_date(2024, time::Month::January, 1).expect("date"),
            time::Date::from_calendar_date(2034, time::Month::January, 1).expect("date"),
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

        let parsed = parse_positions_json(&positions_json).expect("positions");
        let positions = pricing_positions(&parsed);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].0, "POS-1");
        assert_eq!(positions[0].1.id(), "TEST-BOND");
        assert_eq!(positions[0].2, 2.0);
    }
}
