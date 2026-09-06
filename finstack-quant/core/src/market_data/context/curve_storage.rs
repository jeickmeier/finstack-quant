//! Heterogeneous curve storage and curve-specific transformation plumbing.

use crate::market_data::bumps::Bumpable;
use std::sync::Arc;

use crate::types::CurveId;
use crate::Result;

use super::super::term_structures::{
    BaseCorrelationCurve, BasisSpreadCurve, DiscountCurve, ForwardCurve, HazardCurve,
    InflationCurve, ParametricCurve, PriceCurve, PriceCurveKind,
};
use super::BumpSpec;

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
            VolIndex => { accessor: vol_index, ty: PriceCurve, type_name: "VolIndex" },
            BasisSpread => { accessor: basis_spread, ty: BasisSpreadCurve, type_name: "BasisSpread" },
            Parametric => { accessor: parametric, ty: ParametricCurve, type_name: "Parametric" }
        }
    };
}
pub(crate) use for_each_context_curve;

/// Trait for curves that can be rebuilt with a new ID while preserving all other data.
///
/// This is used during market bumping operations where the bump produces a curve
/// with a modified ID (e.g., "USD-OIS_bump_+10bp") but we want to keep the original ID.
trait RebuildableWithId: Sized {
    /// Rebuild the curve with a new ID, preserving all other data.
    fn rebuild_with_id(&self, id: CurveId) -> Result<Self>;
}

macro_rules! impl_simple_rebuildable_with_id {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RebuildableWithId for $ty {
                fn rebuild_with_id(&self, id: CurveId) -> Result<Self> {
                    self.to_builder_with_id(id).build()
                }
            }
        )*
    };
}

impl_simple_rebuildable_with_id!(
    DiscountCurve,
    ForwardCurve,
    HazardCurve,
    InflationCurve,
    PriceCurve,
    BasisSpreadCurve,
    ParametricCurve,
);

impl RebuildableWithId for BaseCorrelationCurve {
    fn rebuild_with_id(&self, id: CurveId) -> Result<Self> {
        BaseCorrelationCurve::builder(id)
            .knots(
                self.detachment_points()
                    .iter()
                    .copied()
                    .zip(self.correlations().iter().copied()),
            )
            .build()
    }
}

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
            pub fn curve_type(&self) -> &'static str {
                match self {
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
    BasisSpread => BasisSpreadCurve,
    Parametric => ParametricCurve,
);

/// A [`PriceCurve`] lands in the `Price` or `VolIndex` slot according to its
/// [`PriceCurveKind`], so `get_price_curve` / `get_vol_index_curve` stay
/// type-checked lookups.
impl From<Arc<PriceCurve>> for CurveStorage {
    fn from(curve: Arc<PriceCurve>) -> Self {
        match curve.kind() {
            PriceCurveKind::Price => Self::Price(curve),
            PriceCurveKind::VolIndex => Self::VolIndex(curve),
        }
    }
}

impl From<PriceCurve> for CurveStorage {
    fn from(curve: PriceCurve) -> Self {
        Arc::new(curve).into()
    }
}

impl CurveStorage {
    /// Roll this curve storage forward by the provided number of days.
    pub(crate) fn roll_forward_storage(&self, days: i64) -> Result<Self> {
        match self {
            Self::Discount(curve) => Ok(Self::Discount(Arc::new(curve.roll_forward(days)?))),
            Self::Forward(curve) => Ok(Self::Forward(Arc::new(curve.roll_forward(days)?))),
            Self::Hazard(curve) => Ok(Self::Hazard(Arc::new(curve.roll_forward(days)?))),
            Self::Inflation(curve) => Ok(Self::Inflation(Arc::new(curve.roll_forward(days)?))),
            Self::BaseCorrelation(curve) => Ok(Self::BaseCorrelation(Arc::clone(curve))),
            Self::Price(curve) => Ok(Self::Price(Arc::new(curve.roll_forward(days)?))),
            Self::VolIndex(curve) => Ok(Self::VolIndex(Arc::new(curve.roll_forward(days)?))),
            Self::BasisSpread(curve) => Ok(Self::BasisSpread(Arc::new(curve.roll_forward(days)?))),
            Self::Parametric(curve) => Ok(Self::Parametric(Arc::clone(curve))),
        }
    }

