//! Heterogeneous curve storage and curve-specific transformation plumbing.

use std::sync::Arc;

use crate::Result;
use crate::market_data::bumps::{BumpSpec, Bumpable};
use crate::types::CurveId;

use super::super::term_structures::{
    BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve,
    ParametricCurve, PriceCurve,
};

macro_rules! for_each_context_curve {
    ($macro:ident) => {
        $macro! {
            Discount => { accessor: discount, ty: DiscountCurve, type_name: "Discount" },
            Forward => { accessor: forward, ty: ForwardCurve, type_name: "Forward" },
            Hazard => { accessor: hazard, ty: HazardCurve, type_name: "Hazard" },
            Inflation => { accessor: inflation, ty: InflationCurve, type_name: "Inflation" },
            BaseCorrelation => {
                accessor: base_correlation,
                ty: BaseCorrelationCurve,
                type_name: "BaseCorrelation"
            },
            Price => { accessor: price, ty: PriceCurve, type_name: "Price" },
            Parametric => { accessor: parametric, ty: ParametricCurve, type_name: "Parametric" }
        }
    };
}
pub(crate) use for_each_context_curve;

macro_rules! define_curve_storage {
    ($( $variant:ident => {
        accessor: $accessor:ident,
        ty: $ty:ident,
        type_name: $type_name:literal
    } ),* $(,)?) => {
        /// Unified storage for all curve types using an enum.
        ///
        /// Downstream code rarely manipulates [`CurveStorage`] directly; it mostly
        /// powers [`super::MarketContext`]'s heterogeneous map. When required, the helper
        /// methods expose the inner `Arc` for each concrete curve type.
        #[derive(Clone, Debug)]
        pub enum CurveStorage {
            $(
                #[doc = concat!($type_name, " curve")]
                $variant(Arc<$ty>),
            )*
        }

        impl CurveStorage {
            /// Return the curve's unique identifier.
            pub fn id(&self) -> &CurveId {
                match self {
                    $( Self::$variant(curve) => curve.id(), )*
                }
            }

            $(
                #[doc = concat!("Borrow the ", $type_name, " curve when the variant matches.")]
                pub fn $accessor(&self) -> Option<&Arc<$ty>> {
                    match self {
                        Self::$variant(curve) => Some(curve),
                        _ => None,
                    }
                }
            )*

            /// Return a human-readable curve type (useful for diagnostics/logging).
            ///
            /// A volatility-index [`PriceCurve`] reports `"VolIndex"`, matching
            /// the type name [`MarketContext::get_vol_index_curve`] checks for.
            ///
            /// [`MarketContext::get_vol_index_curve`]: crate::market_data::context::MarketContext::get_vol_index_curve
            pub fn curve_type(&self) -> &'static str {
                match self {
                    Self::Price(curve)
                        if curve.kind()
                            == crate::market_data::term_structures::PriceCurveKind::VolIndex =>
                    {
                        "VolIndex"
                    }
                    $( Self::$variant(_) => $type_name, )*
                }
            }
        }

    };
}

for_each_context_curve!(define_curve_storage);

macro_rules! impl_curve_storage_from {
    ($( $variant:ident => $ty:ty ),* $(,)?) => {
        $(
            impl From<$ty> for CurveStorage {
                fn from(curve: $ty) -> Self {
                    Self::$variant(Arc::new(curve))
                }
            }

            impl From<Arc<$ty>> for CurveStorage {
                fn from(curve: Arc<$ty>) -> Self {
                    Self::$variant(curve)
                }
            }
        )*
    };
}

impl_curve_storage_from!(
    Discount => DiscountCurve,
    Forward => ForwardCurve,
    Hazard => HazardCurve,
    Inflation => InflationCurve,
    BaseCorrelation => BaseCorrelationCurve,
    Parametric => ParametricCurve,
    // Every PriceCurve uses the single Price variant; its kind lives on the curve.
    Price => PriceCurve,
);

impl CurveStorage {
    /// Condition this curve storage to `reference_date`.
    pub(crate) fn condition_storage(&self, reference_date: crate::dates::Date) -> Result<Self> {
        match self {
            Self::Discount(curve) => Ok(Self::Discount(Arc::new(curve.condition(reference_date)?))),
            Self::Forward(curve) => Ok(Self::Forward(Arc::new(curve.condition(reference_date)?))),
            Self::Hazard(curve) => Ok(Self::Hazard(Arc::new(curve.condition(reference_date)?))),
            Self::Parametric(curve) => {
                Ok(Self::Parametric(Arc::new(curve.condition(reference_date)?)))
            }
            Self::Inflation(curve) => {
                let _ = curve.day_count().signed_year_fraction(
                    curve.base_date(),
                    reference_date,
                    curve.day_count_context(),
                )?;
                Ok(Self::Inflation(Arc::clone(curve)))
            }
            Self::Price(curve) => {
                curve.source_time(reference_date)?;
                Ok(Self::Price(Arc::clone(curve)))
            }
            Self::BaseCorrelation(curve) => Ok(Self::BaseCorrelation(Arc::clone(curve))),
        }
    }

