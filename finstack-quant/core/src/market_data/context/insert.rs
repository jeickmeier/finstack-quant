//! Market-data insertion and mutation APIs.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::market_data::dividends::DividendSchedule;
use crate::market_data::scalars::{InflationIndex, MarketScalar, ScalarTimeSeries};
use crate::market_data::surfaces::{FxDeltaVolSurface, VolCube, VolSurface};
use crate::market_data::term_structures::CreditIndexData;
use crate::money::fx::FxMatrix;
use crate::types::CurveId;

use super::{CreditIndexState, CurveStorage, MarketContext};

impl MarketContext {
    /// Insert a generic curve storage entry, failing atomically if validation or credit-index rebinding fails.
    ///
    /// # Arguments
    ///
    /// * `curve` - Any curve type convertible into [`CurveStorage`], keyed by its own id;
    ///   replaces an existing curve of the same curve type and atomically rebinds dependent credit indices.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if the curve id is empty/blank, attempts to
    /// replace a curve of a different type, or invalidates a bound credit index.
    pub fn try_insert<C>(mut self, curve: C) -> crate::Result<Self>
    where
        C: Into<CurveStorage>,
    {
        self.try_insert_mut(curve)?;
        Ok(self)
    }

    /// Insert a volatility surface.
    ///
    /// Accepts either an owned [`VolSurface`] or an `Arc<VolSurface>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling surface sharing between contexts).
    /// Volatility identifiers are namespace-local to `surfaces`.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::{
    /// #     VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    /// # };
    /// # use std::sync::Arc;
    /// # let surface = VolSurface::builder("IR-Swaption")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .secondary_values(&[90.0, 100.0])
    /// #     .secondary_axis(VolSurfaceAxis::Strike)
    /// #     .quote_type(VolQuoteType::BlackLognormal)
    /// #     .interpolation_mode(VolInterpolationMode::Vol)
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// // Owned value (wrapped in Arc automatically)
    /// let ctx = MarketContext::new().insert_surface(surface);
    /// assert_eq!(ctx.stats().surface_count, 1);
    ///
    /// // Pre-wrapped Arc (for sharing across contexts)
    /// # let surface2 = VolSurface::builder("EQ-Vol")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .secondary_values(&[90.0, 100.0])
    /// #     .secondary_axis(VolSurfaceAxis::Strike)
    /// #     .quote_type(VolQuoteType::BlackLognormal)
    /// #     .interpolation_mode(VolInterpolationMode::Vol)
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// let shared = Arc::new(surface2);
    /// let ctx2 = MarketContext::new().insert_surface(Arc::clone(&shared));
    /// ```
    ///
    /// # Arguments
    ///
    /// * `surface` - Owned or shared volatility surface stored under `surface.id()`.
    pub fn insert_surface(mut self, surface: impl Into<Arc<VolSurface>>) -> Self {
        self.insert_surface_mut(surface);
        self
    }

    /// Insert an FX delta-quoted volatility surface.
    ///
    /// Accepts either an owned [`FxDeltaVolSurface`] or an `Arc<FxDeltaVolSurface>`.
    /// Volatility identifiers are namespace-local to `fx_delta_vol_surfaces`.
    ///
    /// # Arguments
    ///
    /// * `surface` - Owned or shared FX delta-vol surface stored under `surface.id()`.
    pub fn insert_fx_delta_vol_surface(
        mut self,
        surface: impl Into<Arc<FxDeltaVolSurface>>,
    ) -> Self {
        self.insert_fx_delta_vol_surface_mut(surface);
        self
    }

    /// Insert a SABR volatility cube.
    ///
    /// Accepts either an owned [`VolCube`] or an `Arc<VolCube>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling cube sharing between contexts).
    /// Volatility identifiers are namespace-local to `vol_cubes`.
    ///
    /// # Arguments
    ///
    /// * `cube` - Owned or shared SABR vol cube stored under `cube.id()`.
    pub fn insert_vol_cube(mut self, cube: impl Into<Arc<VolCube>>) -> Self {
        self.insert_vol_cube_mut(cube);
        self
    }

