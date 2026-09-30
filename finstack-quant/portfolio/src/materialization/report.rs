//! Structured results and phase timings for portfolio materialization.

use finstack_quant_core::contract::ValidationReport;
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};

/// Nanoseconds spent in each sequential materialization phase.
///
/// Native values retain `std::time::Instant` precision. On WASM, values are
/// derived from the host's monotonic `performance.now()` clock and rounded to
/// nanoseconds, so their effective precision remains host-defined and a phase
/// shorter than the host clock resolution may legitimately report zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaterializationPhases {
    /// Outer bundle parsing time.
    pub parse: u64,
    /// Post-parse count-limit, contract, reference, dependency-shape, and
    /// content-hash validation time.
    pub validate_versions: u64,
    /// Unique instrument decoding and dependency extraction time.
    pub decode_instruments: u64,
    /// Ordered runtime position construction time.
    pub build_positions: u64,
    /// Portfolio index rebuilding and final invariant validation time.
    pub index_build: u64,
}

/// Outcome metadata for one successful portfolio materialization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaterializationReport {
    /// Bounded non-fatal diagnostics retained during loading.
    pub report: ValidationReport,
    /// Number of unique instrument artifacts in the input bundle.
    pub unique_instruments: usize,
    /// Number of ordered positions materialized.
    pub positions: usize,
    /// Sum of normalized market-dependency keys over unique artifacts.
    pub dependencies: usize,
    /// Number of unique artifacts served from the shared decode cache.
    pub cache_hits: usize,
    /// Number of encoded bytes in the input bundle.
    pub input_bytes: usize,
    /// Whether the host supplied a monotonic clock for every phase.
    ///
    /// When false, unavailable phase values are represented as zero in
    /// `phase_nanos` and must not be interpreted as measured durations.
    pub timing_available: bool,
    /// Sequential phase timings in nanoseconds.
    pub phase_nanos: MaterializationPhases,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PhaseTimer = std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub(crate) struct PhaseTimer(Option<f64>);

pub(crate) fn start_timer() -> PhaseTimer {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::Instant::now()
    }
    #[cfg(target_arch = "wasm32")]
    {
        PhaseTimer(monotonic_millis())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn elapsed_nanos(started: PhaseTimer) -> Option<u64> {
    Some(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn elapsed_nanos(started: PhaseTimer) -> Option<u64> {
    let started = started.0?;
    let elapsed_millis = monotonic_millis()? - started;
    if !elapsed_millis.is_finite() || elapsed_millis < 0.0 {
        return None;
    }
    let elapsed_nanos = elapsed_millis * 1_000_000.0;
    if elapsed_nanos >= u64::MAX as f64 {
        Some(u64::MAX)
    } else {
        Some(elapsed_nanos.round() as u64)
    }
}

#[cfg(target_arch = "wasm32")]
fn monotonic_millis() -> Option<f64> {
    let global = js_sys::global();
    let performance = js_sys::Reflect::get(&global, &JsValue::from_str("performance")).ok()?;
    let now = js_sys::Reflect::get(&performance, &JsValue::from_str("now"))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    now.call0(&performance)
        .ok()?
        .as_f64()
        .filter(|value| value.is_finite())
}
