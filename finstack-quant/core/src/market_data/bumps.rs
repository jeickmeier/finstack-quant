//! Bump functionality for scenario analysis and stress testing.
//!
//! Provides types and traits for applying parallel shocks and bumps to market
//! data. Used for risk metrics (DV01, CS01), scenario analysis, and regulatory
//! stress tests.
//!
//! # Conventions
//!
//! - Additive rate bumps are normalized into decimal form before application.
//!   For example, `1bp = 0.0001` and `2% = 0.02`.
//! - Discount-curve bumps are **continuously compounded zero-space** shocks
//!   (`DF_bumped(t) = DF(t) · exp(−δr · t)`), not quote re-bootstraps. Bucketed
//!   DV01 from this path will not match hedge-instrument KRDs from a quote
//!   shock plus re-calibration.
//! - Inflation bumps are interpreted in annualized inflation-rate space rather
//!   than as direct CPI-level multipliers.
//! - FX percentage bumps are quoted in percent (`5.0 = +5%`) and strengthen the
//!   base currency against the quote currency.
//! - Bucketed rate bumps use market-standard triangular key-rate weights so the
//!   sum of bucketed DV01s matches the parallel DV01.
//!
//! # References
//!
//! - Key-rate risk methodology: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

use crate::currency::Currency;
use crate::dates::Date;
use crate::types::CurveId;

/// Mode of applying a bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BumpMode {
    /// Additive bump expressed in a normalized fractional form (e.g., 100bp = 0.01, 2% = 0.02).
    Additive,
    /// Multiplicative bump expressed as a factor (e.g., 1.1 = +10%, 0.9 = -10%).
    Multiplicative,
}

/// Type of bump to apply.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BumpType {
    /// Parallel shift across all maturities.
    #[default]
    Parallel,
    /// Triangular key-rate bump with explicit bucket neighbors (market standard).
    ///
    /// This implements the market-standard key-rate DV01 methodology (per Tuckman/Fabozzi)
    /// where the triangular weight is defined by the **bucket grid**, not curve knots.
    /// This ensures that the sum of all bucketed DV01s equals the parallel DV01.
    ///
    /// # Weight Function
    ///
    /// For an **interior** bucket at `target_bucket` with neighbours
    /// `prev_bucket = Some(p)` and `next_bucket = Some(n)`:
    ///
    /// - w(t) = 0                              if t ≤ p
    /// - w(t) = (t − p) / (target − p)         if p < t ≤ target
    /// - w(t) = (n − t) / (n − target)         if target < t < n
    /// - w(t) = 0                              if t ≥ n
    ///
    /// For the **first bucket** (no left neighbour, `prev_bucket = None`)
    /// the rising edge is replaced by a flat 1.0 for `t ≤ target`. For
    /// the **last bucket** (no right neighbour, `next_bucket = None`)
    /// the falling edge is replaced by a flat 1.0 for `t > target`.
    ///
    /// # Key Property
    ///
    /// When the wings are encoded as `None` for the first/last bucket,
    /// the weights of the full bucket set sum to 1.0 at any time `t`
    /// covered by any bucket:
    /// `Σᵢ wᵢ(t) = 1.0`
    ///
    /// This ensures that the sum of bucketed DV01 equals parallel DV01.
    /// Using `Some(0.0)` for the first bucket (instead of `None`) gives a
    /// rising triangle from `t = 0` and **understates** short-end DV01
    /// because it leaves the [0, target_first] interval with sub-unity
    /// total weight.
    TriangularKeyRate {
        /// Previous bucket time in years. `None` for the first bucket
        /// (half-triangle flat to the left of `target_bucket`).
        prev_bucket: Option<f64>,
        /// Target bucket time in years (peak of the triangle).
        target_bucket: f64,
        /// Next bucket time in years. `None` for the last bucket
        /// (half-triangle flat to the right of `target_bucket`).
        next_bucket: Option<f64>,
    },
}

