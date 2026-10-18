//! Breakeven metric configuration wire types.

use crate::metrics::MetricId;

/// Which valuation parameter to solve the breakeven for.
///
/// # Result units
///
/// The breakeven metric is a bare `f64` whose unit depends on the target. Read
/// the per-variant docs before interpreting a value:
///
/// | Target             | Sensitivity     | Result unit          |
/// |--------------------|-----------------|----------------------|
/// | `ZSpread`          | CS01            | basis points         |
/// | `Ytm`              | DV01            | basis points         |
/// | `Oas`              | CS01            | basis points         |
/// | `ImpliedVol`       | Vega            | vol points (1 = 1%)  |
/// | `BaseCorrelation`  | Correlation01   | correlation points   |
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BreakevenTarget {
    /// Z-spread breakeven, in **basis points** (sensitivity: CS01).
    ZSpread,
    /// Yield-to-maturity breakeven, in **basis points** (sensitivity: DV01).
    Ytm,
    /// Implied volatility breakeven, in **vol points** where 1.0 = 1% absolute
    /// vol (sensitivity: Vega).
    ImpliedVol,
    /// Base correlation breakeven, in **correlation points** (sensitivity:
    /// Correlation01).
    ///
    /// Only [`BreakevenMode::Linear`] is supported. Base-correlation skew is
    /// strongly non-linear, so a first-order breakeven here is a coarser
    /// approximation than for spread or yield targets.
    BaseCorrelation,
    /// OAS breakeven, in **basis points** (sensitivity: CS01).
    ///
    /// Note that under [`BreakevenMode::Iterative`] the solve applies a
    /// parallel discount-curve shift. For an instrument with embedded
    /// optionality that is a duration-space answer, not a true OAS shift,
    /// because OAS is defined relative to the option model.
    Oas,
}

impl BreakevenTarget {
    /// Returns the sensitivity [`MetricId`] used to compute the linear breakeven.
    pub fn sensitivity_metric(&self) -> MetricId {
        match self {
            Self::ZSpread | Self::Oas => MetricId::Cs01,
            Self::Ytm => MetricId::Dv01,
            Self::ImpliedVol => MetricId::Vega,
            Self::BaseCorrelation => MetricId::Correlation01,
        }
    }
}

/// Linear (first-order) or iterative (full-reprice root-find) solve mode.
///
/// # Why the two modes disagree
///
/// The gap between them is usually **not** dominated by convexity. `Linear`
/// divides by the sensitivity measured at `as_of`, whereas `Iterative`
/// reprices at the horizon date, where the instrument has less remaining time
/// and therefore a different sensitivity. On a 5Y bond over a 6M horizon the
/// two differ by several percent even though convexity over the ~9bp solved
/// shift contributes only a fraction of that. The gap grows with horizon
/// length, not just with curvature.
///
/// `Iterative` is the more accurate answer where it is supported; `Linear` is
/// the fast approximation and the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BreakevenMode {
    /// `-(carry_total) / sensitivity`, using the sensitivity at `as_of`.
    ///
    /// Fast, first-order: ignores both convexity and the change in sensitivity
    /// over the horizon.
    #[default]
    Linear,
    /// Brent root-find with a full reprice at the horizon date.
    ///
    /// Captures convexity *and* the horizon change in sensitivity. Not
    /// supported for [`BreakevenTarget::BaseCorrelation`], nor for
    /// credit-curve instruments under `ZSpread`/`Oas` — see
    /// [`BreakevenTarget`] and the calculator docs for why.
    Iterative,
}

/// Configuration for the breakeven calculator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BreakevenConfig {
    /// Which valuation parameter to solve for.
    pub target: BreakevenTarget,
    /// Solve mode.
    pub mode: BreakevenMode,
}
