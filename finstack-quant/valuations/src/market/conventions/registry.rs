//! Global registry for market conventions.

use super::defs::{
    CdsConvention, CdsConventionSpec, InflationSwapConventions, IrFutureConventions,
    RateIndexConventions, SwaptionConventions, XccyConventions,
};
use super::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId, IrFutureContractId,
    SwaptionConventionId, XccyConventionId,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use finstack_quant_core::{Error, Result};
use std::sync::OnceLock;

/// Global registry of market conventions.
///
/// This registry provides a single source of truth for convention lookups, ensuring strict
/// handling of missing data. Conventions are loaded from embedded JSON data on first access
/// and cached for the lifetime of the program.
///
/// # Thread Safety
///
/// The registry is thread-safe and can be accessed concurrently from multiple threads.
/// The singleton is initialized lazily on first access.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::market::conventions::ConventionRegistry;
/// use finstack_quant_core::types::IndexId;
///
/// let registry = ConventionRegistry::try_global()?;
/// let conv = registry.require_rate_index(&IndexId::new("USD-SOFR-OIS"))?;
/// assert_eq!(conv.currency, finstack_quant_core::currency::Currency::USD);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct ConventionRegistry {
    /// Registry of Rate Index conventions.
    rate_index: HashMap<IndexId, RateIndexConventions>,
    /// Registry of CDS conventions keyed by currency and documentation clause.
    cds: HashMap<CdsConventionKey, CdsConventionSpec>,
    /// Loader-only `ANY` fallback rows keyed by regional family.
    cds_any: HashMap<CdsConvention, CdsConventionSpec>,
    /// Explicit regional family for currencies listed in `cds_conventions.json`.
    cds_primary: HashMap<Currency, CdsConvention>,
    /// Registry of Swaption conventions.
    swaption: HashMap<SwaptionConventionId, SwaptionConventions>,
    /// Registry of Inflation Swap conventions.
    inflation_swap: HashMap<InflationSwapConventionId, InflationSwapConventions>,
    /// Registry of Interest Rate Futures conventions.
    ir_future: HashMap<IrFutureContractId, IrFutureConventions>,
    /// Registry of cross-currency swap conventions.
    xccy: HashMap<XccyConventionId, XccyConventions>,
}

impl ConventionRegistry {
    fn not_found(id: impl Into<String>) -> Error {
        finstack_quant_core::InputError::NotFound { id: id.into() }.into()
    }

    /// Access the global singleton registry.
    ///
    /// Initialized with embedded JSON data on the first call. The registry is loaded
    /// from embedded JSON files in `data/conventions/` and cached for the lifetime
    /// of the program.
    ///
    /// # Errors
    ///
    /// Returns an error if convention data cannot be loaded (e.g., corrupted embedded
    /// JSON). Prefer this fallible API over panicking in production.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::market::conventions::ConventionRegistry;
    ///
    /// # fn example() -> finstack_quant_core::Result<()> {
    /// let registry = ConventionRegistry::try_global()?;
    /// // Registry is now initialized and ready to use
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_global() -> Result<&'static Self> {
        static REGISTRY: OnceLock<ConventionRegistry> = OnceLock::new();
        if let Some(reg) = REGISTRY.get() {
            return Ok(reg);
        }

        tracing::debug!("initializing ConventionRegistry from embedded JSON");
        let cds_tables = super::loaders::cds::load_registry()?;
        let built = ConventionRegistry {
            rate_index: super::loaders::rate_index::load_registry()?,
            cds: cds_tables.entries,
            cds_any: cds_tables.any,
            cds_primary: cds_tables.primary_family,
            swaption: super::loaders::swaption::load_registry()?,
            inflation_swap: super::loaders::inflation_swap::load_registry()?,
            ir_future: super::loaders::ir_future::load_registry()?,
            xccy: super::loaders::xccy::load_registry()?,
        };
        let _ = REGISTRY.set(built);