/// Units for the bump magnitude. These control normalization to fraction or factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BumpUnits {
    /// Basis points for rates/spreads (100bp = 0.01).
    RateBp,
    /// Percent units (2.0 = 2%).
    Percent,
    /// Direct fraction (0.02 = 2%).
    Fraction,
    /// Direct factor (1.10 = +10%). Only valid for Multiplicative mode.
    Factor,
}
/// Unified bump specification capturing mode, units, and value.
///
/// # Examples
/// ```rust
/// use finstack_quant_core::market_data::bumps::{BumpSpec, BumpMode, BumpUnits, BumpType};
///
/// let additive = BumpSpec { mode: BumpMode::Additive, units: BumpUnits::RateBp, value: 15.0, bump_type: BumpType::Parallel };
/// assert_eq!(additive.mode, BumpMode::Additive);
/// assert_eq!(additive.units, BumpUnits::RateBp);
///
/// let multiplicative = BumpSpec::multiplier(1.05);
/// assert_eq!(multiplicative.mode, BumpMode::Multiplicative);
/// ```
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BumpSpec {
    /// How the bump should be applied (additive vs multiplicative).
    pub mode: BumpMode,
    /// Units the value is expressed in, controlling normalization.
    pub units: BumpUnits,
    /// Raw magnitude provided by the caller (interpreted using `units`).
    pub value: f64,
    /// Type of bump (parallel or key-rate).
    #[serde(default)]
    pub bump_type: BumpType,
}

