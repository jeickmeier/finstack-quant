//! Shared loader for embedded JSON registries with optional config overrides.
//!
//! Registries keep ownership of their domain payload and validation logic. This
//! loader handles JSON decoding, process-wide caching for the embedded payload,
//! and optional lookup through [`FinstackConfig`](crate::config::FinstackConfig)
//! extensions.
//!
//! Callers provide:
//! - Declare a `static MY_REGISTRY: EmbeddedJsonRegistry<MyRegistry> = …;`
//! - Provide a `fn validate(reg: MyRegistry) -> Result<MyRegistry>`.
//! - Call `MY_REGISTRY.load(validate)` and `MY_REGISTRY.load_from_config(cfg, validate)`.

use crate::config::FinstackConfig;
use crate::{Error, Result};
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

/// Loader for a single versioned JSON registry shipped as a compile-time asset.
///
/// `T` is the typed registry payload. The loader caches the validated registry
/// in a `OnceLock` so the JSON parse + validation cost is paid at most once
/// per process.
pub struct EmbeddedJsonRegistry<T: 'static> {
    /// Raw JSON content, typically `include_str!("…json")`.
    embedded_raw: &'static str,
    /// Configuration extension key used by `load_from_config` to look for an
    /// override before falling back to the embedded copy.
    extension_key: &'static str,
    /// Human-readable label used in error messages (e.g. "credit assumptions").
    parse_label: &'static str,
    /// Process-wide cache of the parsed-and-validated embedded registry.
    cell: OnceLock<Result<T>>,
}

impl<T> EmbeddedJsonRegistry<T>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// Construct a loader. Intended for `static` storage.
    pub const fn new(
        embedded_raw: &'static str,
        extension_key: &'static str,
        parse_label: &'static str,
    ) -> Self {
        Self {
            embedded_raw,
            extension_key,
            parse_label,
            cell: OnceLock::new(),
        }
    }

    /// Load (and cache) the embedded registry, applying `validate`.
    ///
    /// Returns a borrowed reference to the cached value. If parsing or
    /// validation fails, the failure is also cached and returned by clone on
    /// every subsequent call.
    pub fn load<F>(&self, validate: F) -> Result<&T>
    where
        F: FnOnce(T) -> Result<T>,
    {
        match self
            .cell
            .get_or_init(|| parse_and_validate(self.embedded_raw, self.parse_label, validate))
        {
            Ok(registry) => Ok(registry),
            Err(err) => Err(err.clone()),
        }
    }

    /// Load from configuration, preferring an extension override over the
    /// embedded copy. The same `validate` function is applied to both paths.
    pub fn load_from_config<F>(&self, config: &FinstackConfig, validate: F) -> Result<T>
    where
        F: Fn(T) -> Result<T>,
    {
        if let Some(value) = config.extensions.get(self.extension_key) {
            let raw = serde_json::from_value::<T>(value.clone()).map_err(|err| {
                Error::Validation(format!(
                    "failed to parse {} registry extension: {err}",
                    self.parse_label
                ))
            })?;
            validate(raw)
        } else {
            Ok(self.load(validate)?.clone())
        }
    }
}

fn parse_and_validate<T, F>(raw: &str, parse_label: &str, validate: F) -> Result<T>
where
    T: DeserializeOwned,
    F: FnOnce(T) -> Result<T>,
{
    let registry = serde_json::from_str::<T>(raw).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded {parse_label} registry: {err}"
        ))
    })?;
    validate(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    struct DummyRegistry {
        version: u32,
    }

    const RAW: &str = r#"{"version": 1}"#;

    static REG: EmbeddedJsonRegistry<DummyRegistry> =
        EmbeddedJsonRegistry::new(RAW, "core.dummy_registry.v1", "dummy");

    fn validate_v1(r: DummyRegistry) -> Result<DummyRegistry> {
        if r.version == 1 {
            Ok(r)
        } else {
            Err(Error::Validation(format!(
                "unsupported version {}",
                r.version
            )))
        }
    }

    #[test]
    fn embedded_loads_through_validate() {
        let reg = REG.load(validate_v1).expect("should load");
        assert_eq!(reg.version, 1);
    }

    #[test]
    fn config_extension_takes_precedence() {
        let mut config = FinstackConfig::default();
        let value = serde_json::json!({"version": 1});
        config
            .extensions
            .insert("core.dummy_registry.v1", value)
            .expect("static extension key");
        let reg = REG
            .load_from_config(&config, validate_v1)
            .expect("config-loaded registry");
        assert_eq!(reg.version, 1);
    }

    #[test]
    fn validation_failure_is_propagated() {
        let mut config = FinstackConfig::default();
        let value = serde_json::json!({"version": 99});
        config
            .extensions
            .insert("core.dummy_registry.v1", value)
            .expect("static extension key");
        let err = REG
            .load_from_config(&config, validate_v1)
            .expect_err("invalid version must fail validation");
        assert!(matches!(err, Error::Validation(_)));
    }
}
