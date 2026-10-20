//! Instrument families: the registration unit of the valuations stack.
//!
//! A [`Family`] names one group of instruments and owns the two registration
//! tables that make them priceable: the pricers for each
//! `(InstrumentType, ModelKey)` pair and the metric calculators that apply to
//! them. Each built-in family is a `static` defined next to its instruments
//! (`instruments::rates::RATES`, `instruments::fx::FX`, ...), and every
//! built-in instrument names its family through
//! [`Instrument::family`](crate::instruments::Instrument::family).
//!
//! Pricing with no explicit registry resolves through the instrument's
//! family, so a family is self-contained: its own registries price its own
//! instruments and compute their metrics. The umbrella family
//! `STANDARD` (umbrella crate) merges every family for closed-world
//! callers: the JSON entry points, registry introspection, and the composite
//! instruments that can hold any leg.
//!
//! # Quick Example
//!
//! A family is a `static` built from its two registration functions. The
//! registries are built on first use and shared afterwards; the universal
//! metric calculators are part of every family's metric registry.
//!
//! ```rust
//! use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry, MetricRegistryError};
//! use finstack_quant_valuations_core::pricer::{
//!     Family, InstrumentType, ModelKey, PricerKey, PricerRegistry, PricingError,
//! };
//!
//! fn no_pricers(_: &mut PricerRegistry) -> Result<(), PricingError> {
//!     Ok(())
//! }
//! fn no_metrics(_: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
//!     Ok(())
//! }
//!
//! static EMPTY: Family = Family::new("empty", no_pricers, no_metrics);
//!
//! assert_eq!(EMPTY.name(), "empty");
//! assert!(EMPTY
//!     .pricers()
//!     .get_pricer(PricerKey::new(InstrumentType::Irs, ModelKey::Discounting))
//!     .is_none());
//! // Universal metrics (theta, carry) come with every family.
//! assert!(EMPTY.metrics().has_metric(MetricId::Theta));
//! ```

use std::sync::{Arc, OnceLock};

use super::{PricerRegistry, PricingError};
use crate::metrics::{register_universal_metrics, MetricRegistry, MetricRegistryError};

/// Registration function adding a family's pricers to a registry.
pub type RegisterPricers = fn(&mut PricerRegistry) -> Result<(), PricingError>;

/// Registration function adding a family's metric calculators to a registry.
pub type RegisterMetrics = fn(&mut MetricRegistry) -> Result<(), MetricRegistryError>;

/// A named group of instruments with the pricers and metrics that serve them.
///
/// Families are `static` values: the registries they build are initialised
/// once, on first use, and shared for the life of the process. Instruments
/// outside the built-in families belong to [`Family::unregistered`], which
/// registers no pricers and only the universal metrics.
///
/// # Invariants
/// - Every `(InstrumentType, ModelKey)` pricer is registered by exactly one
///   family; the umbrella merges families without duplicates.
/// - A family's metric registry contains the universal metrics plus the
///   family's own calculators, so `metrics_for_instrument` answers the same
///   for an owned type as the merged umbrella registry does.
pub struct Family {
    name: &'static str,
    pricer_table: RegisterPricers,
    metric_table: RegisterMetrics,
    registries: OnceLock<FamilyRegistries>,
}

struct FamilyRegistries {
    pricers: Arc<PricerRegistry>,
    metrics: Arc<MetricRegistry>,
}

static UNREGISTERED: Family = Family::new("unregistered", |_| Ok(()), |_| Ok(()));

impl Family {
    /// Declare a family from its registration tables.
    ///
    /// # Arguments
    ///
    /// * `name` - Stable snake_case family name used in diagnostics and
    ///   introspection (for example `"rates"`).
    /// * `pricer_table` - Adds every pricer the family owns to a registry. It
    ///   must not register an instrument type another family owns.
    /// * `metric_table` - Adds the family's metric calculators to a registry.
    ///   The universal metrics are added by the family itself; do not
    ///   register them here.
    pub const fn new(
        name: &'static str,
        pricer_table: RegisterPricers,
        metric_table: RegisterMetrics,
    ) -> Self {
        Self {
            name,
            pricer_table,
            metric_table,
            registries: OnceLock::new(),
        }
    }