impl BumpSpec {
    /// Validate that every numeric input carried by this specification is finite.
    ///
    /// Call this at public mutation boundaries before cloning or mutating market
    /// data so failed bumps cannot contaminate reusable scratch state.
    ///
    /// For a triangular key-rate bump, this includes the target bucket and any
    /// supplied neighbouring bucket coordinates. It deliberately does not
    /// validate unit/mode compatibility; use a product-specific resolver after
    /// this structural check.
    ///
    /// # Errors
    ///
    /// Returns an error if the bump magnitude, target bucket, or a supplied
    /// neighbouring bucket is NaN or infinite.
    pub fn validate_finite(&self) -> crate::Result<()> {
        let buckets_are_finite = match self.bump_type {
            BumpType::Parallel => true,
            BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            } => {
                prev_bucket.is_none_or(f64::is_finite)
                    && target_bucket.is_finite()
                    && next_bucket.is_none_or(f64::is_finite)
            }
        };
        if !self.value.is_finite() || !buckets_are_finite {
            return Err(crate::Error::Validation(
                "BumpSpec values and bucket coordinates must be finite".to_string(),
            ));
        }
        Ok(())
    }

    /// Create an additive bump specified in basis points (e.g., 100.0 = 100bp = 1%).
    pub fn parallel_bp(bump_bp: f64) -> Self {
        Self {
            mode: BumpMode::Additive,
            units: BumpUnits::RateBp,
            value: bump_bp,
            bump_type: BumpType::Parallel,
        }
    }

    /// Create a triangular key-rate bump for an **interior** bucket.
    ///
    /// This is the market-standard implementation (per Tuckman / Fabozzi)
    /// where the triangular weight is defined by the bucket grid. For
    /// interior buckets a full triangle peaks at `target_bucket` and
    /// falls to zero at `prev_bucket` and `next_bucket`.
    ///
    /// Use [`Self::triangular_key_rate_first_bp`] for the first bucket
    /// (no left neighbour) and [`Self::triangular_key_rate_last_bp`]
    /// for the last bucket (no right neighbour) — those constructors
    /// produce half-triangles that preserve the unity-partition
    /// invariant at the wings.
    ///
    /// # Arguments
    /// * `prev_bucket` - Previous bucket time in years
    /// * `target_bucket` - Target bucket time in years (peak of the triangle)
    /// * `next_bucket` - Next bucket time in years
    /// * `bump_bp` - Bump size in basis points (e.g., 1.0 = 1bp)
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::market_data::bumps::BumpSpec;
    ///
    /// // 5Y bucket with neighbours at 3Y and 7Y.
    /// let spec = BumpSpec::triangular_key_rate_bp(3.0, 5.0, 7.0, 1.0);
    /// ```
    pub fn triangular_key_rate_bp(
        prev_bucket: f64,
        target_bucket: f64,
        next_bucket: f64,
        bump_bp: f64,
    ) -> Self {
        Self {
            mode: BumpMode::Additive,
            units: BumpUnits::RateBp,
            value: bump_bp,
            bump_type: BumpType::TriangularKeyRate {
                prev_bucket: Some(prev_bucket),
                target_bucket,
                next_bucket: Some(next_bucket),
            },
        }
    }

    /// Create a triangular key-rate bump for the **first bucket** (no left
    /// neighbour). The weight is flat at 1.0 for `t ≤ target_bucket`,
    /// then linearly tapers to 0 at `next_bucket`.
    ///
    /// Using this constructor (rather than passing `0.0` as `prev_bucket`
    /// to [`Self::triangular_key_rate_bp`]) preserves the
    /// `Σ wᵢ(t) = 1.0` invariant at the short end of the curve. The
    /// rising-triangle convention with `prev = 0.0` understates DV01 for
    /// any knot in `(0, target_first)` because the weight there is
    /// `t / target_first < 1`, leaving the interval with sub-unity total
    /// weight.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::market_data::bumps::BumpSpec;
    ///
    /// // First bucket at 3M, next bucket at 6M, 1bp shock.
    /// let first = BumpSpec::triangular_key_rate_first_bp(0.25, 0.5, 1.0);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `target_bucket` - Risk bucket receiving the requested market bump.
    /// * `next_bucket` - Adjacent higher bucket used to interpolate a triangular bump.
    /// * `bump_bp` - Parallel or bucket bump size expressed in basis points.
    pub fn triangular_key_rate_first_bp(
        target_bucket: f64,
        next_bucket: f64,
        bump_bp: f64,
    ) -> Self {
        Self {
            mode: BumpMode::Additive,
            units: BumpUnits::RateBp,
            value: bump_bp,
            bump_type: BumpType::TriangularKeyRate {
                prev_bucket: None,
                target_bucket,
                next_bucket: Some(next_bucket),
            },
        }
    }

    /// Create a triangular key-rate bump for the **last bucket** (no
    /// right neighbour). The weight rises linearly from 0 at
    /// `prev_bucket` to 1 at `target_bucket`, then stays flat at 1.0
    /// for `t > target_bucket`.
    ///
    /// # Example
    /// ```rust
    /// use finstack_quant_core::market_data::bumps::BumpSpec;
    ///
    /// // Last bucket at 30Y with previous neighbour at 20Y.
    /// let last = BumpSpec::triangular_key_rate_last_bp(20.0, 30.0, 1.0);
    /// ```
    ///
    /// # Arguments
    ///
    /// * `prev_bucket` - Adjacent lower bucket used to interpolate a triangular bump.
    /// * `target_bucket` - Risk bucket receiving the requested market bump.
    /// * `bump_bp` - Parallel or bucket bump size expressed in basis points.
    pub fn triangular_key_rate_last_bp(prev_bucket: f64, target_bucket: f64, bump_bp: f64) -> Self {
        Self {
            mode: BumpMode::Additive,
            units: BumpUnits::RateBp,
            value: bump_bp,
            bump_type: BumpType::TriangularKeyRate {
                prev_bucket: Some(prev_bucket),
                target_bucket,
                next_bucket: None,
            },
        }
    }

    /// Create a multiplicative bump given as a factor (e.g., 1.1 = +10%).
    pub fn multiplier(factor: f64) -> Self {
        Self {
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Factor,
            value: factor,
            bump_type: BumpType::Parallel,
        }
    }

    /// Create an additive inflation shift specified in percent (e.g., 2.0 = +2%).
    pub fn inflation_shift_pct(bump_pct: f64) -> Self {
        Self {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: bump_pct,
            bump_type: BumpType::Parallel,
        }
    }
    /// If additive, return the bump as a normalized fraction (e.g., 100bp -> 0.01, 2% -> 0.02).
    ///
    /// Ensures the bump type is parallel before a product applies it. This
    /// does not restrict the mode or units because supported combinations are
    /// product-specific.
    ///
    /// # Errors
    ///
    /// Returns an error if any numeric bump input is non-finite or the bump is
    /// a triangular key-rate bump rather than [`BumpType::Parallel`]. The
    /// `context` string is included in the unsupported-bump diagnostic.
    pub fn validate_parallel(&self, context: &str) -> crate::Result<()> {
        self.validate_finite()?;
        if !matches!(self.bump_type, BumpType::Parallel) {
            return Err(crate::error::InputError::UnsupportedBump {
                reason: format!("{} only supports Parallel bumps", context),
            }
            .into());
        }
        Ok(())
    }

    /// Resolve standard bump units to a raw magnitude and a flag indicating if it is multiplicative.
    ///
    /// This handles the common logic found in curve bump implementations:
    /// - Additive/RateBp -> value / 10,000, not multiplicative
    /// - Additive/Percent -> value / 100, not multiplicative
    /// - Additive/Fraction -> value, not multiplicative
    /// - Multiplicative/Factor -> value, is multiplicative
    ///
    /// Returns `None` for other combinations (e.g. Multiplicative/Percent which isn't standard everywhere yet).
    pub fn resolve_standard_values(&self) -> Option<(f64, bool)> {
        match (self.mode, self.units) {
            (BumpMode::Additive, BumpUnits::RateBp) => Some((self.value / 10_000.0, false)),
            (BumpMode::Additive, BumpUnits::Percent) => Some((self.value / 100.0, false)),
            (BumpMode::Additive, BumpUnits::Fraction) => Some((self.value, false)),
            (BumpMode::Multiplicative, BumpUnits::Factor) => Some((self.value, true)),
            _ => None,
        }
    }

    pub(crate) fn resolve_standard_values_or_error(
        &self,
        context: &str,
        supported: &str,
    ) -> crate::Result<(f64, bool)> {
        self.validate_finite()?;
        self.resolve_standard_values().ok_or_else(|| {
            crate::error::InputError::UnsupportedBump {
                reason: format!(
                    "{context} {supported}, got {:?}/{:?}",
                    self.mode, self.units
                ),
            }
            .into()
        })
    }

    pub(crate) fn standard_bump_id(&self, id: &CurveId) -> CurveId {
        match self.units {
            BumpUnits::RateBp => id_bump_bp(id.as_str(), self.value),
            BumpUnits::Percent => id_bump_pct(id.as_str(), self.value),
            _ => CurveId::new(format!("{}_bump_{:.4}", id, self.value)),
        }
    }

    pub(crate) fn hazard_shift_id(&self, id: &CurveId) -> CurveId {
        match self.units {
            BumpUnits::RateBp => id_spread_bp(id.as_str(), self.value),
            BumpUnits::Percent => id_bump_pct(id.as_str(), self.value),
            BumpUnits::Fraction => CurveId::new(format!("{}_shift_{:.4}", id, self.value)),
            BumpUnits::Factor => CurveId::new(format!("{}_shift_factor_{:.4}", id, self.value)),
        }
    }
}

