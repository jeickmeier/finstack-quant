//! Host-facing authoring constructors for quotes and calibration steps.
//!
//! The Python and WASM bindings expose one named constructor per quote type
//! and per step kind (`RateQuote.deposit`, `CalibrationStep.discount`, ...).
//! Each of them marshals its arguments into the serde wire fields of the
//! target type and calls one of the `from_wire_fields` constructors here.
//! This module owns everything beyond that marshalling: the authoring
//! defaults, the discriminator, string-pillar parsing, strict deserialization
//! (unknown fields rejected) and quote validation.

use crate::api::schema::CalibrationStep;
use crate::quotes::cds::CdsQuote;
use crate::quotes::ids::Pillar;
use crate::quotes::rates::RateQuote;
use crate::quotes::vol::VolQuote;
use finstack_quant_core::{Error, Result};
use serde_json::{Map, Value};

/// Wire fields that hold a [`Pillar`] on rate and CDS quotes.
const PILLAR_FIELDS: [&str; 3] = ["pillar", "start", "end"];

/// Step kinds whose produced object is identified by `curve_id`.
const CURVE_ID_KINDS: [&str; 6] = [
    "discount",
    "forward",
    "hazard",
    "inflation",
    "xccy_basis",
    "parametric",
];

/// Step kinds whose produced object is identified by `vol_surface_id`.
const VOL_SURFACE_ID_KINDS: [&str; 3] = ["vol_surface", "swaption_vol", "svi_surface"];

/// Set `key` to `value` when the field is absent or `null`.
fn default_field(fields: &mut Map<String, Value>, key: &str, value: Value) {
    match fields.get(key) {
        Some(existing) if !existing.is_null() => {}
        _ => {
            fields.insert(key.to_string(), value);
        }
    }
}

/// Replace string pillars (`"5Y"`, `"2030-06-20"`) with their wire objects.
fn normalize_pillars(fields: &mut Map<String, Value>) -> Result<()> {
    for key in PILLAR_FIELDS {
        let Some(Value::String(text)) = fields.get(key) else {
            continue;
        };
        let pillar: Pillar = text.parse()?;
        let wire = serde_json::to_value(pillar)
            .map_err(|error| Error::Validation(format!("failed to encode pillar: {error}")))?;
        fields.insert(key.to_string(), wire);
    }
    Ok(())
}

/// Strictly deserialize a wire object, labelling the failure.
fn from_wire<T: serde::de::DeserializeOwned>(value: Value, label: &str) -> Result<T> {
    serde_json::from_value(value)
        .map_err(|error| Error::Validation(format!("invalid {label}: {error}")))
}

impl RateQuote {
    /// Convexity adjustment (decimal rate) of a futures quote authored without one.
    pub const DEFAULT_CONVEXITY_ADJUSTMENT: f64 = 0.0;

    /// Build a validated rate quote from its type label and serde wire fields.
    ///
    /// # Arguments
    ///
    /// * `kind` - Quote type label: `"deposit"`, `"fra"`, `"futures"` or
    ///   `"swap"`. It becomes the `type` discriminator, replacing any `type`
    ///   entry in `fields`.
    /// * `fields` - Remaining wire fields of that variant (`id`, `index`,
    ///   `pillar`, `rate`, ...), with rates as decimals. A pillar given as a
    ///   string is parsed as a tenor (`"3M"`) or ISO-8601 date. An omitted or
    ///   `null` futures `convexity_adjustment` is
    ///   [`Self::DEFAULT_CONVEXITY_ADJUSTMENT`]. Unknown fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an unknown `kind`, an unparsable pillar,
    /// a missing, unknown or mistyped field, or a quote that fails
    /// [`RateQuote::validate`].
    pub fn from_wire_fields(kind: &str, mut fields: Map<String, Value>) -> Result<Self> {
        normalize_pillars(&mut fields)?;
        if kind == "futures" {
            default_field(
                &mut fields,
                "convexity_adjustment",
                Value::from(Self::DEFAULT_CONVEXITY_ADJUSTMENT),
            );
        }
        fields.insert("type".to_string(), Value::String(kind.to_string()));
        let quote: Self = from_wire(Value::Object(fields), "RateQuote")?;
        quote.validate()?;
        Ok(quote)
    }
}

