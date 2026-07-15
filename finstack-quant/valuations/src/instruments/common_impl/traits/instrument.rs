use crate::cashflow::traits::CashflowProvider;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::model_params::ModelParamsSnapshot;
use crate::metrics::MetricId;
use crate::pricer::{
    actionable_unknown_pricer_message, shared_standard_registry, InstrumentType, ModelKey,
    PricerRegistry, PricingError,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::{currency::Currency, dates::Date, money::Money, types::CurveId, Error};
use std::any::Any;

use super::pricing_options::PricingOptions;

/// Unified instrument trait combining identity, attributes, and pricing.
///
/// This is the primary trait for all financial instruments in the valuation framework.
/// It provides a consistent interface for instrument identification, metadata access,
/// and pricing operations. All concrete instrument types (Bond, Swap, Option, etc.)
/// implement this trait.
///
/// # Core Responsibilities
///
/// 1. **Identity**: Unique instrument identifiers and type information
/// 2. **Metadata**: Tags and attributes for categorization and scenario selection
/// 3. **Pricing**: Present value calculation with optional risk metrics
/// 4. **Type Safety**: Strongly-typed instrument dispatch and downcasting
///
/// # Pricing Methods
///
/// The trait provides two pricing methods with different performance characteristics:
///
/// - [`value()`](Instrument::value): Fast NPV-only calculation (no metrics)
/// - [`price_with_metrics()`](Instrument::price_with_metrics): NPV plus requested risk metrics
///
/// `value()` is the canonical rounded pricing path and returns a [`Money`] amount
/// in the instrument's reporting currency. [`value_raw()`](Instrument::value_raw)
/// should be used only when the caller needs the same economics before currency
/// rounding, typically for finite-difference risk calculations.
///
/// # Implementation Guidelines
///
/// Instruments should:
/// - Return unique, stable identifiers from `id()`
/// - Map to the correct `InstrumentType` variant in `key()`
/// - Implement efficient `value()` for hot paths (portfolio aggregation)
/// - Compute metrics on-demand in `price_with_metrics()`
/// - Support `clone_box()` for trait object cloning
///
/// # Examples
///
/// ## Basic Pricing
///
/// ```rust
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_core::dates::create_date;
/// use finstack_quant_core::types::Rate;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let issue = create_date(2025, Month::January, 15)?;
/// let maturity = create_date(2030, Month::January, 15)?;
/// let bond = Bond::fixed(
///     "BOND-001",
///     Money::new(1_000_000.0, Currency::USD),
///     Rate::from_percent(5.0),
///     issue,
///     maturity,
///     "USD-OIS"
/// )?;
///
/// let market = MarketContext::new();
/// let as_of = create_date(2025, Month::January, 1)?;
///
/// // Fast NPV calculation
/// // let pv = bond.value(&market, as_of)?;
/// // println!("PV: {}", pv);
/// # Ok(())
/// # }
/// ```
///
/// ## Pricing with Metrics
///
/// ```rust
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::metrics::MetricId;
/// # use finstack_quant_core::currency::Currency;
/// # use finstack_quant_core::money::Money;
/// # use finstack_quant_core::dates::create_date;
/// # use finstack_quant_core::types::Rate;
/// # use finstack_quant_core::market_data::context::MarketContext;
/// # use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let issue = create_date(2025, Month::January, 15)?;
/// # let maturity = create_date(2030, Month::January, 15)?;
/// # let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
/// #     Rate::from_percent(5.0), issue, maturity, "USD-OIS");
/// # let market = MarketContext::new();
/// # let as_of = create_date(2025, Month::January, 1)?;
///
/// // Request specific metrics
/// let metrics = vec![MetricId::Ytm, MetricId::DurationMod, MetricId::Dv01];
/// // let result = bond.price_with_metrics(&market, as_of, &metrics)?;
///
/// // Access metrics
/// // println!("YTM: {:.2}%", result.measures["ytm"] * 100.0);
/// // println!("Duration: {:.2}", result.measures["duration_mod"]);
/// # Ok(())
/// # }
/// ```
///
/// ## Trait Object Usage
///
/// ```rust
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::instruments::Instrument;
/// # use finstack_quant_core::currency::Currency;
/// # use finstack_quant_core::money::Money;
/// # use finstack_quant_core::dates::create_date;
/// # use finstack_quant_core::types::Rate;
/// # use time::Month;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let issue = create_date(2025, Month::January, 15)?;
/// # let maturity = create_date(2030, Month::January, 15)?;
/// let bond = Bond::fixed(
///     "BOND-001",
///     Money::new(1_000_000.0, Currency::USD),
///     Rate::from_percent(5.0),
///     issue,
///     maturity,
///     "USD-OIS"
/// )?;
///
/// // Use as trait object
/// let instrument: Box<dyn Instrument> = Box::new(bond);
/// assert_eq!(instrument.id(), "BOND-001");
///
/// // Clone trait object
/// let cloned = instrument.clone_box();
/// assert_eq!(cloned.id(), "BOND-001");
/// # Ok(())
/// # }
/// ```
pub trait Instrument: CashflowProvider + Send + Sync {
    /// Get the instrument's unique identifier.
    ///
    /// Returns a stable string identifier that uniquely identifies this
    /// instrument within a portfolio or system. IDs should be immutable
    /// and consistent across serialization boundaries.
    ///
    /// # Returns
    ///
    /// String slice containing the instrument ID
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed(
    ///     "US-TREASURY-5Y-001",
    ///     Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0),
    ///     issue,
    ///     maturity,
    ///     "USD-OIS"
    /// )?;
    ///
    /// assert_eq!(bond.id(), "US-TREASURY-5Y-001");
    /// # Ok(())
    /// # }
    /// ```
    fn id(&self) -> &str;

    /// Get the strongly-typed instrument key for pricer dispatch.
    ///
    /// Returns the `InstrumentType` enum variant corresponding to this
    /// instrument's type. Used by the pricing registry to route instruments
    /// to the correct pricer implementation.
    ///
    /// # Returns
    ///
    /// `InstrumentType` enum variant (e.g., `InstrumentType::Bond`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// use finstack_quant_valuations::pricer::InstrumentType;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// assert_eq!(bond.key(), InstrumentType::Bond);
    /// # Ok(())
    /// # }
    /// ```
    fn key(&self) -> InstrumentType;

    /// Access to the concrete type for downcasting.
    ///
    /// Enables downcasting from `dyn Instrument` trait objects to concrete
    /// instrument types using `Any::downcast_ref()`.
    ///
    /// # Returns
    ///
    /// Reference to `dyn Any` for dynamic type checking
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// let instrument: &dyn Instrument = &bond;
    /// let concrete_bond: Option<&Bond> = instrument.as_any().downcast_ref::<Bond>();
    /// assert!(concrete_bond.is_some());
    /// # Ok(())
    /// # }
    /// ```
    fn as_any(&self) -> &dyn Any;

    /// Access to the concrete type for mutable downcasting.
    ///
    /// Enables mutable downcasting from `dyn Instrument` trait objects to
    /// concrete instrument types using `Any::downcast_mut()`. Used by
    /// scenario adapters that need to modify typed instrument state
    /// (e.g., correlation shocks on `StructuredCredit`).
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Pre-seed a metric context with instrument-specific cached data.
    ///
    /// This hook allows instruments to supply pre-computed values (e.g., cashflows,
    /// discount curve ID, notional) into a [`MetricContext`](crate::metrics::MetricContext)
    /// before the metrics pipeline runs, avoiding redundant computation.
    ///
    /// The default implementation is a no-op. Instruments with expensive cashflow
    /// generation (e.g., structured credit waterfall simulation) should override
    /// this to pre-seed the context.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable metric context to seed
    /// * `market` - Market data context (for computing cashflows if needed)
    /// * `as_of` - Valuation date
    fn seed_metric_context(
        &self,
        _context: &mut crate::metrics::MetricContext,
        _market: &MarketContext,
        _as_of: Date,
    ) {
        // Default no-op
    }

    /// Expose this instrument as a [`finstack_quant_margin::Marginable`] when supported.
    ///
    /// This hook enables the portfolio margin aggregator to obtain margin
    /// sensitivities without relying on manual downcasting for each concrete
    /// instrument type.
    ///
    /// Instruments that implement [`finstack_quant_margin::Marginable`] should override
    /// this method to return `Some(self)`. The default implementation returns
    /// `None`, indicating that the instrument does not support margin calculations.
    fn as_marginable(&self) -> Option<&dyn finstack_quant_margin::Marginable> {
        None
    }

    /// Snapshot model parameters that can be isolated during attribution.
    ///
    /// The default implementation returns [`ModelParamsSnapshot::None`].
    /// Instruments with model inputs that attribution can revalue independently
    /// should override this method and return their concrete snapshot variant.
    fn model_params_snapshot(&self) -> ModelParamsSnapshot {
        ModelParamsSnapshot::None
    }

    /// Clone this instrument with replacement model parameters.
    ///
    /// The default implementation accepts [`ModelParamsSnapshot::None`] and
    /// returns a normal clone. Concrete snapshots are rejected unless the
    /// instrument overrides this hook, keeping model-parameter attribution
    /// dispatch behind the instrument contract.
    fn with_model_params(
        &self,
        params: &ModelParamsSnapshot,
    ) -> finstack_quant_core::Result<Box<dyn Instrument>> {
        match params {
            ModelParamsSnapshot::None => Ok(self.clone_box()),
            _ => Err(Error::Validation(format!(
                "Instrument '{}' ({:?}) does not support model parameter replacement",
                self.id(),
                self.key()
            ))),
        }
    }

    /// Get immutable reference to instrument attributes.
    ///
    /// Returns attributes containing tags and metadata for categorization
    /// and scenario selection.
    ///
    /// # Returns
    ///
    /// Reference to `Attributes`
    fn attributes(&self) -> &Attributes;

    /// Get mutable reference to instrument attributes.
    ///
    /// Allows modifying tags and metadata after instrument construction.
    ///
    /// # Returns
    ///
    /// Mutable reference to `Attributes`
    fn attributes_mut(&mut self) -> &mut Attributes;

    /// Get instrument-owned pricing inputs.
    fn get_instrument_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::InstrumentPricingOverrides> {
        None
    }

    /// Get mutable instrument-owned pricing inputs.
    fn get_instrument_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::InstrumentPricingOverrides> {
        None
    }

    /// Get metric-only pricing controls.
    fn get_metric_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::MetricPricingOverrides> {
        None
    }

    /// Get mutable metric-only pricing controls.
    fn get_metric_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::MetricPricingOverrides> {
        None
    }

    /// Get scenario-only pricing adjustments.
    fn get_scenario_pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::ScenarioPricingOverrides> {
        None
    }

    /// Get mutable scenario-only pricing adjustments.
    fn get_scenario_pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::ScenarioPricingOverrides> {
        None
    }

    /// Whether this instrument's pricer consumes
    /// [`ScenarioPricingOverrides::scenario_spread_shock_bp`](crate::instruments::pricing_overrides::ScenarioPricingOverrides::scenario_spread_shock_bp)
    /// in its current configuration.
    ///
    /// Scenario engines use this to route instrument-level spread shocks: when
    /// `true`, the shock is accumulated into the scenario overrides and applied
    /// as an additional flat Z-spread during valuation; when `false`, callers
    /// must fall back to other mechanisms (e.g. metadata tagging or curve-level
    /// shocks) because setting the override would either be ignored or rejected
    /// at pricing time.
    ///
    /// The default implementation returns `false`. Implementations should
    /// return `true` only when the shock is consumed exactly (no silent
    /// no-ops): e.g. `Bond` returns `true` for bonds without embedded options,
    /// without an assigned credit curve, and without a price-pinning quote
    /// override other than `quoted_z_spread`.
    fn scenario_spread_shock_supported(&self) -> bool {
        false
    }

    /// Clone this instrument as a boxed trait object.
    ///
    /// Enables cloning instruments behind trait objects. Required because
    /// `Clone` cannot be made into a trait object directly.
    ///
    /// # Returns
    ///
    /// Boxed clone of the instrument
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// let instrument: Box<dyn Instrument> = Box::new(bond);
    /// let cloned = instrument.clone_box();
    /// assert_eq!(cloned.id(), instrument.id());
    /// # Ok(())
    /// # }
    /// ```
    fn clone_box(&self) -> Box<dyn Instrument>;

    /// Returns a normalized instrument for computing spread/yield metrics.
    ///
    /// When a non-discounting model (hazard, MC, OAS) produces a price, the
    /// standard metrics pipeline solves z-spread, YTM, duration, etc. against
    /// the instrument's cashflows.  For instruments with non-standard cashflow
    /// structures (e.g. PIK bonds with compounded terminal notional), the
    /// "native" z-spread is not comparable to a standard cash-pay bond.
    ///
    /// This method returns a version of the instrument whose cashflows are
    /// normalized to the standard (discount-model) basis.  The default
    /// implementation returns `self.clone_box()` — instruments with standard
    /// cashflows need no override.  Bonds override this to convert PIK coupon
    /// type to Cash so that spread metrics are on a cash-equivalent basis.
    ///
    /// This method affects spread- and yield-style metrics only. It should not be
    /// used to change the economic basis of PV itself or unrelated risk measures.
    fn metrics_equivalent(&self) -> Box<dyn Instrument> {
        self.clone_box()
    }

    /// Returns `true` when [`Self::metrics_equivalent`] changes the instrument's
    /// metric cashflow basis.
    ///
    /// The default equivalent is just `clone_box()`. Non-discounting pricing
    /// paths use this flag to avoid splitting spread and risk metrics when both
    /// sides would consume the same cloned instrument.
    fn has_custom_metrics_equivalent(&self) -> bool {
        false
    }

    /// Validate instrument-specific structural and economic invariants.
    ///
    /// The default implementation accepts the instrument. Types with domain
    /// invariants that Serde cannot express (for example positive strikes,
    /// distinct basis indices, or aligned reset schedules) override this
    /// method. JSON, Python, WASM, and direct Rust pricing all call the same
    /// hook through [`Instrument::validate_for_pricing`].
    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        Ok(())
    }

    /// Validate the complete instrument boundary before pricing or exposing a
    /// deserialized payload to a host language.
    fn validate_for_pricing(&self) -> finstack_quant_core::Result<()> {
        self.validate_invariants()?;
        if let Some(overrides) = self.get_instrument_pricing_overrides() {
            overrides.validate()?;
        }
        if let Some(overrides) = self.get_metric_pricing_overrides() {
            overrides.validate()?;
        }
        if let Some(overrides) = self.get_scenario_pricing_overrides() {
            overrides.validate()?;
        }
        Ok(())
    }

    // === Pricing Methods ===

    /// Compute the present value only (fast path, no metrics).
    ///
    /// This is the performance-optimized method for obtaining just the NPV
    /// without computing any risk metrics. Use this in hot paths like
    /// portfolio aggregation where metrics are not needed.
    ///
    /// The returned [`Money`] is the canonical rounded pricing output. Callers
    /// that need pre-rounding arithmetic for sensitivities should use
    /// [`Instrument::value_raw`] rather than inferring precision from `Money`.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data context containing discount curves, forward curves,
    ///   volatility surfaces, and other pricing inputs
    /// * `as_of` - Valuation date (T+0)
    ///
    /// # Returns
    ///
    /// Present value in the instrument's native currency
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Required market curves are missing
    /// - Instrument parameters are invalid
    /// - Numerical computation fails
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// let market = MarketContext::new();
    /// let as_of = create_date(2025, Month::January, 1)?;
    ///
    /// // Fast NPV-only calculation
    /// // let pv = bond.value(&market, as_of)?;
    /// // assert_eq!(pv.currency(), Currency::USD);
    /// # Ok(())
    /// # }
    /// ```
    //
    // # Override Contract
    //
    // - `MarketQuoteOverrides` (e.g. `quoted_clean_price`, `quoted_ytm`,
    //   `implied_volatility`): honored by `base_value` (instrument-specific).
    // - `ModelConfig` (e.g. `tree_steps`, `mc_paths`, `vol_model`): honored by
    //   the pricer implementing `base_value`.
    // - `ScenarioPricingOverrides` (e.g. `scenario_price_shock_pct`): applied
    //   automatically by the default [`Instrument::value`] wrapper; the
    //   `base_value` implementation must not apply scenario shocks itself.
    // - `MetricPricingOverrides` (bump sizes, `theta_period`, deterministic
    //   MC seeds) are out of scope for `base_value`/`value`; they are consumed
    //   by the metrics pipeline.
    fn base_value(&self, market: &MarketContext, as_of: Date)
        -> finstack_quant_core::Result<Money>;

    /// Resolve the effective valuation date used to price and stamp this instrument.
    ///
    /// Most instruments price and stamp their result with the `as_of` requested
    /// by the caller. The default implementation therefore returns `requested`
    /// unchanged.
    ///
    /// A small number of instruments derive their own valuation date from their
    /// own state or market data and intentionally ignore the requested `as_of`
    /// (e.g. a private markets fund anchors valuation to its discount curve's
    /// base date, or to its last event date when undiscounted). Such instruments
    /// override this method; the generic pricer then uses the returned date for
    /// both the [`Instrument::base_value`] computation and the result stamp.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data context (may be consulted to derive the date)
    /// * `requested` - The valuation date requested by the caller
    ///
    /// # Returns
    ///
    /// The effective valuation date to use for pricing and stamping.
    fn resolve_pricing_as_of(&self, _market: &MarketContext, requested: Date) -> Date {
        requested
    }

    /// Optional typed [`ValuationDetails`](crate::results::ValuationDetails)
    /// to attach to this instrument's [`ValuationResult`](crate::results::ValuationResult).
    ///
    /// The generic instrument pricer
    /// ([`GenericInstrumentPricer`](crate::instruments::common_impl::GenericInstrumentPricer))
    /// calls this after computing PV and, when `Some`, attaches the returned
    /// details via [`ValuationResult::with_details`](crate::results::ValuationResult::with_details).
    /// FX-bearing instruments (`FxForward`, `FxOption`, `QuantoOption`) use
    /// this to surface the FX matrix triangulation flag — satisfying the
    /// project-wide FX-policy-visibility invariant — without requiring a
    /// per-instrument bespoke pricer.
    ///
    /// Default implementation returns `None`.
    fn valuation_details(
        &self,
        _market: &MarketContext,
        _as_of: Date,
    ) -> Option<crate::results::ValuationDetails> {
        None
    }

    /// Compute the scenario-adjusted present value.
    ///
    /// Default implementation invokes [`Instrument::base_value`] and applies any
    /// active [`crate::instruments::ScenarioPricingOverrides`] exactly once. Instruments should not
    /// override this method; implement pricing logic in `base_value` instead.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data context
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// Present value with scenario price shock applied (if any).
    fn value(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<Money> {
        self.validate_for_pricing()?;
        let base = self.base_value(market, as_of)?;
        Ok(crate::instruments::common_impl::helpers::apply_scenario_value(self, base))
    }

    /// Compute the base present value as raw f64, before scenario adjustments.
    ///
    /// Default implementation delegates to [`Instrument::base_value`] and extracts
    /// the amount. Override for high-precision pricers that want to avoid
    /// Money rounding in intermediate calculations.
    fn base_value_raw(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate_for_pricing()?;
        Ok(self.base_value(market, as_of)?.amount())
    }

    /// Compute the present value as raw f64 (high precision path for risk calculations).
    ///
    /// This method returns the NPV as an unrounded f64, avoiding the precision loss
    /// that occurs when Money rounds to currency decimal places (e.g., 2 for USD).
    /// This is critical for finite difference sensitivity calculations where small
    /// PV differences matter (e.g., bucketed DV01, key-rate sensitivities).
    ///
    /// # Arguments
    ///
    /// * `market` - Market data context containing discount curves, forward curves, etc.
    /// * `as_of` - Valuation date (T+0)
    ///
    /// # Returns
    ///
    /// Present value as raw f64 without currency rounding
    ///
    /// # Default Implementation
    ///
    /// The default implementation delegates to `value()` and extracts the amount.
    /// Instruments with internal high-precision pricing should override this method
    /// to return the raw value before Money wrapping for better sensitivity accuracy.
    ///
    /// `value_raw()` should represent the same economics as [`Instrument::value`]
    /// before currency rounding, not a different pricing convention.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Used internally by risk calculators for high-precision sensitivities
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::Rate;
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let instrument = Bond::fixed(
    ///     "BOND-001",
    ///     Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0),
    ///     date!(2025-01-15),
    ///     date!(2030-01-15),
    ///     "USD-OIS",
    /// )?;
    /// let market = MarketContext::new();
    /// let bumped_market = MarketContext::new();
    /// let as_of = date!(2025-01-15);
    /// let bump_bp = 1e-4; // 1bp = 0.0001
    ///
    /// let base_pv = instrument.value_raw(&market, as_of)?;
    /// let bumped_pv = instrument.value_raw(&bumped_market, as_of)?;
    /// let dv01 = (bumped_pv - base_pv) / bump_bp;
    /// # let _ = dv01;
    /// # Ok(())
    /// # }
    /// ```
    fn value_raw(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        Ok(self.value(market, as_of)?.amount())
    }

    /// Return the default pricing model for this instrument.
    ///
    /// Most instruments use [`ModelKey::Discounting`]. Instruments whose native
    /// pricing path uses a different model should override this method so the
    /// canonical pricing API preserves existing behavior.
    fn default_model(&self) -> ModelKey {
        ModelKey::Discounting
    }

    /// Compute present value with specified risk metrics.
    ///
    /// This method computes NPV plus any requested risk metrics (duration, DV01,
    /// Greeks, etc.). Metrics are computed on-demand based on the provided list,
    /// enabling efficient calculation of only the required sensitivities.
    ///
    /// This is the canonical pricing entry point for most callers. By default,
    /// it prices with [`Instrument::default_model`]. To force a different
    /// registered pricing path, pass [`PricingOptions::with_model`].
    ///
    /// PV is always returned in [`crate::results::ValuationResult::value`].
    /// Requested metrics are stored in `ValuationResult::measures` and must be
    /// interpreted using their [`crate::metrics::MetricId`] contracts.
    ///
    /// # Arguments
    ///
    /// * `market` - Market data context containing all pricing inputs
    /// * `as_of` - Valuation date (T+0)
    /// * `metrics` - List of metric IDs to compute (e.g., `MetricId::Dv01`)
    /// * `options` - Optional overrides for config, market history, registry,
    ///   or explicit model selection
    ///
    /// # Returns
    ///
    /// `ValuationResult` containing:
    /// - Present value
    /// - Computed risk metrics
    /// - Calculation metadata (timing, precision, etc.)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Required market data is missing
    /// - Metric calculation fails
    /// - Instrument configuration is invalid
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// let market = MarketContext::new();
    /// let as_of = create_date(2025, Month::January, 1)?;
    ///
    /// // Request specific metrics
    /// let metrics_to_compute = vec![
    ///     MetricId::Ytm,
    ///     MetricId::DurationMod,
    ///     MetricId::Dv01,
    /// ];
    ///
    /// let result = bond.price_with_metrics(
    ///     &market,
    ///     as_of,
    ///     &metrics_to_compute,
    ///     PricingOptions::default(),
    /// )?;
    /// println!("NPV: {}", result.value);
    /// println!("DV01: {}", result.measures.get("dv01").expect("should succeed"));
    /// # let _ = result;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::{Bond, Instrument, PricingOptions};
    /// use finstack_quant_valuations::metrics::MetricId;
    /// use finstack_quant_valuations::pricer::ModelKey;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::money::Money;
    /// # use finstack_quant_core::dates::create_date;
    /// # use finstack_quant_core::types::Rate;
    /// # use time::Month;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let issue = create_date(2025, Month::January, 15)?;
    /// # let maturity = create_date(2030, Month::January, 15)?;
    /// let bond = Bond::fixed("BOND-001", Money::new(1_000_000.0, Currency::USD),
    ///     Rate::from_percent(5.0), issue, maturity, "USD-OIS")?;
    ///
    /// let market = MarketContext::new();
    /// let as_of = create_date(2025, Month::January, 1)?;
    ///
    /// let result = bond.price_with_metrics(
    ///     &market,
    ///     as_of,
    ///     &[MetricId::Dv01],
    ///     PricingOptions::default().with_model(ModelKey::HazardRate),
    /// )?;
    /// # let _ = result;
    /// # Ok(())
    /// # }
    /// ```
    /// Compute present value with specified risk metrics.
    ///
    /// This is the canonical user-facing pricing entry point. Pass `&[]` for
    /// PV only; pass requested metric IDs to get DV01, duration, YTM, etc.
    /// alongside the PV.
    fn price_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> finstack_quant_core::Result<crate::results::ValuationResult> {
        let PricingOptions {
            config,
            market_history,
            model,
            registry,
        } = options;
        let model = model.unwrap_or_else(|| self.default_model());
        let registry = registry.unwrap_or_else(shared_standard_registry);
        let instrument = self.clone_box();
        let registry_options = PricingOptions {
            config,
            market_history,
            model: None,
            registry: None,
        };

        PricerRegistry::price_with_metrics_shared(
            &registry,
            instrument.as_ref(),
            model,
            market,
            as_of,
            metrics,
            registry_options,
        )
        .map_err(|e| match e {
            PricingError::UnknownPricer {
                key,
                available_models,
            } => actionable_unknown_pricer_message(key, &available_models)
                .map(finstack_quant_core::Error::Validation)
                .unwrap_or_else(|| {
                    PricingError::UnknownPricer {
                        key,
                        available_models,
                    }
                    .into()
                }),
            other => other.into(),
        })
    }

    // === Market Data Introspection (for Attribution) ===

    /// Unified market data dependencies for this instrument.
    ///
    /// This is the canonical dependency surface and should be overridden by
    /// all instruments to declare their market data needs.
    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        Ok(MarketDependencies::new())
    }

    /// FX exposure for this instrument.
    ///
    /// Returns the currency pair if this instrument has FX exposure.
    /// Used by FX attribution to measure spot rate changes.
    ///
    /// Default implementation returns `None`.
    ///
    /// # Returns
    ///
    /// `Some((base, quote))` if FX-sensitive, `None` otherwise
    ///
    /// # Examples
    ///
    /// FX Forward would return `Some((USD, EUR))` for a USD/EUR forward.
    fn fx_exposure(&self) -> Option<(Currency, Currency)> {
        None
    }

    /// Dividend schedule ID for this instrument.
    ///
    /// Returns the dividend schedule ID if this instrument depends on dividends.
    /// Used by dividend attribution.
    ///
    /// Default implementation returns `None`.
    ///
    /// # Returns
    ///
    /// `Some(schedule_id)` if dividend-sensitive, `None` otherwise
    fn dividend_schedule_id(&self) -> Option<CurveId> {
        None
    }

    /// Funding curve ID for this instrument.
    ///
    /// Returns the funding or repo curve used to finance the position for
    /// carry cost calculations.
    ///
    /// Default implementation returns `None`.
    fn funding_curve_id(&self) -> Option<CurveId> {
        None
    }

    /// Convert this instrument to its JSON representation for serialization.
    ///
    /// This method enables serialization of instruments by converting them to
    /// the `InstrumentJson` tagged union. Instruments that support serialization
    /// should override this method to return `Some(instrument_json)`.
    ///
    /// Default implementation returns `None`, indicating that serialization
    /// is not supported for this instrument type.
    ///
    /// # Returns
    ///
    /// `Some(InstrumentJson)` if conversion is supported, `None` otherwise
    fn to_instrument_json(&self) -> Option<crate::instruments::InstrumentJson> {
        None
    }

    /// Get the instrument's expiry or maturity date, if applicable.
    ///
    /// Returns the date at which the instrument expires, matures, or otherwise
    /// terminates. This is used by theta calculations to cap the roll date
    /// and by other time-dependent calculations.
    ///
    /// # Returns
    ///
    /// - `Some(Date)` for instruments with a defined expiry/maturity
    /// - `None` for instruments without a clear expiry (e.g., equity spot positions)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::Instrument;
    ///
    /// let bond = Bond::example().unwrap();
    /// if let Some(maturity) = bond.expiry() {
    ///     println!("Bond matures on: {}", maturity);
    /// }
    /// ```
    fn expiry(&self) -> Option<Date> {
        None
    }

    /// Get the instrument's effective start/value date, if applicable.
    ///
    /// Returns the date at which the instrument's economics begin (e.g., accrual start,
    /// effective date, or issue date). This is used by shared metrics such as DfStart
    /// and year-fraction calculations.
    ///
    /// # Returns
    ///
    /// - `Some(Date)` for instruments with a defined effective start/value date
    /// - `None` for instruments without a clear start (e.g., equity spot positions)
    fn effective_start_date(&self) -> Option<Date> {
        None
    }
}

// Note: Methods formerly on the `Attributable` trait are now default methods on `Instrument`.