/// Unified bump description spanning curves, surfaces, FX, and scalar prices.
///
/// This enum is the heterogeneous input consumed by
/// [`MarketContext::bump`](crate::market_data::context::MarketContext::bump).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketBump {
    /// Standard curve/surface/price bumps addressed by `CurveId`.
    Curve {
        /// Identifier of the curve/surface/price entry.
        id: CurveId,
        /// How to bump the entry (parallel/key-rate, additive/multiplicative).
        spec: BumpSpec,
    },
    /// FX rate percentage shock (positive strengthens the base currency).
    FxPct {
        /// Base currency.
        base: Currency,
        /// Quote currency.
        quote: Currency,
        /// Percentage change (e.g., 5.0 = +5%).
        pct: f64,
        /// Valuation date used for the FX lookup.
        as_of: Date,
    },
    /// Volatility surface bucket bump (percentage multiplier).
    VolBucketPct {
        /// Volatility-surface identifier.
        vol_surface_id: CurveId,
        /// Optional expiry filters (year fractions).
        expiries: Option<Vec<f64>>,
        /// Optional strike filters.
        strikes: Option<Vec<f64>>,
        /// Percentage change to apply to matching buckets.
        pct: f64,
    },
    /// Base correlation bucket bump (additive points).
    BaseCorrBucketPts {
        /// Curve identifier.
        surface_id: CurveId,
        /// Optional detachment filters (percent, e.g., 3.0 for 3%).
        detachments: Option<Vec<f64>>,
        /// Absolute correlation points to add (0.02 = +2 points).
        points: f64,
    },
}

/// Format a bump magnitude for embedding in a derived curve ID.
///
/// Integral values keep the compact `{:.0}` form (`25bp`, `-10bp`); fractional
/// values keep one decimal so distinct bumps yield distinct IDs (`0.4bp` vs
/// `-0.4bp` vs `0bp`). A rounded `-0` is normalized to `0`.
#[inline]
fn format_bump_magnitude(value: f64) -> String {
    let rounded = value.round();
    if (value - rounded).abs() < 1e-9 {
        // Normalize -0.0 so "+0" and "-0" bumps share one ID.
        let normalized = if rounded == 0.0 { 0.0 } else { rounded };
        format!("{:.0}", normalized)
    } else {
        format!("{:.1}", value)
    }
}

#[inline]
pub(crate) fn id_bump_bp(id: &str, bp: f64) -> CurveId {
    CurveId::new(format!("{}_bump_{}bp", id, format_bump_magnitude(bp)))
}