    /// Insert a dividend schedule.
    ///
    /// Accepts either an owned [`DividendSchedule`] or an `Arc<DividendSchedule>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling schedule sharing between contexts).
    ///
    /// # Arguments
    ///
    /// * `schedule` - Owned or shared dividend schedule stored under `schedule.get_id()`.
    pub fn insert_dividends(mut self, schedule: impl Into<Arc<DividendSchedule>>) -> Self {
        self.insert_dividends_mut(schedule);
        self
    }

    /// Insert a market scalar/price.
    ///
    /// # Arguments
    ///
    /// * `id` - Lookup key stored as [`CurveId`].
    /// * `price` - Scalar observable such as a spot price, spread, or unitless parameter.
    pub fn insert_price(mut self, id: impl AsRef<str>, price: MarketScalar) -> Self {
        self.insert_price_mut(id, price);
        self
    }

    /// Insert a scalar time series.
    ///
    /// # Arguments
    ///
    /// * `series` - Dated scalar series stored under `series.id()`.
    pub fn insert_series(mut self, series: ScalarTimeSeries) -> Self {
        self.insert_series_mut(series);
        self
    }

    /// Insert an inflation index.
    ///
    /// Accepts either an owned [`InflationIndex`] or an `Arc<InflationIndex>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling index sharing between contexts).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::{InflationIndex, InflationObservation};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// let observations = vec![
    ///     InflationObservation {
    ///         reference_month: Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"),
    ///         value: 100.0,
    ///         available_on: Date::from_calendar_date(2024, Month::February, 13).expect("Valid date"),
    ///     },
    ///     InflationObservation {
    ///         reference_month: Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"),
    ///         value: 101.0,
    ///         available_on: Date::from_calendar_date(2024, Month::March, 12).expect("Valid date"),
    ///     },
    /// ];
    /// let cutoff = Date::from_calendar_date(2024, Month::April, 1).expect("Valid date");
    /// let index = InflationIndex::new("US-CPI", observations, Currency::USD, cutoff)
    ///     .expect("InflationIndex creation should succeed");
    /// let ctx = MarketContext::new().insert_inflation_index(index);
    /// assert!(ctx.get_inflation_index("US-CPI").is_ok());
    ///
    /// // With Arc for sharing
    /// # let observations2 = vec![
    /// #     InflationObservation {
    /// #         reference_month: Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"),
    /// #         value: 100.0,
    /// #         available_on: Date::from_calendar_date(2024, Month::February, 13).expect("Valid date"),
    /// #     },
    /// #     InflationObservation {
    /// #         reference_month: Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"),
    /// #         value: 101.0,
    /// #         available_on: Date::from_calendar_date(2024, Month::March, 12).expect("Valid date"),
    /// #     },
    /// # ];
    /// # let index2 = InflationIndex::new("EU-HICP", observations2, Currency::EUR, cutoff)
    /// #     .expect("InflationIndex creation should succeed");
    /// let shared = Arc::new(index2);
    /// let ctx2 = MarketContext::new().insert_inflation_index(Arc::clone(&shared));
    /// ```
    ///
    /// # Arguments
    ///
    /// * `index` - Owned or shared inflation index, stored under its embedded `id`; replaces any index with the same ID.
    pub fn insert_inflation_index(mut self, index: impl Into<Arc<InflationIndex>>) -> Self {
        self.insert_inflation_index_mut(index);
        self
    }

