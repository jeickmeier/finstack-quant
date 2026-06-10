//! Build context for quote-to-instrument construction.

use finstack_core::dates::Date;
use finstack_core::{Error, HashMap, InputError, Result};

use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
use crate::instruments::rates::irs::FloatingLegCompounding;

/// Context for building instruments from market quotes.
///
/// Provides the necessary environment (valuation date, notional, curve mappings) to construct
/// a concrete instrument instance that matches the quote. The context is used by all builders
/// to resolve dates, configure instruments, and map curve identifiers.
///
/// # Invariants
///
/// - `as_of` is the valuation date for all instruments built with this context
/// - `notional` is used consistently across all instruments unless overridden
/// - `curve_ids` maps role names (e.g., "discount", "forward", "credit") to curve identifiers
///
/// # Curve Role Conventions
///
/// Each builder requires or optionally uses specific curve roles from `curve_ids`:
///
/// | Builder | Required roles | Optional (fallback) |
/// |---------|---------------|---------------------|
/// | **Rates** | `"discount"`, `"forward"` | *(none -- errors if missing)* |
/// | **CDS** | `"discount"`, `"credit"` | *(none -- errors if missing)* |
/// | **XCCY** | *(none at context level)* | `"domestic_discount"`, `"foreign_discount"`, `"domestic_forward"`, `"foreign_forward"` (convention-derived defaults) |
/// | **CDS Tranche** | `"discount"`, `"credit"` | *(none -- errors if missing)* |
///
/// This table covers the public quote-to-instrument builders re-exported from
/// `crate::market`. Internal builders may use additional roles.
///
/// # Examples
///
/// Basic usage:
/// ```rust
/// use finstack_valuations::market::BuildCtx;
/// use finstack_core::dates::Date;
/// use finstack_core::HashMap;
///
/// let as_of = Date::from_calendar_date(2024, time::Month::January, 2).unwrap();
/// let mut curve_ids = HashMap::default();
/// curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
/// curve_ids.insert("forward".to_string(), "USD-SOFR".to_string());
///
/// let ctx = BuildCtx::new(as_of, 1_000_000.0, curve_ids);
/// assert_eq!(ctx.curve_id("discount"), Some("USD-OIS"));
/// ```
///
/// Minimal context with no curve roles:
/// ```rust
/// use finstack_valuations::market::BuildCtx;
/// use finstack_core::dates::Date;
/// use finstack_core::HashMap;
///
/// let ctx = BuildCtx::new(
///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
///     1_000_000.0,
///     HashMap::default(), // Builders that require roles will fail closed.
/// );
/// ```
#[derive(Debug, Clone)]
pub struct BuildCtx {
    /// The valuation date for which the instrument is being built.
    ///
    /// This date is used to calculate spot dates, fixing dates, and all other
    /// date-dependent parameters in the constructed instrument.
    as_of: Date,
    /// The notional amount to use for the instrument.
    ///
    /// Calibration typically uses a standard notional (e.g., 1M or 10k) but this allows override.
    /// The notional is applied consistently across all instruments built with this context.
    notional: f64,
    /// Mapping of curve roles to curve IDs.
    ///
    /// Common roles include:
    /// - `"discount"`: Discount curve for present value calculations
    /// - `"forward"`: Forward curve for floating rate projections
    /// - `"credit"`: Credit curve for CDS instruments
    ///
    /// Missing-role behavior is builder-specific:
    /// - rates, CDS, and CDS tranche builders error when required roles are absent
    /// - XCCY can derive curve IDs from conventions or currencies when roles are absent
    ///
    /// Do not assume that a missing role always falls back to a currency-based ID.
    /// Consult the builder-specific docs when the distinction matters.
    curve_ids: HashMap<String, String>,
    /// Optional CDS valuation convention for instruments built from CDS quotes.
    cds_valuation_convention: Option<CdsValuationConvention>,
    /// Optional override for the OIS floating-leg compounding mode used when
    /// building swap instruments from `RateQuote::Swap` quotes whose index is
    /// classified as `OvernightRfr`.
    ///
    /// When `None`, [`crate::market::build::rates::build_swap`] uses the
    /// per-index registry default (e.g. `CompoundedInArrears { lookback_days: 0 }`
    /// for SOFR, the cleared OIS plain in-arrears convention). Set this from a
    /// calibration step to
    /// match a different vendor convention (e.g. Bloomberg SWPM SOFR uses
    /// `CompoundedWithRateCutoff { cutoff_days: 1 }`).
    ois_compounding_override: Option<FloatingLegCompounding>,
}