#[inline]
pub(crate) fn id_spread_bp(id: &str, bp: f64) -> CurveId {
    CurveId::new(format!("{}_spread_{}bp", id, format_bump_magnitude(bp)))
}

#[inline]
pub(crate) fn id_bump_pct(id: &str, pct: f64) -> CurveId {
    CurveId::new(format!("{}_bump_{}pct", id, format_bump_magnitude(pct)))
}

/// Trait for types that can be bumped with a BumpSpec.
///
/// This trait provides a uniform interface for applying market data bumps
/// (parallel shifts, key-rate bumps, etc.) across different curve and surface types.
///
/// # Error Handling
///
/// Returns `Err` with a descriptive message when:
/// - The bump type is not supported for this curve type
/// - The mode/units combination is invalid
/// - The curve reconstruction fails after applying the bump
/// - Input validation fails (e.g., invalid recovery rate for hazard curves)
pub trait Bumpable: Sized + Send + Sync {
    /// Apply a bump specification to create a new bumped instance.
    ///
    /// # Arguments
    ///
    /// * `spec` - Shock to apply: mode, units, magnitude, and parallel versus
    ///   triangular key-rate shape. Supported combinations are type-specific;
    ///   see each implementor for the accepted unit/mode pairs and bump space.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::UnsupportedBump`](crate::error::InputError::UnsupportedBump)
    /// if the bump operation is not supported for this type.
    fn apply_bump(&self, spec: BumpSpec) -> crate::Result<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market_data::scalars::MarketScalar;

    #[test]
    fn test_resolve_standard_values() {
        // Additive RateBp (divided by 10,000)
        let spec = BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::RateBp,
            value: 50.0,
            bump_type: BumpType::Parallel,
        };
        assert_eq!(spec.resolve_standard_values(), Some((0.0050, false)));