impl CdsQuote {
    /// Build a validated CDS quote from its type label and serde wire fields.
    ///
    /// # Arguments
    ///
    /// * `kind` - Quote type label: `"cds_par_spread"` or `"cds_upfront"`. It
    ///   becomes the `type` discriminator, replacing any `type` entry in
    ///   `fields`.
    /// * `fields` - Remaining wire fields of that variant: `id`, `entity`,
    ///   `convention` (`{"currency", "doc_clause"}`), `pillar`,
    ///   `recovery_rate` (decimal) and either `spread_bp` (basis points) or
    ///   `coupon_bp` (basis points) with `upfront_pct`. A pillar given as a
    ///   string is parsed as a tenor (`"5Y"`) or ISO-8601 date. Unknown
    ///   fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an unknown `kind`, an unparsable pillar,
    /// a missing, unknown or mistyped field, or a quote that fails
    /// [`CdsQuote::validate`].
    pub fn from_wire_fields(kind: &str, mut fields: Map<String, Value>) -> Result<Self> {
        normalize_pillars(&mut fields)?;
        fields.insert("type".to_string(), Value::String(kind.to_string()));
        let quote: Self = from_wire(Value::Object(fields), "CdsQuote")?;
        quote.validate()?;
        Ok(quote)
    }
}

impl VolQuote {
    /// Option type label of an option-vol quote authored without one.
    pub const DEFAULT_OPTION_TYPE: &'static str = "call";
    /// Volatility convention label of a swaption or cap/floor quote authored without one.
    pub const DEFAULT_QUOTE_TYPE: &'static str = "normal";
    /// Swaption convention identifier of a swaption quote authored without one.
    pub const DEFAULT_SWAPTION_CONVENTION: &'static str = "USD";
    /// Cap (`true`) or floor (`false`) side of a cap/floor quote authored without one.
    pub const DEFAULT_IS_CAP: bool = true;

    /// Build a validated volatility quote from its variant label and serde wire fields.
    ///
    /// # Arguments
    ///
    /// * `kind` - Variant label: `"option_vol"`, `"swaption_vol"` or
    ///   `"cap_floor_vol"`. It becomes the external tag of the wire object.
    /// * `fields` - Wire fields of that variant (`id`, `expiry` as an ISO-8601
    ///   date, `strike`, `vol` as a decimal, ...). Omitted or `null` entries
    ///   take the authoring defaults: `option_type` is
    ///   [`Self::DEFAULT_OPTION_TYPE`], `quote_type` is
    ///   [`Self::DEFAULT_QUOTE_TYPE`], a swaption `convention` is
    ///   [`Self::DEFAULT_SWAPTION_CONVENTION`] and `is_cap` is
    ///   [`Self::DEFAULT_IS_CAP`]. Unknown fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an unknown `kind`, a missing, unknown or
    /// mistyped field, or a quote that fails [`VolQuote::validate`].
    pub fn from_wire_fields(kind: &str, mut fields: Map<String, Value>) -> Result<Self> {
        match kind {
            "option_vol" => {
                default_field(
                    &mut fields,
                    "option_type",
                    Value::from(Self::DEFAULT_OPTION_TYPE),
                );
            }
            "swaption_vol" => {
                default_field(
                    &mut fields,
                    "quote_type",
                    Value::from(Self::DEFAULT_QUOTE_TYPE),
                );
                default_field(
                    &mut fields,
                    "convention",
                    Value::from(Self::DEFAULT_SWAPTION_CONVENTION),
                );
            }
            "cap_floor_vol" => {
                default_field(
                    &mut fields,
                    "quote_type",
                    Value::from(Self::DEFAULT_QUOTE_TYPE),
                );
                default_field(&mut fields, "is_cap", Value::from(Self::DEFAULT_IS_CAP));
            }
            _ => {}
        }
        let mut outer = Map::new();
        outer.insert(kind.to_string(), Value::Object(fields));
        let quote: Self = from_wire(Value::Object(outer), "VolQuote")?;
        quote.validate()?;
        Ok(quote)
    }
}

