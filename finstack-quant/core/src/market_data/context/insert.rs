//! Market-data insertion and mutation APIs.

use std::sync::Arc;

use crate::market_data::dividends::DividendSchedule;
use crate::market_data::scalars::{InflationIndex, MarketScalar, ScalarTimeSeries};
use crate::market_data::surfaces::{FxDeltaVolSurface, VolCube, VolSurface};
use crate::market_data::term_structures::CreditIndexData;
use crate::money::fx::FxMatrix;
use crate::types::CurveId;

use super::{CurveStorage, MarketContext};

impl MarketContext {
    /// Insert a generic curve storage entry.
    ///
    /// This is primarily intended for downstream crates that operate on heterogeneous
    /// curve types (e.g., calibration pipelines) and want to update the context
    /// without matching on concrete curve variants.
    ///
    /// # Arguments
    ///
    /// * `curve` - Term structure inserted into or consumed from the market context.
    pub fn insert<C>(mut self, curve: C) -> Self
    where
        C: Into<CurveStorage>,
    {
        self.insert_mut(curve);
        self
    }

    /// Insert a volatility surface.
    ///
    /// Accepts either an owned [`VolSurface`] or an `Arc<VolSurface>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling surface sharing between contexts).
    ///
    /// # Parameters
    /// - `surface`: a [`VolSurface`] or `Arc<VolSurface>`
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::VolSurface;
    /// # use std::sync::Arc;
    /// # let surface = VolSurface::builder("IR-Swaption")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .strikes(&[90.0, 100.0])
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
    /// #     .strikes(&[90.0, 100.0])
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
    /// * `surface` - Volatility surface inserted into or consumed from the market context.
    pub fn insert_surface(mut self, surface: impl Into<Arc<VolSurface>>) -> Self {
        self.insert_surface_mut(surface);
        self
    }

    /// Insert an FX delta-quoted volatility surface.
    ///
    /// Accepts either an owned [`FxDeltaVolSurface`] or an `Arc<FxDeltaVolSurface>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling surface sharing between contexts).
    ///
    /// # Parameters
    /// - `surface`: a [`FxDeltaVolSurface`] or `Arc<FxDeltaVolSurface>`
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;
    /// let surface = FxDeltaVolSurface::new(
    ///     "EURUSD-DELTA-VOL",
    ///     vec![0.25, 0.5, 1.0],
    ///     vec![0.08, 0.085, 0.09],
    ///     vec![0.01, 0.012, 0.015],
    ///     vec![0.005, 0.006, 0.007],
    ///     None,
    /// ).expect("surface should build");
    /// let ctx = MarketContext::new().insert_fx_delta_vol_surface(surface);
    /// assert!(ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL").is_ok());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `surface` - Volatility surface inserted into or consumed from the market context.
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
    ///
    /// # Parameters
    /// - `cube`: a [`VolCube`] or `Arc<VolCube>`
    ///
    /// # Arguments
    ///
    /// * `cube` - Volatility cube whose expiry, tenor, and strike dimensions are validated.
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
    /// # Parameters
    /// - `schedule`: a [`DividendSchedule`] or `Arc<DividendSchedule>` built via its builder
    ///
    /// # Arguments
    ///
    /// * `schedule` - Dated cash-flow or fixing schedule registered in the market context.
    pub fn insert_dividends(mut self, schedule: impl Into<Arc<DividendSchedule>>) -> Self {
        self.insert_dividends_mut(schedule);
        self
    }

    /// Insert a market scalar/price.
    ///
    /// # Parameters
    /// - `id`: identifier (string-like) stored as [`CurveId`]
    /// - `price`: scalar value to store
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `price` - Finite market price in the instrument quote convention.
    pub fn insert_price(mut self, id: impl AsRef<str>, price: MarketScalar) -> Self {
        self.insert_price_mut(id, price);
        self
    }

    /// Insert a scalar time series.
    ///
    /// # Parameters
    /// - `series`: [`ScalarTimeSeries`] to store
    ///
    /// # Arguments
    ///
    /// * `series` - Time-ordered numeric samples for a single risk factor or price series
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
    /// # Parameters
    /// - `id`: identifier stored as [`CurveId`]
    /// - `index`: an [`InflationIndex`] or `Arc<InflationIndex>`
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::dates::Date;
    /// use std::sync::Arc;
    /// use time::Month;
    ///
    /// let observations = vec![
    ///     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    ///     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// ];
    /// let index = InflationIndex::new("US-CPI", observations, Currency::USD)
    ///     .expect("InflationIndex creation should succeed")
    ///     .with_interpolation(InflationInterpolation::Linear);
    /// let ctx = MarketContext::new().insert_inflation_index("US-CPI", index);
    /// assert!(ctx.get_inflation_index("US-CPI").is_ok());
    ///
    /// // With Arc for sharing
    /// # let observations2 = vec![
    /// #     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    /// #     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// # ];
    /// # let index2 = InflationIndex::new("EU-HICP", observations2, Currency::EUR)
    /// #     .expect("InflationIndex creation should succeed");
    /// let shared = Arc::new(index2);
    /// let ctx2 = MarketContext::new().insert_inflation_index("EU-HICP", Arc::clone(&shared));
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `index` - Zero-based index selecting an entry from the ordered collection
    pub fn insert_inflation_index(
        mut self,
        id: impl AsRef<str>,
        index: impl Into<Arc<InflationIndex>>,
    ) -> Self {
        self.insert_inflation_index_mut(id, index);
        self
    }

