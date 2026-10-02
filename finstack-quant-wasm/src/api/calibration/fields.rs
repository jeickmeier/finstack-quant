//! Wire-field marshalling shared by the quote and step constructor twins.

use crate::utils::input::{
    from_js_json, js_f64, js_opt_bool, js_opt_f64, js_opt_string, js_string, js_uint,
};
use crate::utils::{date_to_iso, parse_iso_date};
use serde_json::{Map, Value};
use wasm_bindgen::prelude::*;

/// Wire-field accumulator: converts each JavaScript argument strictly and
/// stores it under its serde field name. Omitted optional arguments are left
/// out so the Rust constructor applies its default.
pub(super) struct Fields(pub(super) Map<String, Value>);

impl Fields {
    pub(super) fn new() -> Self {
        Self(Map::new())
    }

    /// Start from the optional `params` overrides object (or JSON string).
    pub(super) fn from_overrides(params: Option<&JsValue>) -> Result<Self, JsValue> {
        match params {
            Some(value) if !(value.is_null() || value.is_undefined()) => {
                from_js_json(value, "params").map(Self)
            }
            _ => Ok(Self::new()),
        }
    }

    pub(super) fn string(
        mut self,
        key: &str,
        value: &JsValue,
        label: &str,
    ) -> Result<Self, JsValue> {
        self.0
            .insert(key.to_string(), Value::String(js_string(value, label)?));
        Ok(self)
    }

    pub(super) fn opt_string(
        mut self,
        key: &str,
        value: Option<&JsValue>,
        label: &str,
    ) -> Result<Self, JsValue> {
        if let Some(text) = js_opt_string(value, label)? {
            self.0.insert(key.to_string(), Value::String(text));
        }
        Ok(self)
    }

    pub(super) fn date(mut self, key: &str, value: &JsValue, label: &str) -> Result<Self, JsValue> {
        let date = parse_iso_date(&js_string(value, label)?)?;
        self.0
            .insert(key.to_string(), Value::String(date_to_iso(date)));
        Ok(self)
    }

    pub(super) fn number(
        mut self,
        key: &str,
        value: &JsValue,
        label: &str,
    ) -> Result<Self, JsValue> {
        self.0
            .insert(key.to_string(), Value::from(js_f64(value, label)?));
        Ok(self)
    }

    pub(super) fn opt_number(
        mut self,
        key: &str,
        value: Option<&JsValue>,
        label: &str,
    ) -> Result<Self, JsValue> {
        if let Some(number) = js_opt_f64(value, label)? {
            self.0.insert(key.to_string(), Value::from(number));
        }
        Ok(self)
    }

    pub(super) fn opt_bool(
        mut self,
        key: &str,
        value: Option<&JsValue>,
        label: &str,
    ) -> Result<Self, JsValue> {
        if let Some(flag) = js_opt_bool(value, label)? {
            self.0.insert(key.to_string(), Value::Bool(flag));
        }
        Ok(self)
    }

    /// A pillar: a tenor / ISO-date string (parsed by Rust) or a
    /// `{"tenor": ...}` / `{"date": ...}` object.
    pub(super) fn pillar(
        mut self,
        key: &str,
        value: &JsValue,
        label: &str,
    ) -> Result<Self, JsValue> {
        let pillar = match value.as_string() {
            Some(text) => Value::String(text),
            None => from_js_json(value, label)?,
        };
        self.0.insert(key.to_string(), pillar);
        Ok(self)
    }

    /// The nested CDS convention key `{currency, doc_clause}`.
    pub(super) fn cds_convention(
        mut self,
        currency: &JsValue,
        doc_clause: &JsValue,
    ) -> Result<Self, JsValue> {
        let mut convention = Map::new();
        convention.insert(
            "currency".to_string(),
            Value::String(js_string(currency, "currency")?),
        );
        convention.insert(
            "doc_clause".to_string(),
            Value::String(js_string(doc_clause, "docClause")?),
        );
        self.0
            .insert("convention".to_string(), Value::Object(convention));
        Ok(self)
    }

    /// The base-correlation index series number.
    pub(super) fn series(mut self, value: &JsValue) -> Result<Self, JsValue> {
        let series: u16 = js_uint(value, "series")?;
        self.0.insert("series".to_string(), Value::from(series));
        Ok(self)
    }
}