    /// Apply a bump to this curve storage, preserving the original ID.
    ///
    /// After bumping, if the bumped curve has a different ID (e.g., "USD-OIS_bump_+10bp"),
    /// it is rebuilt with the original ID to maintain context consistency.
    ///
    /// # Special Cases
    ///
    /// - `InflationCurve` with `TriangularKeyRate` bump: Custom point-level bumping
    ///   that modifies the CPI level at the target bucket.
    pub(crate) fn apply_bump_preserving_id(
        &mut self,
        original_id: &CurveId,
        spec: BumpSpec,
    ) -> Result<()> {
        spec.validate_finite()?;

        fn bump_curve_preserving_id<C>(
            original: &C,
            original_id: &CurveId,
            spec: BumpSpec,
            id_of: fn(&C) -> &CurveId,
        ) -> Result<C>
        where
            C: Bumpable + RebuildableWithId,
        {
            let bumped = original.apply_bump(spec)?;
            if id_of(&bumped) != original_id {
                bumped.rebuild_with_id(original_id.clone())
            } else {
                Ok(bumped)
            }
        }

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
                let curve = bump_curve_preserving_id(
                    original.as_ref(),
                    original_id,
                    spec,
                    InflationCurve::id,
                )?;
                *self = Self::Inflation(Arc::new(curve));
                Ok(())
            }
            Self::BaseCorrelation(original) => {
                let curve = bump_curve_preserving_id(
                    original.as_ref(),
                    original_id,
                    spec,
                    BaseCorrelationCurve::id,
                )?;
                *self = Self::BaseCorrelation(Arc::new(curve));
                Ok(())
            }
            Self::VolIndex(original) => {
                let curve =
                    bump_curve_preserving_id(original.as_ref(), original_id, spec, PriceCurve::id)?;
                *self = Self::VolIndex(Arc::new(curve));
                Ok(())
            }
            Self::Price(original) => {
                let curve =
                    bump_curve_preserving_id(original.as_ref(), original_id, spec, PriceCurve::id)?;
                *self = Self::Price(Arc::new(curve));
                Ok(())
            }
            Self::BasisSpread(_) => Err(crate::error::InputError::UnsupportedBump {
                reason: "BasisSpreadCurve does not support bumping".to_string(),
            }
            .into()),
            Self::Parametric(_) => Err(crate::error::InputError::UnsupportedBump {
                reason: "ParametricCurve does not support bumping".to_string(),
            }
            .into()),
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
        let curve = ForwardCurve::builder("FWD", 0.25)
            .base_date(test_date())
            .reset_lag(0)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .knots([(0.5, 0.02), (1.0, 0.025), (2.0, 0.03)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("FWD"), BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.forward().expect("forward curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["reset_lag"], original["reset_lag"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }

    #[test]
    fn inflation_bump_preserves_lag_day_count_and_interp() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .base_cpi(300.0)
            .day_count(DayCount::Act360)
            .indexation_lag_months(2)
            .interp(InterpStyle::LogLinear)
            .knots([(0.0, 300.0), (5.0, 325.0), (10.0, 350.0)])
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("CPI"), BumpSpec::inflation_shift_pct(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.inflation().expect("inflation curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.day_count(), DayCount::Act360);
        assert_eq!(bumped_curve.indexation_lag_months(), 2);
        assert_eq!(bumped_curve.interp_style(), InterpStyle::LogLinear);
        assert_eq!(bumped_json["base_date"], original["base_date"]);
        assert_eq!(bumped_json["day_count"], original["day_count"]);
        assert_eq!(
            bumped_json["indexation_lag_months"],
            original["indexation_lag_months"]
        );
        assert_eq!(bumped_json["interp_style"], original["interp_style"]);
        assert_eq!(bumped_json["extrapolation"], original["extrapolation"]);
    }

    #[test]
    fn inflation_triangular_key_rate_bump_weights_neighboring_knots() {
        let curve = InflationCurve::builder("CPI")
            .base_date(test_date())
            .base_cpi(300.0)
            .day_count(DayCount::Act360)
            .indexation_lag_months(3)
            .interp(InterpStyle::Linear)
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
            .apply_bump_preserving_id(
                &CurveId::from("CPI"),
                BumpSpec::triangular_key_rate_bp(0.0, 5.0, 10.0, 100.0),
            )
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
    fn discount_bump_preserves_forward_controls() {
        let curve = DiscountCurve::builder("DISC")
            .base_date(test_date())
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .knots([(0.5, 1.0), (1.0, 1.001), (2.0, 1.002)])
            .validation(
                crate::market_data::term_structures::ValidationMode::NegativeRateFriendly {
                    forward_floor: -0.05,
                },
            )
            .min_forward_tenor(1e-8)
            .build()
            .expect("curve builds");
        let original = json(&curve);
        let mut storage = CurveStorage::from(curve);
        storage
            .apply_bump_preserving_id(&CurveId::from("DISC"), BumpSpec::parallel_bp(1.0))
            .expect("bump succeeds");
        let bumped_curve = storage.discount().expect("discount curve");
        let bumped_json = json(bumped_curve.as_ref());
        assert_eq!(bumped_curve.interp_style(), InterpStyle::Linear);
        assert_eq!(
            bumped_json["allow_non_monotonic"],
            original["allow_non_monotonic"]
        );
        assert_eq!(
            bumped_json["min_forward_rate"],
            original["min_forward_rate"]
        );
        assert_eq!(
            bumped_json["min_forward_tenor"],
            original["min_forward_tenor"]
        );
    }
}
