//! Typed market-data retrieval APIs.

use std::sync::Arc;

use crate::collections::HashMap;
use crate::market_data::dividends::DividendSchedule;
use crate::market_data::scalars::{InflationIndex, MarketScalar, ScalarTimeSeries};
use crate::market_data::surfaces::{FxDeltaVolSurface, VolCube, VolSurface};
use crate::market_data::term_structures::{
    BaseCorrelationCurve, BasisSpreadCurve, CreditIndexData, DiscountCurve, ForwardCurve,
    HazardCurve, InflationCurve, ParametricCurve, PriceCurve,
};
use crate::market_data::traits::Discounting;
use crate::types::CurveId;
use crate::Result;

use super::{CurveStorage, MarketContext};

impl MarketContext {
    #[inline]
    fn not_found_error(id: &str) -> crate::Error {
        crate::error::InputError::NotFound { id: id.to_string() }.into()
    }

    fn missing_curve_error(&self, id: &str) -> crate::Error {
        // Sort for deterministic error messages (HashMap iteration order varies).
        let mut available: Vec<&str> = self.curves.keys().map(|k| k.as_str()).collect();
        available.sort_unstable();
        crate::error::Error::missing_curve_with_suggestions(id, &available)
    }

    #[inline]
    fn get_cloned<T>(&self, map: &HashMap<CurveId, Arc<T>>, id: &str) -> Result<Arc<T>> {
        map.get(id)
            .cloned()
            .ok_or_else(|| Self::not_found_error(id))
    }