impl BuildCtx {
    /// Create a new build context.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date for all instruments built with this context
    /// * `notional` - Standard notional amount to use for instruments
    /// * `curve_ids` - Mapping of curve roles to curve identifiers
    ///
    /// # Returns
    ///
    /// A new `BuildCtx`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::market::BuildCtx;
    /// use finstack_core::dates::Date;
    /// use finstack_core::HashMap;
    ///
    /// let ctx = BuildCtx::new(
    ///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
    ///     1_000_000.0,
    ///     HashMap::default(),
    /// );
    /// ```
    pub fn new(as_of: Date, notional: f64, curve_ids: HashMap<String, String>) -> Self {
        Self {
            as_of,
            notional,
            curve_ids,
            cds_valuation_convention: None,
            ois_compounding_override: None,
        }
    }

    /// Return a copy with a CDS valuation convention applied to CDS quote builds.
    #[must_use]
    pub(crate) fn with_cds_valuation_convention(
        mut self,
        convention: Option<CdsValuationConvention>,
    ) -> Self {
        self.cds_valuation_convention = convention;
        self
    }

    /// Optional CDS valuation convention for CDS quote-built instruments.
    pub(crate) fn cds_valuation_convention(&self) -> Option<CdsValuationConvention> {
        self.cds_valuation_convention
    }

    /// Return a copy with an OIS compounding override for swap quote builds.
    ///
    /// When set, [`crate::market::build::rates::build_swap`] uses this
    /// compounding mode for `OvernightRfr`-classified indices instead of
    /// the per-index registry default. Pass-through (`None`) preserves
    /// existing behavior.
    #[must_use]
    pub(crate) fn with_ois_compounding_override(
        mut self,
        override_compounding: Option<FloatingLegCompounding>,
    ) -> Self {
        self.ois_compounding_override = override_compounding;
        self
    }

    /// Optional OIS compounding override for swap-quote instrument construction.
    pub(crate) fn ois_compounding_override(&self) -> Option<&FloatingLegCompounding> {
        self.ois_compounding_override.as_ref()
    }

    /// Valuation date for instruments built with this context.
    pub fn as_of(&self) -> Date {
        self.as_of
    }

    /// Default notional applied during instrument construction.
    pub fn notional(&self) -> f64 {
        self.notional
    }

    /// Get a curve ID by role name.
    ///
    /// # Arguments
    ///
    /// * `role` - The curve role name (e.g., "discount", "forward", "credit")
    ///
    /// # Returns
    ///
    /// `Some(curve_id)` if the role is mapped, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_valuations::market::BuildCtx;
    /// use finstack_core::dates::Date;
    /// use finstack_core::HashMap;
    ///
    /// let mut curve_ids = HashMap::default();
    /// curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    ///
    /// let ctx = BuildCtx::new(
    ///     Date::from_calendar_date(2024, time::Month::January, 2).unwrap(),
    ///     1_000_000.0,
    ///     curve_ids,
    /// );
    ///
    /// assert_eq!(ctx.curve_id("discount"), Some("USD-OIS"));
    /// assert_eq!(ctx.curve_id("forward"), None);
    /// ```
    pub fn curve_id(&self, role: &str) -> Option<&str> {
        self.curve_ids.get(role).map(|s| s.as_str())
    }

    /// Get a required curve ID by role name.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::NotFound`] when the role is not mapped in this context.
    pub fn require_curve_id(&self, role: &str) -> Result<&str> {
        self.curve_id(role).ok_or_else(|| {
            Error::Input(InputError::NotFound {
                id: format!("curve role '{}'", role),
            })
        })
    }
}