    /// Insert a credit index aggregate.
    ///
    /// # Parameters
    /// - `id`: identifier stored as [`CurveId`]
    /// - `data`: [`CreditIndexData`] bundle
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
    ///     .knots([(0.0, 0.01), (5.0, 0.015)])
    ///     .build()
    ///     .expect("HazardCurve builder should succeed"));
    /// let base_corr = Arc::new(BaseCorrelationCurve::builder("CDX")
    ///     .knots([(3.0, 0.25), (10.0, 0.55)])
    ///     .build()
    ///     .expect("BaseCorrelationCurve builder should succeed"));
    /// let data = CreditIndexData::builder()
    ///     .num_constituents(125)
    ///     .recovery_rate(0.4)
    ///     .index_credit_curve(Arc::clone(&hazard))
    ///     .base_correlation_curve(base_corr)
    ///     .build()
    ///     .expect("CreditIndexData builder should succeed");
    /// let ctx = MarketContext::new().insert_credit_index("CDX-IG", data);
    /// assert!(ctx.get_credit_index("CDX-IG").is_ok());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
    /// * `data` - Validated market-data payload to insert or transform.
    pub fn insert_credit_index(mut self, id: impl AsRef<str>, data: CreditIndexData) -> Self {
        self.insert_credit_index_mut(id, data);
        self
    }

    /// Insert an FX matrix.
    ///
    /// Accepts either an owned [`FxMatrix`] or an `Arc<FxMatrix>`.
    /// When passing an owned value, it will be wrapped in an `Arc` automatically.
    /// When passing an `Arc`, it is used directly (enabling FX matrix sharing between contexts).
    ///
    /// # Parameters
    /// - `fx`: [`FxMatrix`] or `Arc<FxMatrix>` instance used for currency conversions
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
    /// let fx = FxMatrix::new(Arc::new(StaticFx));
    /// let ctx = MarketContext::new().insert_fx(fx);
    /// assert!(ctx.fx().is_some());
    ///
    /// // Pre-wrapped Arc for sharing
    /// # struct StaticFx2;
    /// # impl FxProvider for StaticFx2 {
    /// #     fn rate(&self, _from: Currency, _to: Currency, _on: Date, _policy: FxConversionPolicy) -> finstack_quant_core::Result<f64> { Ok(1.2) }
    /// # }
    /// let shared_fx = Arc::new(FxMatrix::new(Arc::new(StaticFx2)));
    /// let ctx2 = MarketContext::new().insert_fx(Arc::clone(&shared_fx));
    /// ```
    ///
    /// # Arguments
    ///
    /// * `fx` - FX matrix or provider used to convert cashflows into reporting currency
    pub fn insert_fx(mut self, fx: impl Into<Arc<FxMatrix>>) -> Self {
        self.insert_fx_mut(fx);
        self
    }
    /// Map collateral CSA code to a discount curve identifier.
    ///
    /// # Parameters
    /// - `csa_code`: CSA identifier (e.g., "USD-CSA")
    /// - `discount_id`: target discount curve [`CurveId`]
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::dates::Date;
    /// use finstack_quant_core::types::CurveId;
    /// use time::Month;
    ///
    /// let curve = DiscountCurve::builder("USD-OIS")
    ///     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    ///     .knots([(0.0, 1.0), (1.0, 0.99)])
    ///     .build()
    ///     .expect("... builder should succeed");
    /// let ctx = MarketContext::new()
    ///     .insert(curve)
    ///     .map_collateral("USD-CSA", CurveId::from("USD-OIS"));
    /// assert!(ctx.get_collateral("USD-CSA").is_ok());
    /// ```
    ///
    /// # Arguments
    ///
    /// * `csa_code` - CSA identifier used to select collateral and discounting terms.
    /// * `discount_id` - Identifier of the discount curve used for present-value calculations.
    pub fn map_collateral(mut self, csa_code: impl Into<String>, discount_id: CurveId) -> Self {
        self.map_collateral_mut(csa_code, discount_id);
        self
    }

    // Insert methods (mutable variants for binding layers)
    //
    // These `&mut self` variants mirror the consuming `insert_*` methods above but
    // mutate in place. They exist primarily so that Python/WASM binding wrappers
    // can avoid the `self.inner = std::mem::take(&mut self.inner).insert(..)` dance
    // that is required to bridge a fluent builder API across an FFI boundary.
    //
    // The behaviour is identical to the fluent variants — same storage layout,
    // same credit-index rebinding — just with `&mut self` instead of `mut self`.
    /// Insert a generic curve storage entry, mutating in place.
    ///
    /// Mirrors [`Self::insert`] but takes `&mut self`.
    pub fn insert_mut<C>(&mut self, curve: C) -> &mut Self
    where
        C: Into<CurveStorage>,
    {
        let curve: CurveStorage = curve.into();
        let id = curve.id().to_owned();
        Arc::make_mut(&mut self.curves).insert(id, curve);
        if !self.credit_indices.is_empty() {
            let _invalidated = self.rebind_all_credit_indices();
        }
        self
    }