    #[inline]
    fn get_ref<'a, T>(&self, map: &'a HashMap<CurveId, T>, id: &str) -> Result<&'a T> {
        map.get(id).ok_or_else(|| Self::not_found_error(id))
    }

    /// Helper method to extract curve with type checking and error handling
    fn get_curve_with_type_check<T, F>(
        &self,
        id: &str,
        expected_type: &'static str,
        extractor: F,
    ) -> Result<T>
    where
        F: FnOnce(&CurveStorage) -> Option<T>,
    {
        match self.curves.get(id) {
            Some(storage) => extractor(storage).ok_or_else(|| {
                crate::error::InputError::WrongCurveType {
                    id: id.to_string(),
                    expected: expected_type.to_string(),
                    actual: storage.curve_type().to_string(),
                }
                .into()
            }),
            None => Err(self.missing_curve_error(id)),
        }
    }

    /// Get a discount curve by identifier.
    ///
    /// Returns an [`Arc`] clone of the stored curve so callers can retain the
    /// curve independently of the context. The identifier should match the
    /// curve's logical market-data key such as `"USD-OIS"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or exists under a different
    /// curve type.
    pub fn get_discount(&self, id: impl AsRef<str>) -> Result<Arc<DiscountCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Discount", |storage| {
            storage.discount().map(Arc::clone)
        })
    }

    /// Get a forward curve by identifier.
    ///
    /// Forward-curve identifiers usually encode both the market and tenor, such
    /// as `"USD-SOFR3M"` or `"EUR-EURIBOR6M"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a non-forward curve.
    pub fn get_forward(&self, id: impl AsRef<str>) -> Result<Arc<ForwardCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Forward", |storage| {
            storage.forward().map(Arc::clone)
        })
    }

    /// Get a hazard curve by identifier.
    ///
    /// Hazard curves are stored as annualized default intensities keyed by a
    /// credit-specific identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_hazard(&self, id: impl AsRef<str>) -> Result<Arc<HazardCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Hazard", |storage| storage.hazard().map(Arc::clone))
    }

    /// Get an inflation curve by identifier.
    ///
    /// Inflation curves are typically keyed by an index family such as
    /// `"US-CPI"` or `"UK-RPI"` and represent CPI-linked term structures rather
    /// than spot index observations.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_inflation_curve(&self, id: impl AsRef<str>) -> Result<Arc<InflationCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Inflation", |storage| {
            storage.inflation().map(Arc::clone)
        })
    }

    /// Get a base correlation curve by identifier.
    ///
    /// Use this for tranche-style credit workflows where correlation is quoted
    /// by detachment point.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_base_correlation(&self, id: impl AsRef<str>) -> Result<Arc<BaseCorrelationCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "BaseCorrelation", |storage| {
            storage.base_correlation().map(Arc::clone)
        })
    }

    /// Get a volatility index curve by identifier.
    ///
    /// These curves typically store forward levels for instruments such as VIX futures.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_vol_index_curve(&self, id: impl AsRef<str>) -> Result<Arc<PriceCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "VolIndex", |storage| {
            storage.vol_index().map(Arc::clone)
        })
    }

    /// Get a price curve by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::PriceCurve;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let base = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
    /// # let curve = PriceCurve::builder("WTI-FORWARD")
    /// #     .base_date(base)
    /// #     .spot_price(75.0)
    /// #     .knots([(0.0, 75.0), (0.5, 77.0)])
    /// #     .build()
    /// #     .expect("PriceCurve builder should succeed");
    /// # let ctx = MarketContext::new().insert(curve);
    /// let price_curve = ctx.get_price_curve("WTI-FORWARD").expect("Price curve should exist");
    /// assert!(price_curve.price(0.25) > 0.0);
    /// ```
    pub fn get_price_curve(&self, id: impl AsRef<str>) -> Result<Arc<PriceCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Price", |storage| storage.price().map(Arc::clone))
    }

    /// Get a basis spread curve by identifier.
    ///
    /// Basis spread curves store cross-currency or multi-curve spread data.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_basis_spread(&self, id: impl AsRef<str>) -> Result<Arc<BasisSpreadCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "BasisSpread", |storage| {
            storage.basis_spread().map(Arc::clone)
        })
    }

    /// Get a parametric curve by identifier.
    ///
    /// Parametric curves use Nelson-Siegel or Nelson-Siegel-Svensson models.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is missing or refers to a different curve type.
    pub fn get_parametric(&self, id: impl AsRef<str>) -> Result<Arc<ParametricCurve>> {
        let id_str = id.as_ref();
        self.get_curve_with_type_check(id_str, "Parametric", |storage| {
            storage.parametric().map(Arc::clone)
        })
    }

    /// Iterate over all stored discount curves without cloning the context.
    ///
    /// Yields `(curve_id, Arc<DiscountCurve>)` pairs in arbitrary order. Use this
    /// when you need to scan discount curves (e.g. for currency-prefix matching)
    /// in a hot path where materializing a
    /// [`MarketContextState`](super::MarketContextState) would be wasteful.
    pub fn iter_discount_curves(
        &self,
    ) -> impl Iterator<Item = (&CurveId, Arc<DiscountCurve>)> + '_ {
        self.curves
            .iter()
            .filter_map(|(id, storage)| storage.discount().map(|c| (id, Arc::clone(c))))
    }

    /// Clone a volatility surface by identifier.
    ///
    /// Returns the strike-grid surface stored under `id`. Use
    /// [`Self::get_fx_delta_vol_surface`] for FX smiles quoted in delta space.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::VolSurface;
    /// # use finstack_quant_core::types::CurveId;
    /// # let surface = VolSurface::builder("IR-Swaption")
    /// #     .expiries(&[1.0, 2.0])
    /// #     .strikes(&[90.0, 100.0])
    /// #     .row(&[0.2, 0.2])
    /// #     .row(&[0.2, 0.2])
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert_surface(surface);
    /// let surface = ctx.get_surface("IR-Swaption").expect("Surface should exist");
    /// assert_eq!(surface.id(), &CurveId::from("IR-Swaption"));
    /// assert_eq!(surface.grid_shape(), (2, 2));
    /// ```
    pub fn get_surface(&self, id: impl AsRef<str>) -> Result<Arc<VolSurface>> {
        self.get_cloned(&self.surfaces, id.as_ref())
    }

    /// Borrow a market price/scalar by identifier.
    ///
    /// Scalars are borrowed rather than cloned because they are typically small
    /// immutable values already owned by the context.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::scalars::MarketScalar;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::currency::Currency;
    ///
    /// let ctx = MarketContext::new()
    ///     .insert_price("AAPL", MarketScalar::Price(Money::from((180_i64, Currency::USD))));
    /// if let MarketScalar::Price(price) = ctx.get_price("AAPL").expect("Price should exist") {
    ///     assert_eq!(price.currency(), Currency::USD);
    /// }
    /// ```
    pub fn get_price(&self, id: impl AsRef<str>) -> Result<&MarketScalar> {
        self.get_ref(&self.prices, id.as_ref())
    }

    /// Borrow a scalar time series by identifier.
    ///
    /// Time series remain borrowed so callers can inspect them without copying
    /// the stored observation history.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let series = ScalarTimeSeries::new(
    /// #     "VOL-TS",
    /// #     vec![
    /// #         (Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"), 0.2),
    /// #         (Date::from_calendar_date(2024, Month::February, 1).expect("Valid date"), 0.25),
    /// #     ],
    /// #     None,
    /// # ).expect("... creation should succeed");
    /// # let ctx = MarketContext::new().insert_series(series);
    /// let series = ctx.get_series("VOL-TS").expect("Series should exist");
    /// assert_eq!(series.id().as_str(), "VOL-TS");
    /// ```
    pub fn get_series(&self, id: impl AsRef<str>) -> Result<&ScalarTimeSeries> {
        self.get_ref(&self.series, id.as_ref())
    }

    /// Clone an inflation index by identifier.
    ///
    /// Inflation indices represent historical CPI-style observations, not
    /// forward-looking inflation curves.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::scalars::{InflationIndex, InflationInterpolation};
    /// # use finstack_quant_core::currency::Currency;
    /// # use finstack_quant_core::dates::Date;
    /// # use time::Month;
    /// # let observations = vec![
    /// #     (Date::from_calendar_date(2024, Month::January, 31).expect("Valid date"), 100.0),
    /// #     (Date::from_calendar_date(2024, Month::February, 29).expect("Valid date"), 101.0),
    /// # ];
    /// # let index = InflationIndex::new("US-CPI", observations, Currency::USD)
    /// #     .expect("... creation should succeed")
    /// #     .with_interpolation(InflationInterpolation::Linear);
    /// # let ctx = MarketContext::new().insert_inflation_index("US-CPI", index);
    /// let idx = ctx.get_inflation_index("US-CPI").expect("Inflation index should exist");
    /// assert_eq!(idx.id, "US-CPI");
    /// ```
    pub fn get_inflation_index(&self, id: impl AsRef<str>) -> Result<Arc<InflationIndex>> {
        self.get_cloned(&self.inflation_indices, id.as_ref())
    }

    /// Clone an FX delta-quoted volatility surface by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::surfaces::FxDeltaVolSurface;
    /// # let surface = FxDeltaVolSurface::new(
    /// #     "EURUSD-DELTA-VOL",
    /// #     vec![0.25, 0.5, 1.0],
    /// #     vec![0.08, 0.085, 0.09],
    /// #     vec![0.01, 0.012, 0.015],
    /// #     vec![0.005, 0.006, 0.007],
    /// #     None,
    /// # ).expect("surface should build");
    /// # let ctx = MarketContext::new().insert_fx_delta_vol_surface(surface);
    /// let surf = ctx.get_fx_delta_vol_surface("EURUSD-DELTA-VOL")
    ///     .expect("surface should exist");
    /// assert_eq!(surf.id().as_str(), "EURUSD-DELTA-VOL");
    /// ```
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier used when the surface was inserted.
    pub fn get_fx_delta_vol_surface(&self, id: impl AsRef<str>) -> Result<Arc<FxDeltaVolSurface>> {
        self.get_cloned(&self.fx_delta_vol_surfaces, id.as_ref())
    }

    /// Clone a dividend schedule by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    pub fn get_dividend_schedule(&self, id: impl AsRef<str>) -> Result<Arc<DividendSchedule>> {
        self.get_cloned(&self.dividends, id.as_ref())
    }

    /// Clone a credit index aggregate by identifier.
    ///
    /// # Examples
    /// ```rust
    /// # use finstack_quant_core::market_data::context::MarketContext;
    /// # use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData, HazardCurve};
    /// # use finstack_quant_core::dates::Date;
    /// # use std::sync::Arc;
    /// # use time::Month;
    /// # let hazard = Arc::new(HazardCurve::builder("CDX")
    /// #     .base_date(Date::from_calendar_date(2024, Month::January, 1).expect("Valid date"))
    /// #     .recovery_rate(0.40)
    /// #     .knots([(0.0, 0.01), (5.0, 0.015)])
    /// #     .build()
    /// #     .expect("... creation should succeed"));
    /// # let base_corr = Arc::new(BaseCorrelationCurve::builder("CDX")
    /// #     .knots([(3.0, 0.25), (10.0, 0.55)])
    /// #     .build()
    /// #     .expect("... creation should succeed"));
    /// # let data = CreditIndexData::builder()
    /// #     .num_constituents(125)
    /// #     .recovery_rate(0.4)
    /// #     .index_credit_curve(Arc::clone(&hazard))
    /// #     .base_correlation_curve(base_corr)
    /// #     .build()
    /// #     .expect("... builder should succeed");
    /// # let ctx = MarketContext::new().insert_credit_index("CDX-IG", data);
    /// let idx = ctx.get_credit_index("CDX-IG").expect("Credit index should exist");
    /// assert_eq!(idx.num_constituents, 125);
    /// ```
    pub fn get_credit_index(&self, id: impl AsRef<str>) -> Result<Arc<CreditIndexData>> {
        self.get_cloned(&self.credit_indices, id.as_ref())
    }

    /// Clone a SABR volatility cube by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is not present.
    pub fn get_vol_cube(&self, id: impl AsRef<str>) -> Result<Arc<VolCube>> {
        self.get_cloned(&self.vol_cubes, id.as_ref())
    }

    /// Resolve a collateral discount curve for a CSA code.
    ///
    /// This performs the indirection from a CSA or collateral agreement code to
    /// the discount curve configured for that collateral set. The returned trait
    /// object exposes the generic [`Discounting`] interface rather than the
    /// concrete [`DiscountCurve`] type.
    ///
    /// # Errors
    ///
    /// Returns an error if the CSA code is not mapped or if the mapped curve is missing.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::market_data::term_structures::DiscountCurve;
    /// use finstack_quant_core::math::interp::InterpStyle;
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
    /// let discount = ctx.get_collateral("USD-CSA").expect("Collateral curve should exist");
    /// assert!(discount.df(0.5) <= 1.0);
    /// ```
    pub fn get_collateral(&self, csa_code: &str) -> Result<Arc<dyn Discounting + Send + Sync>> {
        let curve_id = self
            .collateral
            .get(csa_code)
            .ok_or(crate::error::InputError::NotFound {
                id: format!("collateral:{}", csa_code),
            })?;
        self.get_discount(curve_id.as_str())
            .map(|arc| arc as Arc<dyn Discounting + Send + Sync>)
    }
}
