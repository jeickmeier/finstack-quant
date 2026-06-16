//! Core types for volatility surface arbitrage detection.
//!
//! Defines the violation taxonomy, severity model, and aggregated report
//! structure used by all arbitrage checks. These types are serializable
//! for audit trails, Python/WASM interop, and downstream reporting.

use crate::HashMap;

/// A point on the volatility surface where an arbitrage condition is violated.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViolationLocation {
    /// Strike or log-moneyness at which the violation occurs.
    pub strike: f64,
    /// Expiry (years) at which the violation occurs.
    pub expiry: f64,
    /// For calendar spread violations: the adjacent expiry involved.
    pub adjacent_expiry: Option<f64>,
}

/// Classification of the arbitrage condition that was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArbitrageType {
    /// Call prices not convex in strike: C(K-d) - 2C(K) + C(K+d) < 0.
    /// Equivalently, implied density is negative at this strike.
    Butterfly,
    /// Option value decreases with maturity at fixed strike:
    /// w(k, T2) < w(k, T1) for T2 > T1.
    CalendarSpread,
    /// Dupire local variance is non-positive: sigma^2_local(K, T) <= 0.
    /// This is the definitive no-arbitrage condition for a vol surface.
    LocalVolDensity,
    /// SVI-specific: Roger Lee moment formula violated.
    /// Wing slope |dw/dk| > 2 at extreme strikes.
    SviMomentBound,
    /// SVI-specific: Gatheral-Jacquier sufficient conditions for
    /// butterfly-free SVI not satisfied (density g(k) < 0).
    SviButterflyCondition,
    /// SVI-specific: cross-slice total variance ordering violated
    /// (calendar spread between SVI slices).
    SviCalendarSpread,
}

/// Categorical severity of an arbitrage violation.
///
/// Severity is a *function of magnitude relative to tolerance*, not of
/// arbitrage type. The same `Butterfly` violation can be `Negligible` (a
/// rounding artifact at deep wings) or `Critical` (negative density at ATM).
/// Use the `Ord` derive to filter or sort: `Negligible` < `Minor` < `Major`
/// < `Critical`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ArbitrageSeverity {
    /// Violation is within numerical noise (< tolerance).
    /// Likely a finite-difference artifact, not a real arbitrage.
    Negligible,
    /// Small violation that may not be exploitable in practice
    /// given bid-ask spreads, but indicates surface quality issues.
    Minor,
    /// Material violation that would produce negative density or
    /// negative local vol in a region with meaningful option liquidity.
    Major,
    /// Severe violation: negative prices, negative total variance,
    /// or local vol density failure at ATM strikes.
    Critical,
}

impl std::fmt::Display for ArbitrageSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negligible => write!(f, "Negligible"),
            Self::Minor => write!(f, "Minor"),
            Self::Major => write!(f, "Major"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// A single arbitrage violation detected on a volatility surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrageViolation {
    /// What type of arbitrage condition was violated.
    pub violation_type: ArbitrageType,
    /// Where on the surface the violation occurs.
    pub location: ViolationLocation,
    /// Categorical severity.
    pub severity: ArbitrageSeverity,
    /// Magnitude of the violation in total-variance units.
    /// For butterfly: the negative second derivative value.
    /// For calendar spread: the variance decrease (w1 - w2) where w2 < w1.
    /// For local vol density: the negative local variance value.
    pub magnitude: f64,
    /// Human-readable description of the violation.
    pub description: String,
    /// Suggested adjustment to the implied vol at this point to
    /// remove the violation (in vol units, additive). `None` if no
    /// simple fix is available. Reserved for Phase 2 repair.
    pub suggested_fix: Option<f64>,
}

/// Aggregated arbitrage report for a volatility surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrageReport {
    /// Identifier of the surface that was checked.
    pub surface_id: String,
    /// All violations found, sorted by severity (critical first).
    pub violations: Vec<ArbitrageViolation>,
    /// Whether the surface passes all checks (no violations above Negligible).
    pub passed: bool,
    /// Count of violations by type.
    pub counts_by_type: HashMap<ArbitrageType, usize>,
    /// Count of violations by severity.
    pub counts_by_severity: HashMap<ArbitrageSeverity, usize>,
    /// Wall-clock time for the full check suite (microseconds).
    ///
    /// **Non-deterministic.** This field is populated from the host's
    /// monotonic clock and varies between runs. It is excluded from serde
    /// when zero (which is also the default after deserialization), so
    /// golden-test snapshots remain reproducible: serialize sets the field
    /// only on freshly-computed reports, and the field is dropped from
    /// the wire format when the report is round-tripped from a snapshot.
    /// Use [`Self::skip_elapsed`] to drop it explicitly when comparing.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub elapsed_us: u64,
}

/// Helper for `#[serde(skip_serializing_if = ...)]`.
#[inline]
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

impl ArbitrageReport {
    /// Filter violations to only those at or above the given severity.
    pub fn above_severity(&self, min: ArbitrageSeverity) -> Vec<&ArbitrageViolation> {
        self.violations
            .iter()
            .filter(|v| v.severity >= min)
            .collect()
    }

    /// True if any violation is at or above the given severity.
    pub fn has_violations_above(&self, min: ArbitrageSeverity) -> bool {
        self.violations.iter().any(|v| v.severity >= min)
    }

    /// Return a copy of this report with `elapsed_us` zeroed.
    ///
    /// Use when comparing reports against deterministic snapshots: clock
    /// readings vary between runs and would otherwise cause golden-test
    /// false negatives.
    #[must_use]
    pub fn skip_elapsed(mut self) -> Self {
        self.elapsed_us = 0;
        self
    }
}