        REGISTRY.get().ok_or_else(|| {
            Error::Validation("ConventionRegistry::try_global failed to initialize".to_string())
        })
    }

    /// Resolve conventions for a Rate Index.
    ///
    /// # Arguments
    ///
    /// * `id` - The rate index identifier
    ///
    /// # Returns
    ///
    /// `Ok(&RateIndexConventions)` if found, or `Err` with an `InputError::NotFound` if missing.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NotFound` if the index is not found in the registry.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::market::conventions::ConventionRegistry;
    /// use finstack_quant_core::types::IndexId;
    ///
    /// let registry = ConventionRegistry::try_global()?;
    /// let conv = registry.require_rate_index(&IndexId::new("USD-SOFR-OIS"))?;
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn require_rate_index(&self, id: &IndexId) -> Result<&RateIndexConventions> {
        self.rate_index
            .get(id)
            .ok_or_else(|| Self::not_found(id.to_string()))
    }

    /// Resolve CDS schedule conventions for a currency and documentation clause.
    ///
    /// Meta clauses map to their regional family (`Au`/`Nz` use Asia). Exact
    /// restructuring clauses stay on the instrument; the schedule family then
    /// comes from the currency's explicit registry row, falling back to the
    /// loader-only `ANY:isda_na` family when the currency has no explicit row.
    ///
    /// # Arguments
    ///
    /// * `key` - Currency plus documentation clause from the quote or instrument
    ///
    /// # Errors
    ///
    /// Returns `InputError::NotFound` when neither an explicit currency row nor
    /// an `ANY` family fallback can be resolved.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_valuations::market::conventions::ConventionRegistry;
    /// use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let registry = ConventionRegistry::try_global()?;
    /// let key = CdsConventionKey {
    ///     currency: Currency::USD,
    ///     doc_clause: CdsDocClause::Cr14,
    /// };
    /// let conv = registry.resolve_cds(&key)?;
    /// assert_eq!(conv.family.to_string(), "isda_na");
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn resolve_cds(&self, key: &CdsConventionKey) -> Result<&CdsConventionSpec> {
        let schedule_key = self.cds_schedule_key(key);
        if let Some(spec) = self.cds.get(&schedule_key) {
            return Ok(spec);
        }
        let family = CdsConvention::family_from_doc_clause(schedule_key.doc_clause)
            .unwrap_or(CdsConvention::IsdaNa);
        self.cds_any
            .get(&family)
            .ok_or_else(|| Self::not_found(key.to_string()))
    }

    /// Explicit regional family for `currency`, if the JSON lists one.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 currency whose explicit `cds_conventions.json`
    ///   regional row is returned. Currencies present only via `ANY` expansion
    ///   yield `None`.
    #[must_use]
    pub fn primary_cds_family(&self, currency: Currency) -> Option<CdsConvention> {
        self.cds_primary.get(&currency).copied()
    }

    fn cds_schedule_key(&self, key: &CdsConventionKey) -> CdsConventionKey {
        let doc_clause = match key.doc_clause {
            CdsDocClause::IsdaAu | CdsDocClause::IsdaNz => CdsDocClause::IsdaAs,
            CdsDocClause::Cr14 | CdsDocClause::Mr14 | CdsDocClause::Mm14 | CdsDocClause::Xr14 => {
                self.cds_primary
                    .get(&key.currency)
                    .copied()
                    .unwrap_or(CdsConvention::IsdaNa)
                    .as_doc_clause()
            }
            other => other,
        };
        CdsConventionKey {
            currency: key.currency,
            doc_clause,
        }
    }

    /// Resolve conventions for a Swaption.
    ///
    /// # Arguments
    ///
    /// * `id` - The swaption convention identifier
    ///
    /// # Returns
    ///
    /// `Ok(&SwaptionConventions)` if found, or `Err` with an `InputError::NotFound` if missing.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NotFound` if the ID is not found in the registry.
    pub fn require_swaption(&self, id: &SwaptionConventionId) -> Result<&SwaptionConventions> {
        self.swaption
            .get(id)
            .ok_or_else(|| Self::not_found(id.to_string()))
    }

    /// Resolve conventions for an Inflation Swap.
    ///
    /// # Arguments
    ///
    /// * `id` - The inflation swap convention identifier
    ///
    /// # Returns
    ///
    /// `Ok(&InflationSwapConventions)` if found, or `Err` with an `InputError::NotFound` if missing.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NotFound` if the ID is not found in the registry.
    pub fn require_inflation_swap(
        &self,
        id: &InflationSwapConventionId,
    ) -> Result<&InflationSwapConventions> {
        self.inflation_swap
            .get(id)
            .ok_or_else(|| Self::not_found(id.to_string()))
    }

    /// Resolve conventions for an Interest Rate Future contract.
    ///
    /// # Arguments
    ///
    /// * `id` - The IR future contract identifier
    ///
    /// # Returns
    ///
    /// `Ok(&IrFutureConventions)` if found, or `Err` with an `InputError::NotFound` if missing.
    ///
    /// # Errors
    ///
    /// Returns `InputError::NotFound` if the ID is not found in the registry.
    pub fn require_ir_future(&self, id: &IrFutureContractId) -> Result<&IrFutureConventions> {
        self.ir_future
            .get(id)
            .ok_or_else(|| Self::not_found(id.to_string()))
    }

    /// Resolve conventions for a cross-currency swap pair.
    pub fn require_xccy(&self, id: &XccyConventionId) -> Result<&XccyConventions> {
        self.xccy
            .get(id)
            .ok_or_else(|| Self::not_found(id.to_string()))
    }
}