    /// The family of instruments that are not registered with any built-in
    /// family.
    ///
    /// Its pricer registry is empty and its metric registry holds only the
    /// universal metrics, so such an instrument is priced through its own
    /// `value` or through a registry the caller supplies in
    /// [`PricingOptions::registry`](crate::instruments::PricingOptions::registry).
    pub fn unregistered() -> &'static Family {
        &UNREGISTERED
    }

    /// Stable snake_case family name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Add this family's pricers to `registry`.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry receiving the pricers; an already registered
    ///   `(InstrumentType, ModelKey)` key fails with
    ///   [`PricingError::DuplicateRegistration`].
    pub fn register_pricers(&self, registry: &mut PricerRegistry) -> Result<(), PricingError> {
        (self.pricer_table)(registry)
    }

    /// Add this family's metric calculators to `registry`.
    ///
    /// The universal metrics are not part of the family table; callers
    /// assembling a registry by hand add them with
    /// [`Family::build_metrics`] or the umbrella's standard registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry receiving the calculators; a metric slot that
    ///   already has an owner for one of the family's instrument types fails
    ///   with [`MetricRegistryError::DuplicateRegistration`].
    pub fn register_metrics(
        &self,
        registry: &mut MetricRegistry,
    ) -> Result<(), MetricRegistryError> {
        (self.metric_table)(registry)
    }

    /// Build a fresh pricer registry holding only this family's pricers.
    ///
    /// # Errors
    ///
    /// Returns the registry error when the family table registers a key
    /// twice.
    pub fn build_pricers(&self) -> Result<PricerRegistry, PricingError> {
        let mut registry = PricerRegistry::new();
        self.register_pricers(&mut registry)?;
        Ok(registry)
    }

    /// Build a fresh metric registry: the universal metrics plus this
    /// family's calculators.
    ///
    /// # Errors
    ///
    /// Returns the registry error when a metric slot has two owners.
    pub fn build_metrics(&self) -> Result<MetricRegistry, MetricRegistryError> {
        let mut registry = MetricRegistry::new();
        register_universal_metrics(&mut registry)?;
        self.register_metrics(&mut registry)?;
        Ok(registry)
    }

    #[allow(clippy::expect_used)]
    fn registries(&'static self) -> &'static FamilyRegistries {
        self.registries.get_or_init(|| FamilyRegistries {
            pricers: Arc::new(
                self.build_pricers()
                    .expect("built-in pricer registrations must be valid"),
            ),
            metrics: Arc::new(
                self.build_metrics()
                    .expect("built-in metric registrations must be valid"),
            ),
        })
    }

    /// The family's shared pricer registry, built on first use.
    pub fn pricers(&'static self) -> &'static PricerRegistry {
        self.registries().pricers.as_ref()
    }

    /// The family's shared pricer registry as an `Arc` for pricing options
    /// and dispatch handles.
    pub fn shared_pricers(&'static self) -> Arc<PricerRegistry> {
        Arc::clone(&self.registries().pricers)
    }

    /// The family's shared metric registry, built on first use.
    pub fn metrics(&'static self) -> &'static MetricRegistry {
        self.registries().metrics.as_ref()
    }
}

impl std::fmt::Debug for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Family").field("name", &self.name).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricId;

    #[test]
    fn unregistered_family_has_no_pricers_and_only_universal_metrics() {
        let family = Family::unregistered();
        assert_eq!(family.name(), "unregistered");
        assert!(family.pricers().all_models_grouped().is_empty());
        let metrics = family.metrics();
        assert!(metrics.available_metrics().contains(&MetricId::Theta));
        assert!(!metrics.available_metrics().contains(&MetricId::Ytm));
    }

    #[test]
    fn family_registries_are_shared_singletons() {
        let family = Family::unregistered();
        assert!(std::ptr::eq(family.pricers(), family.pricers()));
        assert!(Arc::ptr_eq(
            &family.shared_pricers(),
            &family.shared_pricers()
        ));
        assert!(std::ptr::eq(family.metrics(), family.metrics()));
    }
}