    /// Insert a volatility surface, mutating in place.
    ///
    /// Mirrors [`Self::insert_surface`] but takes `&mut self`.
    pub fn insert_surface_mut(&mut self, surface: impl Into<Arc<VolSurface>>) -> &mut Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.surfaces).insert(id, arc_surface);
        self
    }

    /// Insert an FX delta-quoted volatility surface, mutating in place.
    ///
    /// Mirrors [`Self::insert_fx_delta_vol_surface`] but takes `&mut self`.
    pub fn insert_fx_delta_vol_surface_mut(
        &mut self,
        surface: impl Into<Arc<FxDeltaVolSurface>>,
    ) -> &mut Self {
        let arc_surface = surface.into();
        let id = arc_surface.id().to_owned();
        Arc::make_mut(&mut self.fx_delta_vol_surfaces).insert(id, arc_surface);
        self
    }

    /// Insert a SABR volatility cube, mutating in place.
    ///
    /// Mirrors [`Self::insert_vol_cube`] but takes `&mut self`.
    pub fn insert_vol_cube_mut(&mut self, cube: impl Into<Arc<VolCube>>) -> &mut Self {
        let arc = cube.into();
        let id = arc.id().to_owned();
        Arc::make_mut(&mut self.vol_cubes).insert(id, arc);
        self
    }

    /// Insert a dividend schedule, mutating in place.
    ///
    /// Mirrors [`Self::insert_dividends`] but takes `&mut self`.
    pub fn insert_dividends_mut(
        &mut self,
        schedule: impl Into<Arc<DividendSchedule>>,
    ) -> &mut Self {
        let arc_schedule = schedule.into();
        let id = arc_schedule.get_id().to_owned();
        Arc::make_mut(&mut self.dividends).insert(id, arc_schedule);
        self
    }

    /// Insert a market scalar/price, mutating in place.
    ///
    /// Mirrors [`Self::insert_price`] but takes `&mut self`.
    pub fn insert_price_mut(&mut self, id: impl AsRef<str>, price: MarketScalar) -> &mut Self {
        Arc::make_mut(&mut self.prices).insert(CurveId::from(id.as_ref()), price);
        self
    }

    /// Insert a scalar time series, mutating in place.
    ///
    /// Mirrors [`Self::insert_series`] but takes `&mut self`.
    pub fn insert_series_mut(&mut self, series: ScalarTimeSeries) -> &mut Self {
        let id = series.id().to_owned();
        Arc::make_mut(&mut self.series).insert(id, series);
        self
    }

    /// Insert an inflation index, mutating in place.
    ///
    /// Mirrors [`Self::insert_inflation_index`] but takes `&mut self`.
    pub fn insert_inflation_index_mut(
        &mut self,
        id: impl AsRef<str>,
        index: impl Into<Arc<InflationIndex>>,
    ) -> &mut Self {
        let index = index.into();
        let key = Self::inflation_index_key_for_insert(id, index.as_ref());
        Arc::make_mut(&mut self.inflation_indices).insert(key, index);
        self
    }

    /// Insert a credit index aggregate, mutating in place.
    ///
    /// Mirrors [`Self::insert_credit_index`] but takes `&mut self`.
    pub fn insert_credit_index_mut(
        &mut self,
        id: impl AsRef<str>,
        data: CreditIndexData,
    ) -> &mut Self {
        let key = CurveId::from(id.as_ref());
        Arc::make_mut(&mut self.credit_indices).insert(key, Arc::new(data));
        self
    }

    /// Insert an FX matrix, mutating in place.
    ///
    /// Mirrors [`Self::insert_fx`] but takes `&mut self`.
    pub fn insert_fx_mut(&mut self, fx: impl Into<Arc<FxMatrix>>) -> &mut Self {
        self.fx = Some(fx.into());
        self
    }

    /// Clear the FX matrix, mutating in place.
    pub fn clear_fx_mut(&mut self) -> &mut Self {
        self.fx = None;
        self
    }

    /// Map collateral CSA code to a discount curve identifier, mutating in place.
    ///
    /// Mirrors [`Self::map_collateral`] but takes `&mut self`.
    pub fn map_collateral_mut(
        &mut self,
        csa_code: impl Into<String>,
        discount_id: CurveId,
    ) -> &mut Self {
        Arc::make_mut(&mut self.collateral).insert(csa_code.into(), discount_id);
        self
    }

    #[inline]
    pub(crate) fn inflation_index_key_for_insert(
        id: impl AsRef<str>,
        index: &InflationIndex,
    ) -> CurveId {
        let key = CurveId::from(id.as_ref());
        assert!(
            key.as_str() == index.id,
            "MarketContext::insert_inflation_index key '{}' must match InflationIndex.id '{}'",
            key.as_str(),
            index.id
        );
        key
    }
}