    /// Apply a bump to this curve storage, preserving the original ID.
    ///
    /// # Special Cases
    ///
    /// - `InflationCurve` with `TriangularKeyRate` bump: Custom point-level bumping
    ///   that modifies the CPI level at the target bucket.
    pub(crate) fn apply_bump_preserving_id(&mut self, spec: BumpSpec) -> Result<()> {
        spec.validate()?;

        match self {
            Self::Discount(arc) => {
                // In-place bump: Arc::make_mut deep-clones only if refcount > 1
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Forward(arc) => {
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Hazard(arc) => {
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
            Self::Inflation(original) => {
                let curve = original.apply_bump(spec)?;
                *self = Self::Inflation(Arc::new(curve));
                Ok(())
            }
            Self::BaseCorrelation(original) => {
                let curve = original.apply_bump(spec)?;
                *self = Self::BaseCorrelation(Arc::new(curve));
                Ok(())
            }
            Self::Price(original) => {
                let curve = original.apply_bump(spec)?;
                *self = Self::Price(Arc::new(curve));
                Ok(())
            }
            Self::Parametric(arc) => {
                Arc::make_mut(arc).bump_in_place(&spec)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::{Date, DayCount};
    use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
    use serde_json::Value;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).expect("valid test date")
    }

    fn json(curve: &impl serde::Serialize) -> Value {
        serde_json::to_value(curve).expect("curve should serialize")
    }

    #[test]
    fn forward_bump_preserves_interp_and_extrapolation() {
        let curve = ForwardCurve::builder("FWD")
            .base_date(test_date())
            .day_count(DayCount::Act365F)
            .index_tenor(crate::dates::Tenor::quarterly())
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .knots([(0.5, 0.99), (1.0, 0.975), (2.0, 0.94)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.forward().expect("forward curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["index_tenor"], original["index_tenor"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }

    #[test]
    fn inflation_bump_preserves_day_count_interp_and_extrapolation() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .day_count(DayCount::Act360)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots([(0.0, 300.0), (5.0, 325.0), (10.0, 350.0)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(BumpSpec::inflation_shift_pct(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.inflation().expect("inflation curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.day_count(), DayCount::Act360);
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["base_date"], original["base_date"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }

    #[test]
    fn inflation_triangular_key_rate_bump_weights_neighboring_knots() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .day_count(DayCount::Act360)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .knots([
                (0.0, 300.0),
                (2.5, 306.0),
                (5.0, 312.0),
                (7.5, 318.0),
                (10.0, 324.0),
            ])
            .build()
            .expect("curve builds");
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(BumpSpec::triangular_key_rate_bp(0.0, 5.0, 10.0, 100.0))
            .expect("triangular bump succeeds");
        let bumped = storage.inflation().expect("inflation curve");
        let levels = bumped.cpi_levels();
        assert!(
            (levels[0] - 300.0).abs() < 1e-12,
            "left boundary should stay unchanged"
        );
        assert!(
            (levels[4] - 324.0).abs() < 1e-12,
            "right boundary should stay unchanged"
        );
        assert!(
            (levels[2] - 300.0 * ((312.0_f64 / 300.0).powf(1.0 / 5.0) + 0.01).powf(5.0)).abs()
                < 1e-10,
            "target knot should receive the full bump"
        );
        assert!(
            (levels[1] - 300.0 * ((306.0_f64 / 300.0).powf(1.0 / 2.5) + 0.005).powf(2.5)).abs()
                < 1e-10,
            "left neighbor should receive half the bump"
        );
        assert!(
            (levels[3] - 300.0 * ((318.0_f64 / 300.0).powf(1.0 / 7.5) + 0.005).powf(7.5)).abs()
                < 1e-10,
            "right neighbor should receive half the bump"
        );
    }

    #[test]
    fn discount_bump_preserves_interpolator_and_clock() {
        let curve = DiscountCurve::builder("DISC")
            .base_date(test_date())
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots([(0.5, 1.0), (1.0, 1.001), (2.0, 1.002)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.discount().expect("discount curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::Linear);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
    }
}