impl CalibrationStep {
    /// Seniority label of a hazard step authored without one.
    pub const DEFAULT_HAZARD_SENIORITY: &'static str = "senior";
    /// Surface model label of a `vol_surface` step authored without one.
    pub const DEFAULT_VOL_SURFACE_MODEL: &'static str = "sabr";
    /// Parametric family label of a `parametric` step authored without one.
    pub const DEFAULT_PARAMETRIC_MODEL: &'static str = "ns";

    /// Build a calibration step from its kind label and serde wire fields.
    ///
    /// # Arguments
    ///
    /// * `kind` - Step kind label (`"discount"`, `"forward"`, `"hazard"`,
    ///   `"inflation"`, `"vol_surface"`, `"swaption_vol"`,
    ///   `"base_correlation"`, `"student_t"`, `"hull_white"`,
    ///   `"cap_floor_hull_white"`, `"svi_surface"`, `"xccy_basis"` or
    ///   `"parametric"`). It becomes the `kind` discriminator, replacing any
    ///   `kind` entry in `fields`.
    /// * `id` - Step identifier. It replaces any `id` entry in `fields` and is
    ///   the fallback for the quote-set name and the produced object's
    ///   identifier.
    /// * `quote_set` - Name of the quote set in the parent plan's
    ///   `quote_sets`; `None` uses `id`.
    /// * `fields` - Kind-specific wire fields of the matching `StepParams`
    ///   variant (dates as ISO-8601 strings, currencies as ISO-4217 codes).
    ///   Omitted or `null` entries take the authoring defaults: `curve_id`
    ///   (curve kinds) or `vol_surface_id` (surface kinds) is `id`, a hazard
    ///   `seniority` is [`Self::DEFAULT_HAZARD_SENIORITY`], a `vol_surface`
    ///   `model` is [`Self::DEFAULT_VOL_SURFACE_MODEL`] and a `parametric`
    ///   `model` is [`Self::DEFAULT_PARAMETRIC_MODEL`]. Remaining optional
    ///   fields keep their serde defaults; unknown fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an unknown `kind` or a missing, unknown
    /// or mistyped field.
    pub fn from_wire_fields(
        kind: &str,
        id: &str,
        quote_set: Option<&str>,
        mut fields: Map<String, Value>,
    ) -> Result<Self> {
        if CURVE_ID_KINDS.contains(&kind) {
            default_field(&mut fields, "curve_id", Value::from(id));
        }
        if VOL_SURFACE_ID_KINDS.contains(&kind) {
            default_field(&mut fields, "vol_surface_id", Value::from(id));
        }
        match kind {
            "hazard" => default_field(
                &mut fields,
                "seniority",
                Value::from(Self::DEFAULT_HAZARD_SENIORITY),
            ),
            "vol_surface" => default_field(
                &mut fields,
                "model",
                Value::from(Self::DEFAULT_VOL_SURFACE_MODEL),
            ),
            "parametric" => default_field(
                &mut fields,
                "model",
                Value::from(Self::DEFAULT_PARAMETRIC_MODEL),
            ),
            _ => {}
        }
        fields.insert("kind".to_string(), Value::String(kind.to_string()));
        fields.insert("id".to_string(), Value::String(id.to_string()));
        fields.insert(
            "quote_set".to_string(),
            Value::String(quote_set.unwrap_or(id).to_string()),
        );
        from_wire(Value::Object(fields), &format!("{kind} step"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::schema::StepParams;
    use serde_json::json;

    fn map(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => Map::new(),
        }
    }

    #[test]
    fn rate_quote_parses_string_pillars_and_defaults_convexity() {
        let deposit = RateQuote::from_wire_fields(
            "deposit",
            map(json!({"id": "D3M", "index": "USD-SOFR-OIS", "pillar": "3M", "rate": 0.052})),
        )
        .expect("deposit");
        let wire = serde_json::to_value(&deposit).expect("wire");
        assert_eq!(wire["type"], "deposit");
        assert_eq!(
            wire["pillar"],
            serde_json::to_value("3M".parse::<Pillar>().expect("tenor")).expect("pillar")
        );

        let dated = RateQuote::from_wire_fields(
            "swap",
            map(json!({"id": "S", "index": "USD-SOFR-OIS", "pillar": "2030-06-20", "rate": 0.04})),
        )
        .expect("swap");
        assert_eq!(
            serde_json::to_value(&dated).expect("wire")["pillar"]["date"],
            "2030-06-20"
        );

        let futures = RateQuote::from_wire_fields(
            "futures",
            map(
                json!({"id": "F", "contract": "CME:SR3", "expiry": "2026-09-15", "price": 96.5,
                       "convexity_adjustment": null}),
            ),
        )
        .expect("futures");
        assert_eq!(
            serde_json::to_value(&futures).expect("wire")["convexity_adjustment"],
            0.0
        );
    }

    #[test]
    fn rate_quote_rejects_unknown_kinds_fields_and_pillars() {
        let fields = || map(json!({"id": "D", "index": "X", "pillar": "3M", "rate": 0.05}));
        assert!(RateQuote::from_wire_fields("bond", fields()).is_err());
        let mut extra = fields();
        extra.insert("bogus".into(), json!(1));
        assert!(RateQuote::from_wire_fields("deposit", extra).is_err());
        let mut bad_pillar = fields();
        bad_pillar.insert("pillar".into(), json!("soon"));
        assert!(RateQuote::from_wire_fields("deposit", bad_pillar).is_err());
        let mut non_finite = fields();
        non_finite.insert("rate".into(), json!(null));
        assert!(RateQuote::from_wire_fields("deposit", non_finite).is_err());
    }

    #[test]
    fn cds_quote_builds_both_variants() {
        let convention = json!({"currency": "USD", "doc_clause": "isda_na"});
        let par = CdsQuote::from_wire_fields(
            "cds_par_spread",
            map(
                json!({"id": "ACME-5Y", "entity": "ACME", "convention": convention, "pillar": "5Y",
                       "spread_bp": 80.0, "recovery_rate": 0.4}),
            ),
        )
        .expect("par spread");
        assert_eq!(par.coupon_bp(), 80.0);
        let upfront = CdsQuote::from_wire_fields(
            "cds_upfront",
            map(json!({"id": "ACME-5Y-U", "entity": "ACME", "convention": convention, "pillar": "5Y",
                       "coupon_bp": 100.0, "upfront_pct": 0.01, "recovery_rate": 0.4})),
        )
        .expect("upfront");
        assert_eq!(upfront.coupon_bp(), 100.0);
        assert!(CdsQuote::from_wire_fields("cds_upfront", Map::new()).is_err());
    }

    #[test]
    fn vol_quote_applies_authoring_defaults() {
        let option = VolQuote::from_wire_fields(
            "option_vol",
            map(json!({"id": "O", "underlying": "AAPL", "expiry": "2027-05-08", "strike": 155.0, "vol": 0.28})),
        )
        .expect("option vol");
        assert_eq!(
            serde_json::to_value(&option).expect("wire")["option_vol"]["option_type"],
            "call"
        );

        let swaption = VolQuote::from_wire_fields(
            "swaption_vol",
            map(json!({"id": "S", "expiry": "2027-05-08", "maturity": "2032-05-08", "strike": 0.04, "vol": 0.007})),
        )
        .expect("swaption vol");
        let wire = serde_json::to_value(&swaption).expect("wire");
        assert_eq!(wire["swaption_vol"]["quote_type"], "normal");
        assert_eq!(wire["swaption_vol"]["convention"], "USD");

        let floor = VolQuote::from_wire_fields(
            "cap_floor_vol",
            map(json!({"id": "C", "expiry": "2027-05-08", "strike": 0.04, "vol": 0.007, "is_cap": false})),
        )
        .expect("floor vol");
        let wire = serde_json::to_value(&floor).expect("wire");
        assert_eq!(wire["cap_floor_vol"]["is_cap"], false);
        assert_eq!(wire["cap_floor_vol"]["quote_type"], "normal");
        assert!(VolQuote::from_wire_fields("fx_vol", Map::new()).is_err());
    }

    #[test]
    fn step_defaults_identifiers_from_the_step_id() {
        let step = CalibrationStep::from_wire_fields(
            "discount",
            "USD-OIS",
            None,
            map(json!({"currency": "USD", "base_date": "2026-05-08", "curve_id": null})),
        )
        .expect("discount step");
        assert_eq!(step.id, "USD-OIS");
        assert_eq!(step.quote_set, "USD-OIS");
        let StepParams::Discount(params) = &step.params else {
            panic!("expected a discount step");
        };
        assert_eq!(params.curve_id.as_str(), "USD-OIS");

        let named = CalibrationStep::from_wire_fields(
            "discount",
            "step",
            Some("quotes"),
            map(json!({"currency": "USD", "base_date": "2026-05-08", "curve_id": "USD-OIS"})),
        )
        .expect("named discount step");
        assert_eq!(named.quote_set, "quotes");
        let wire = serde_json::to_value(&named).expect("wire");
        assert_eq!(wire["curve_id"], "USD-OIS");
        assert_eq!(wire["kind"], "discount");
    }

    #[test]
    fn step_defaults_labels_per_kind() {
        let hazard = CalibrationStep::from_wire_fields(
            "hazard",
            "ACME",
            None,
            map(
                json!({"entity": "ACME", "currency": "USD", "base_date": "2026-05-08",
                       "discount_curve_id": "USD-OIS", "recovery_rate": 0.4}),
            ),
        )
        .expect("hazard step");
        assert_eq!(
            serde_json::to_value(&hazard).expect("wire")["seniority"],
            "senior"
        );

        let surface = CalibrationStep::from_wire_fields(
            "vol_surface",
            "AAPL-VOL",
            None,
            map(json!({"base_date": "2026-05-08", "underlying_ticker": "AAPL"})),
        )
        .expect("vol surface step");
        let wire = serde_json::to_value(&surface).expect("wire");
        assert_eq!(wire["model"], "sabr");
        assert_eq!(wire["vol_surface_id"], "AAPL-VOL");

        let parametric = CalibrationStep::from_wire_fields(
            "parametric",
            "USD-NS",
            None,
            map(json!({"base_date": "2026-05-08"})),
        )
        .expect("parametric step");
        let wire = serde_json::to_value(&parametric).expect("wire");
        assert_eq!(wire["model"], "ns");
        assert_eq!(wire["curve_id"], "USD-NS");
    }

    #[test]
    fn step_rejects_unknown_kinds_and_fields() {
        assert!(CalibrationStep::from_wire_fields("bogus", "x", None, Map::new()).is_err());
        let error = CalibrationStep::from_wire_fields(
            "discount",
            "USD-OIS",
            None,
            map(json!({"currency": "USD", "base_date": "2026-05-08", "bogus": 1})),
        )
        .expect_err("unknown field");
        assert!(error.to_string().contains("invalid discount step"));
    }
}