    /// Insert a credit index aggregate using the context's canonical curves.
    ///
    /// Hazard, base-correlation, and issuer references are resolved by ID from
    /// curves already in this context. The supplied references identify those
    /// curves; their embedded values do not replace the canonical curves.
    /// Insert replacement curves before inserting an updated index aggregate.
    ///
    /// # Errors
    ///
    /// Returns an error if a referenced curve is absent or has the wrong type
    /// (the aggregate's own invariants were checked by its builder).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData, HazardCurve};
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// let hazard = Arc::new(HazardCurve::builder("CDX")
    ///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    ///     .recovery_rate(0.40)
    ///     .knots([(1.0, 0.01), (5.0, 0.015)])
    ///     .day_count(finstack_quant_core::dates::DayCount::Act365F)
    ///     .build()
    ///     .expect("HazardCurve builder should succeed"));
    /// let base_corr = Arc::new(BaseCorrelationCurve::builder("CDX-BC")
    ///     .knots([(3.0, 0.25), (10.0, 0.55)])
    ///     .build()
    ///     .expect("BaseCorrelationCurve builder should succeed"));
    /// let data = CreditIndexData::builder()
    ///     .num_constituents(125)
    ///     .recovery_rate(0.4)
    ///     .index_credit_curve(Arc::clone(&hazard))
    ///     .base_correlation_curve(Arc::clone(&base_corr))
    ///     .build()
    ///     .expect("CreditIndexData builder should succeed");
    /// let ctx = MarketContext::new()
    ///     .try_insert(hazard).expect("insert market curve")
    ///     .try_insert(base_corr).expect("insert market curve")
    ///     .insert_credit_index("CDX-IG", data)
    ///     .expect("index dependencies are present");
    /// assert!(ctx.get_credit_index("CDX-IG").is_ok());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Lookup key stored as [`CurveId`].
    /// * `data` - Constituent count, recovery, and curve references. Each curve
    ///   ID must resolve to the matching curve type already in this context.
    pub fn insert_credit_index(
        mut self,
        id: impl AsRef<str>,
        data: CreditIndexData,
    ) -> crate::Result<Self> {
        self.insert_credit_index_mut(id, data)?;
        Ok(self)
    }

    /// Insert an FX matrix.
    ///
    /// Accepts either an owned [`FxMatrix`] or an `Arc<FxMatrix>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling FX matrix sharing between contexts).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::fx::{FxMatrix, FxProvider, FxConversionPolicy};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// struct StaticFx;
    /// impl FxProvider for StaticFx {
    ///     fn rate(
    ///         &self,
    ///         _from: Currency,
    ///         _to: Currency,
    ///         _on: Date,
    ///         _policy: FxConversionPolicy,
    ///     ) -> finstack_quant_core::Result<f64> {
    ///         Ok(1.1)
    ///     }
    /// }
    ///
    /// // Owned value
    /// let fx = FxMatrix::new(Arc::new(StaticFx), None);
    /// let ctx = MarketContext::new().insert_fx(fx);
    /// assert!(ctx.fx().is_some());
    ///
    /// // Pre-wrapped Arc for sharing
    /// # struct StaticFx2;
    /// # impl FxProvider for StaticFx2 {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy) -> finstack_quant_core::Result<f64> { Ok(1.2) }
    /// # }
    /// let shared_fx = Arc::new(FxMatrix::new(Arc::new(StaticFx2), None));
    /// let ctx2 = MarketContext::new().insert_fx(Arc::clone(&shared_fx));
    /// ```
    ///
    /// # Arguments
    ///
    /// * `fx` - Owned or shared FX matrix used for currency conversion.
    pub fn insert_fx(mut self, fx: impl Into<Arc<FxMatrix>>) -> Self {
        self.insert_fx_mut(fx);
        self
    }

    // Insert methods (mutable variants for binding layers)
    //
    // These `&mut self` variants mirror the consuming `insert_*` methods above but
    // mutate in place. They exist primarily so that Python/WASM binding wrappers
    // can avoid the `self.inner = std::mem::take(&mut self.inner).try_insert(..)?` dance
    // that is required to bridge a fluent builder API across an FFI boundary.
    //
    // The behaviour is identical to the fluent variants — same storage layout,
    // same credit-index rebinding — just with `&mut self` instead of `mut self`.
    /// Insert a generic curve storage entry in place, failing atomically if validation or credit-index rebinding fails.
    ///
    /// # Arguments
    ///
    /// * `curve` - Any curve type convertible into [`CurveStorage`], keyed by its own id;
    ///   replaces an existing curve of the same type and atomically rebinds dependent credit indices.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if the curve id is empty/blank, attempts to replace
    /// a curve of a different type, or invalidates a bound credit index.
    pub fn try_insert_mut<C>(&mut self, curve: C) -> crate::Result<&mut Self>
    where
        C: Into<CurveStorage>,
    {
        let curve: CurveStorage = curve.into();
        let id = curve.id().to_owned();
        if id.as_str().trim().is_empty() {
            return Err(crate::Error::Validation(
                "MarketContext curve id cannot be empty or blank".to_string(),
            ));
        }
        if let Some(existing) = self.curves.get(id.as_str())
            && existing.curve_type() != curve.curve_type()
        {
            return Err(crate::Error::Validation(format!(
                "Cannot replace curve '{id}' of type '{}' with curve of type '{}'",
                existing.curve_type(),
                curve.curve_type(),
            )));
        }
        let affects_credit_indices = self.curve_affects_credit_indices(&id);
        let previous = Arc::make_mut(&mut self.curves).insert(id.clone(), curve);
        if affects_credit_indices && let Err(err) = self.rebind_all_credit_indices() {
            let curves = Arc::make_mut(&mut self.curves);
            match previous {
                Some(prev) => {
                    curves.insert(id, prev);
                }
                None => {
                    curves.remove(id.as_str());
                }
            }
            return Err(err);
        }
        self.invalidate_scratch_tokens();
        Ok(self)
    }