impl CdsConvention {
    /// Resolve the regional CDS conventions from the embedded registry.
    ///
    /// # Errors
    /// Propagates registry initialization failures and returns NotFound when
    /// this family has no fallback entry.
    pub fn get_spec(self) -> Result<&'static CdsConventionSpec> {
        ConventionRegistry::try_global()?
            .cds_any
            .get(&self)
            .ok_or_else(|| ConventionRegistry::not_found(self.to_string()))
    }

    /// Detect the appropriate CDS convention based on the currency's registry family.
    ///
    /// # Arguments
    ///
    /// * `currency` - ISO-4217 currency whose explicit CDS registry entry selects
    ///   the regional family. Currencies without an explicit regional row fall
    ///   back to North America.
    ///
    /// # Errors
    /// Propagates failures to initialize the embedded registry.
    pub fn detect_from_currency(currency: Currency) -> Result<Self> {
        Ok(ConventionRegistry::try_global()?
            .primary_cds_family(currency)
            .unwrap_or(Self::IsdaNa))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::StubKind;

    #[test]
    fn resolve_cds_maps_meta_and_exact_clauses() {
        let registry = ConventionRegistry::try_global().expect("registry");

        let usd_cr14 = registry
            .resolve_cds(&CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::Cr14,
            })
            .expect("USD CR14");
        assert_eq!(usd_cr14.family, CdsConvention::IsdaNa);
        assert_eq!(usd_cr14.calendar_id, "nyse");
        assert_eq!(usd_cr14.settlement_days, 3);
        assert_eq!(usd_cr14.stub, StubKind::ShortFront);

        let eur_mm14 = registry
            .resolve_cds(&CdsConventionKey {
                currency: Currency::EUR,
                doc_clause: CdsDocClause::Mm14,
            })
            .expect("EUR MM14");
        assert_eq!(eur_mm14.family, CdsConvention::IsdaEu);
        assert_eq!(eur_mm14.calendar_id, "target2");
        assert_eq!(eur_mm14.settlement_days, 1);

        let aud_au = registry
            .resolve_cds(&CdsConventionKey {
                currency: Currency::AUD,
                doc_clause: CdsDocClause::IsdaAu,
            })
            .expect("AUD IsdaAu");
        assert_eq!(aud_au.family, CdsConvention::IsdaAs);
        assert_eq!(aud_au.calendar_id, "auce");

        let nzd_nz = registry
            .resolve_cds(&CdsConventionKey {
                currency: Currency::NZD,
                doc_clause: CdsDocClause::IsdaNz,
            })
            .expect("NZD IsdaNz");
        assert_eq!(nzd_nz.family, CdsConvention::IsdaAs);
        assert_eq!(nzd_nz.calendar_id, "nzau");

        let brl_cr14 = registry
            .resolve_cds(&CdsConventionKey {
                currency: Currency::BRL,
                doc_clause: CdsDocClause::Cr14,
            })
            .expect("BRL CR14 falls back to ANY NA");
        assert_eq!(brl_cr14.family, CdsConvention::IsdaNa);
        assert_eq!(brl_cr14.calendar_id, "nyse");
    }
}
