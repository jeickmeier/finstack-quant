// ================================================================================================
// Equity Market Data Dependencies
// ================================================================================================

/// Trait for instruments that depend on equity market data.
///
/// Provides a unified interface for discovering equity-related market data dependencies,
/// similar to how [`CurveDependencies`](super::CurveDependencies) works for curves. This
/// trait enables generic metric calculators (e.g., finite difference greeks) to discover
/// which equity market data an instrument requires without runtime downcasting.
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::{EquityDependencies, EquityInstrumentDeps};
///
/// struct EquityOption {
///     spot_id: String,
///     vol_surface_id: String,
/// }
///
/// impl EquityDependencies for EquityOption {
///     fn equity_dependencies(&self) -> finstack_core::Result<EquityInstrumentDeps> {
///         EquityInstrumentDeps::builder()
///             .spot(self.spot_id.clone())
///             .vol_surface(self.vol_surface_id.clone())
///             .build()
///     }
/// }
/// ```
pub trait EquityDependencies {
    /// Return equity market data dependencies for this instrument.
    fn equity_dependencies(&self) -> finstack_core::Result<EquityInstrumentDeps>;
}

/// Collection of equity market data used by an instrument.
///
/// This struct provides a type-safe way to declare equity market data dependencies
/// for risk calculations and generic metric implementations.
///
/// # Examples
///
/// ```rust
/// use finstack_valuations::instruments::EquityInstrumentDeps;
///
/// let deps = EquityInstrumentDeps::builder()
///     .spot("AAPL")
///     .vol_surface("AAPL-VOL")
///     .build()
///     .expect("infallible");
///
/// assert_eq!(deps.spot_id, Some("AAPL".to_string()));
/// assert_eq!(deps.vol_surface_id, Some("AAPL-VOL".to_string()));
/// ```
#[derive(Default, Clone, Debug)]
pub struct EquityInstrumentDeps {
    /// Spot price identifier (e.g., "AAPL", "SPX").
    ///
    /// This is used to look up the current equity price in the market context
    /// for pricing and sensitivity calculations.
    pub spot_id: Option<String>,

    /// Volatility surface identifier.
    ///
    /// This is used to look up implied volatilities for option pricing
    /// and volatility greeks (vega, volga, vanna).
    pub vol_surface_id: Option<String>,

    /// Optional reference strike used to localise vol-surface diagnostics
    /// (currently the vega-clamp detection in finite-difference Greeks).
    ///
    /// When set, the vega calculator checks for the additive
    /// down-bump clamp at the strikes *adjacent* to this value rather than
    /// across the entire surface grid. This avoids forcing a one-sided
    /// (O(h)) difference in the common case where a far-OTM corner of the
    /// surface has σ < bump but the option's actual vega draw is from a
    /// high-σ region of the smile. Instruments with multiple or path-
    /// dependent strikes (basket payoffs, autocallables on non-trivial
    /// triggers) may legitimately leave this unset, in which case the
    /// calculator falls back to the conservative global-min check.
    pub reference_strike: Option<f64>,
}

impl EquityInstrumentDeps {
    /// Create a new empty equity dependencies collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start building an equity dependencies collection.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::EquityInstrumentDeps;
    ///
    /// let deps = EquityInstrumentDeps::builder()
    ///     .spot("SPX")
    ///     .vol_surface("SPX-VOL")
    ///     .build()
    ///     .expect("infallible");
    /// ```
    #[must_use]
    pub fn builder() -> EquityInstrumentDepsBuilder {
        EquityInstrumentDepsBuilder::default()
    }
}

/// Builder for [`EquityInstrumentDeps`].
///
/// Provides a fluent interface for constructing equity dependency declarations.
#[derive(Default)]
pub struct EquityInstrumentDepsBuilder {
    deps: EquityInstrumentDeps,
}

impl EquityInstrumentDepsBuilder {
    /// Add a spot price identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Spot price identifier (e.g., "AAPL", "SPX")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::EquityInstrumentDeps;
    ///
    /// let deps = EquityInstrumentDeps::builder()
    ///     .spot("AAPL")
    ///     .build();
    /// ```
    pub fn spot(mut self, id: impl Into<String>) -> Self {
        self.deps.spot_id = Some(id.into());
        self
    }

    /// Add a volatility surface identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Volatility surface identifier
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::instruments::EquityInstrumentDeps;
    ///
    /// let deps = EquityInstrumentDeps::builder()
    ///     .vol_surface("SPX-VOL")
    ///     .build();
    /// ```
    pub fn vol_surface(mut self, id: impl Into<String>) -> Self {
        self.deps.vol_surface_id = Some(id.into());
        self
    }

    /// Declare a reference strike for vol-surface diagnostics.
    ///
    /// See [`EquityInstrumentDeps::reference_strike`] for the rationale.
    /// Instruments with a single contractual strike (vanilla European,
    /// vanilla barrier, vanilla autocallable) should set this so the
    /// vega clamp detection only inspects the relevant region of the
    /// surface. Multi-strike payoffs that cannot identify a single
    /// reference may omit this; the calculator then falls back to a
    /// global-min check.
    pub fn reference_strike(mut self, strike: f64) -> Self {
        self.deps.reference_strike = Some(strike);
        self
    }

    /// Build the final equity dependencies collection.
    pub fn build(self) -> finstack_core::Result<EquityInstrumentDeps> {
        Ok(self.deps)
    }
}