        // Additive Percent (divided by 100)
        let spec = BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: 2.0,
            bump_type: BumpType::Parallel,
        };
        assert_eq!(spec.resolve_standard_values(), Some((0.02, false)));

        // Additive Fraction (raw value)
        let spec = BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: 0.05,
            bump_type: BumpType::Parallel,
        };
        assert_eq!(spec.resolve_standard_values(), Some((0.05, false)));

        // Multiplicative Factor (raw value, is_multiplicative=true)
        let spec = BumpSpec {
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Factor,
            value: 1.10,
            bump_type: BumpType::Parallel,
        };
        assert_eq!(spec.resolve_standard_values(), Some((1.10, true)));

        // Unsupported combination (Multiplicative/Percent) -> None
        let spec = BumpSpec {
            mode: BumpMode::Multiplicative,
            units: BumpUnits::Percent,
            value: 10.0,
            bump_type: BumpType::Parallel,
        };
        assert_eq!(spec.resolve_standard_values(), None);
    }

    #[test]
    fn test_market_scalar_price_fraction_bump_is_absolute() -> crate::Result<()> {
        let price = MarketScalar::Price(crate::money::Money::from((
            100_i64,
            crate::currency::Currency::USD,
        )));
        let bumped = price.apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: 2.5,
            bump_type: BumpType::Parallel,
        })?;

        let MarketScalar::Price(m) = bumped else {
            return Err(crate::Error::Validation(
                "expected MarketScalar::Price result".to_string(),
            ));
        };
        assert!((m.amount() - 102.5).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_market_scalar_price_percent_bump_is_proportional() -> crate::Result<()> {
        let price = MarketScalar::Price(crate::money::Money::from((
            100_i64,
            crate::currency::Currency::USD,
        )));
        let bumped = price.apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: 2.5,
            bump_type: BumpType::Parallel,
        })?;

        let MarketScalar::Price(m) = bumped else {
            return Err(crate::Error::Validation(
                "expected MarketScalar::Price result".to_string(),
            ));
        };
        assert!((m.amount() - 102.5).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_forward_curve_bump() -> crate::Result<()> {
        use crate::dates::Date;
        use crate::market_data::term_structures::ForwardCurve;
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 1).map_err(|_| {
            crate::error::InputError::InvalidDate {
                year: 2025,
                month: 1,
                day: 1,
            }
        })?;
        let height_check = 0.04; // rate at 1.0 (knot)

        let fc = ForwardCurve::builder("USD-TEST-3M", 0.25)
            .base_date(base)
            .knots([(0.5, 0.038), (1.0, height_check)]) // Minimum 2 points required
            .build()?;

        // 1. Additive RateBp
        let spec = BumpSpec::parallel_bp(10.0); // +10bps = +0.0010
        let bumped = fc.apply_bump(spec)?;
        assert!((bumped.rate(1.0) - 0.0410).abs() < 1e-12);

        // 2. Additive Percent
        let spec_pct = BumpSpec::inflation_shift_pct(0.5); // Using helper, treated as Additive Percent
                                                           // ForwardCurve generic logic handles Additive/Percent -> value/100
                                                           // So 0.5 -> 0.005. rate = 0.04 + 0.005 = 0.045
        let bumped_pct = fc.apply_bump(spec_pct)?;
        assert!((bumped_pct.rate(1.0) - 0.045).abs() < 1e-12);

        // 3. Multiplicative Factor
        let spec_mul = BumpSpec::multiplier(1.10); // +10%
        let bumped_mul = fc.apply_bump(spec_mul)?;
        assert!((bumped_mul.rate(1.0) - 0.044).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_hazard_curve_bump() -> crate::Result<()> {
        use crate::dates::Date;
        use crate::market_data::term_structures::HazardCurve;
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 1).map_err(|_| {
            crate::error::InputError::InvalidDate {
                year: 2025,
                month: 1,
                day: 1,
            }
        })?;
        let hc = HazardCurve::builder("CDS-TEST")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.02)]) // lambda = 0.02
            .build()?;

        // Additive RateBp
        // shift = 10bps / (1 - R) = 0.0010 / 0.6 = 0.001666...
        let spec = BumpSpec::parallel_bp(10.0);
        let bumped = hc.apply_bump(spec)?;
        let expected_lambda = 0.02 + (0.0010 / 0.60);
        assert!((bumped.hazard_rate(1.0) - expected_lambda).abs() < 1e-10);

        // Verify Multiplicative raises error
        let spec_mul = BumpSpec::multiplier(1.10);
        assert!(hc.apply_bump(spec_mul).is_err());

        Ok(())
    }

    #[test]
    fn test_hazard_curve_negative_bump_errors() -> crate::Result<()> {
        // A down-bump that would push a hazard rate negative now fails loudly
        // instead of clamping to zero (clamping made two-sided CS01 silently
        // asymmetric for tight names) — unified with `bump_in_place`; see
        //
        use crate::dates::Date;
        use crate::market_data::term_structures::HazardCurve;
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 1).map_err(|_| {
            crate::error::InputError::InvalidDate {
                year: 2025,
                month: 1,
                day: 1,
            }
        })?;
        let hc = HazardCurve::builder("CDS-CLAMP")
            .base_date(base)
            .recovery_rate(0.40)
            .knots([(1.0, 0.0010)])
            .build()?;

        let spec = BumpSpec::parallel_bp(-20.0);
        let err = hc.apply_bump(spec).expect_err("negative hazard must error");
        assert!(
            err.to_string().contains("negative hazard rate after bump"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn test_discount_curve_bump() -> crate::Result<()> {
        use crate::dates::Date;
        use crate::market_data::term_structures::DiscountCurve;
        use time::Month;

        let base = Date::from_calendar_date(2025, Month::January, 1).map_err(|_| {
            crate::error::InputError::InvalidDate {
                year: 2025,
                month: 1,
                day: 1,
            }
        })?;
        // Flat curve: 5% continuously compounded -> DF(1) = exp(-0.05) ≈ 0.951229
        let day_count = DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots([(0.5, 0.975309912), (1.0, 0.9512294245)]) // Minimum 2 points
            .build()?;

        // 1. Additive RateBp
        let spec = BumpSpec::parallel_bp(100.0); // +100bps = +1%
        let bumped = day_count.apply_bump(spec)?;
        // New rate = 5% + 1% = 6%
        // DF(1) = exp(-0.06) ≈ 0.9417645336
        assert!((bumped.df(1.0) - 0.9417645336).abs() < 1e-8);

        // 2. Additive Percent (New capability!)
        // 1% additive bump (same as 100bp)
        let spec_pct = BumpSpec::inflation_shift_pct(1.0);
        let bumped_pct = day_count.apply_bump(spec_pct)?;
        assert!((bumped_pct.df(1.0) - 0.9417645336).abs() < 1e-8);

        // 3. Verify Multiplicative raises error
        let spec_mul = BumpSpec::multiplier(1.10);
        assert!(day_count.apply_bump(spec_mul).is_err());

        Ok(())
    }
}