    /// Insert a volatility surface, mutating in place.
    ///
    /// Mirrors [`Self::insert_surface`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `surface` - Owned or shared volatility surface stored under `surface.id()`.
    pub fn insert_surface_mut(&mut self, surface: impl Into<Arc<VolSurface>>) -> &mut Self {
        let surface = surface.into();
        Arc::make_mut(&mut self.surfaces).insert(surface.id().to_owned(), surface);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert an FX delta-quoted volatility surface, mutating in place.
    ///
    /// Accepts either an owned [`FxDeltaVolSurface`] or an `Arc<FxDeltaVolSurface>`;
    /// an owned value is wrapped in an `Arc`, a shared one is stored as is.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::market_data::surfaces::{
    /// #     FxAtmConvention, FxButterflyConvention, FxDeltaConvention, FxDeltaVolSurface,
    /// # };
    /// # use finstack_quant_core::money::fx::CurrencyPair;
    /// # let pair = CurrencyPair::try_new(Currency::EUR, Currency::USD).expect("valid pair");
    /// let surface = FxDeltaVolSurface::builder("EURUSD-DELTA-VOL", pair)
    ///     .delta_convention(FxDeltaConvention::ForwardUnadjusted)
    ///     .atm_convention(FxAtmConvention::DeltaNeutralStraddle)
    ///     .butterfly_convention(FxButterflyConvention::Smile)
    ///     .expiries(&[0.25, 0.5, 1.0])
    ///     .atm_vols(&[0.08, 0.085, 0.09])
    ///     .rr_25d(&[0.01, 0.012, 0.015])
    ///     .bf_25d(&[0.005, 0.006, 0.007])
    ///     .build()
    ///     .expect("surface should build");
    /// let mut ctx = MarketContext::new();
    /// ctx.insert_fx_delta_vol_surface_mut(surface);
    /// assert!(ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL").is_ok());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `surface` - Owned or shared FX delta-vol surface stored under `surface.id()`.
    pub fn insert_fx_delta_vol_surface_mut(
        &mut self,
        surface: impl Into<Arc<FxDeltaVolSurface>>,
    ) -> &mut Self {
        let surface = surface.into();
        Arc::make_mut(&mut self.fx_delta_vol_surfaces).insert(surface.id().to_owned(), surface);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert a SABR volatility cube, mutating in place.
    ///
    /// Mirrors [`Self::insert_vol_cube`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `cube` - Owned or shared SABR vol cube stored under `cube.id()`.
    pub fn insert_vol_cube_mut(&mut self, cube: impl Into<Arc<VolCube>>) -> &mut Self {
        let cube = cube.into();
        Arc::make_mut(&mut self.vol_cubes).insert(cube.id().to_owned(), cube);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert a dividend schedule, mutating in place.
    ///
    /// Mirrors [`Self::insert_dividends`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Owned or shared dividend schedule stored under `schedule.get_id()`.
    pub fn insert_dividends_mut(
        &mut self,
        schedule: impl Into<Arc<DividendSchedule>>,
    ) -> &mut Self {
        let arc_schedule = schedule.into();
        let id = arc_schedule.get_id().to_owned();
        Arc::make_mut(&mut self.dividends).insert(id, arc_schedule);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert a market scalar/price, mutating in place.
    ///
    /// Mirrors [`Self::insert_price`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `id` - Lookup key stored as [`CurveId`].
    /// * `price` - Scalar observable such as a spot price, spread, or unitless parameter.
    pub fn insert_price_mut(&mut self, id: impl AsRef<str>, price: MarketScalar) -> &mut Self {
        let id = CurveId::from(id.as_ref());
        Arc::make_mut(&mut self.prices).insert(id, price);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert a scalar time series, mutating in place.
    ///
    /// Mirrors [`Self::insert_series`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `series` - Dated scalar series stored under `series.id()`.
    pub fn insert_series_mut(&mut self, series: ScalarTimeSeries) -> &mut Self {
        let id = series.id().to_owned();
        Arc::make_mut(&mut self.series).insert(id, series);
        self.invalidate_scratch_tokens();
        self
    }

    /// Store a series of historical observations under a contractual fixing index.
    ///
    /// # Arguments
    ///
    /// * `index_id` - Trimmed contractual rate or benchmark name without the
    ///   `FIXING:` prefix.
    /// * `series` - Dated observations, stored under the derived
    ///   `FIXING:{index_id}` identifier. Replaces an existing series under that key.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the index name is empty, untrimmed, or prefixed.
    pub fn insert_fixing(
        &mut self,
        index_id: &str,
        series: ScalarTimeSeries,
    ) -> crate::Result<&mut Self> {
        use crate::market_data::fixings::{fixing_series_id, validate_fixing_index_id};
        let id = fixing_series_id(validate_fixing_index_id(index_id)?);
        Ok(self.insert_series_mut(series.with_id(id.into())))
    }

    /// Store a continuous dividend yield as a unitless market scalar.
    ///
    /// # Arguments
    ///
    /// * `id` - Market identifier configured on the instrument's dividend-yield input.
    /// * `yield_rate` - Finite annualized continuously compounded decimal yield (0.02 is 2%).
    ///
    /// # Errors
    ///
    /// Returns a validation error for a non-finite yield.
    pub fn insert_dividend_yield(&mut self, id: &str, yield_rate: f64) -> crate::Result<&mut Self> {
        Ok(self.insert_price_mut(id, MarketScalar::unitless(yield_rate)?))
    }

    /// Insert an inflation index, mutating in place.
    ///
    /// Mirrors [`Self::insert_inflation_index`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `index` - Owned or shared inflation index, stored under its embedded `id`; replaces any index with the same ID.
    pub fn insert_inflation_index_mut(
        &mut self,
        index: impl Into<Arc<InflationIndex>>,
    ) -> &mut Self {
        let index = index.into();
        let key = CurveId::from(index.id());
        Arc::make_mut(&mut self.inflation_indices).insert(key, index);
        self.invalidate_scratch_tokens();
        self
    }

    /// Insert a credit index aggregate, mutating in place.
    ///
    /// Resolves every curve reference against the canonical context before
    /// changing the index map. Failure leaves the context unchanged.
    ///
    /// # Arguments
    ///
    /// * `id` - Lookup key for the credit index aggregate.
    /// * `data` - Constituent data whose hazard, base-correlation, and issuer
    ///   curve IDs must already resolve in this context to the matching types.
    ///   Embedded curve values are replaced by those canonical references.
    ///
    /// # Errors
    ///
    /// Returns an error when `id` is empty/blank or a required curve is absent or has
    /// the wrong type; the aggregate's own invariants were checked by its builder.
    pub fn insert_credit_index_mut(
        &mut self,
        id: impl AsRef<str>,
        data: CreditIndexData,
    ) -> crate::Result<&mut Self> {
        let key = id.as_ref();
        if key.trim().is_empty() {
            return Err(crate::Error::Validation(
                "MarketContext credit index id cannot be empty or blank".to_string(),
            ));
        }
        let data = self.rebind_credit_index_data(&data)?;
        let key = CurveId::from(key);
        Arc::make_mut(&mut self.credit_indices).insert(key, Arc::new(data));
        self.invalidate_scratch_tokens();
        Ok(self)
    }

    /// Remove a credit index aggregate by identifier, mutating in place.
    ///
    /// # Arguments
    ///
    /// * `id` - Lookup key of the credit index aggregate to remove.
    ///
    /// # Returns
    ///
    /// The removed shared [`CreditIndexData`] if present, or `None` when absent.
    pub fn remove_credit_index_mut(&mut self, id: impl AsRef<str>) -> Option<Arc<CreditIndexData>> {
        let removed = Arc::make_mut(&mut self.credit_indices).remove(id.as_ref());
        self.invalidate_scratch_tokens();
        removed
    }

    /// Restore and insert a credit index from its persisted curve references.
    ///
    /// Every referenced curve must already exist in this context. Resolves the
    /// canonical curve handles, validates constituent data, and then inserts the
    /// aggregate under `state.id`. Failure leaves the context unchanged.
    ///
    /// # Arguments
    ///
    /// * `state` - Borrowed snapshot containing the index lookup ID, positive
    ///   constituent count, finite decimal recoveries in `[0, 1]`, and IDs of
    ///   hazard/base-correlation curves already in this context. Optional issuer
    ///   curves must cover every constituent; supplied weights must cover those
    ///   issuers, be finite and nonnegative, and sum to one. Omitted issuer curves
    ///   use the index hazard curve, omitted recoveries use the index recovery,
    ///   and omitted weights are equal. Existing data under `state.id` is replaced
    ///   only after all references and values pass validation.
    ///
    /// # Errors
    ///
    /// Returns an error if a referenced curve is absent or has the wrong type,
    /// or constituent counts, recoveries, issuer maps or weights are invalid.
    pub fn insert_credit_index_state(&mut self, state: &CreditIndexState) -> crate::Result<()> {
        if state.id.trim().is_empty() {
            return Err(crate::Error::Validation(
                "MarketContext credit index state id cannot be empty or blank".to_string(),
            ));
        }
        let index_curve = self.get_hazard(&state.index_credit_curve_id)?;
        let base_corr = self.get_base_correlation(&state.base_correlation_curve_id)?;
        let mut builder = CreditIndexData::builder()
            .num_constituents(state.num_constituents)
            .recovery_rate(state.recovery_rate)
            .index_credit_curve(index_curve)
            .base_correlation_curve(base_corr);
        if let Some(issuer_ids) = &state.issuer_credit_curve_ids {
            let mut issuer_curves = BTreeMap::new();
            for (issuer, curve_id) in issuer_ids {
                issuer_curves.insert(issuer.clone(), self.get_hazard(curve_id)?);
            }
            builder = builder.issuer_curves(issuer_curves);
        }
        if let Some(recovery_rates) = &state.issuer_recovery_rates {
            builder = builder.issuer_recovery_rates(
                recovery_rates
                    .iter()
                    .map(|(key, value)| (key.clone(), *value)),
            );
        }
        if let Some(weights) = &state.issuer_weights {
            builder =
                builder.issuer_weights(weights.iter().map(|(key, value)| (key.clone(), *value)));
        }
        // The curves were resolved above; insert directly rather than rebinding
        // them a second time through `insert_credit_index_mut`.
        let data = builder.build()?;
        Arc::make_mut(&mut self.credit_indices)
            .insert(CurveId::from(state.id.as_str()), Arc::new(data));
        self.invalidate_scratch_tokens();
        Ok(())
    }

    /// Insert an FX matrix, mutating in place.
    ///
    /// Mirrors [`Self::insert_fx`] but takes `&mut self`.
    ///
    /// # Arguments
    ///
    /// * `fx` - Owned or shared FX matrix used for currency conversion.
    pub fn insert_fx_mut(&mut self, fx: impl Into<Arc<FxMatrix>>) -> &mut Self {
        self.fx = Some(fx.into());
        self.invalidate_scratch_tokens();
        self
    }

    /// Clear the FX matrix, mutating in place.
    pub fn clear_fx_mut(&mut self) -> &mut Self {
        self.fx = None;
        self.invalidate_scratch_tokens();
        self
    }
}
