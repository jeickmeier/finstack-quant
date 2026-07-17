use super::traits::FactorSensitivityEngine;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, InputError, Result};
use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_quant_factor_model::{
    BumpSizeConfig, FactorBumpUnit, FactorDefinition, MarketMapping,
};
use finstack_quant_valuations::instruments::Instrument;

use rayon::prelude::*;

/// Finite-difference sensitivity engine using central bumps around the base market.
#[derive(Debug, Clone)]
pub struct DeltaBasedEngine {
    bump_config: BumpSizeConfig,
}

impl DeltaBasedEngine {
    /// Create a new delta-based engine with the provided bump configuration.
    #[must_use]
    pub fn new(bump_config: BumpSizeConfig) -> Self {
        Self { bump_config }
    }

    fn compute_factor_column(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factor: &FactorDefinition,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<f64>> {
        let (bump_size, bump_unit) = self
            .bump_config
            .bump_size_with_unit_for_factor(&factor.id, &factor.factor_type);
        if !bump_size.is_finite() || bump_size.abs() < f64::EPSILON {
            return Err(InputError::Invalid.into());
        }

        let up_market = market.bump(mapping_to_market_bumps(
            &factor.market_mapping,
            bump_size,
            bump_unit,
            as_of,
        )?)?;
        let down_market = market.bump(mapping_to_market_bumps(
            &factor.market_mapping,
            -bump_size,
            bump_unit,
            as_of,
        )?)?;

        positions
            .iter()
            .map(|(position_id, instrument, weight)| {
                let pv_up = instrument.value_raw(&up_market, as_of)?;
                let pv_down = instrument.value_raw(&down_market, as_of)?;
                if !pv_up.is_finite() || !pv_down.is_finite() {
                    return Err(Error::Validation(format!(
                        "minor 15: non-finite bumped PV for position '{position_id}' on factor '{}' (up = {pv_up}, down = {pv_down})",
                        factor.id.as_str()
                    )));
                }
                Ok((pv_up - pv_down) / (2.0 * bump_size) * *weight)
            })
            .collect()
    }
}

/// Convert a `MarketMapping` into concrete `MarketBump`s that can be applied
/// to a market.
///
/// `bump_size` is the magnitude; `bump_unit` tags its interpretation
/// ([basis points](FactorBumpUnit::BasisPoint), [percent](FactorBumpUnit::Percent),
/// [absolute](FactorBumpUnit::Absolute), etc.). Making the unit
/// explicit prevents silent 100× errors where a rates-bp magnitude is
/// fed into the `EquitySpot` branch (which assumes percent), or vice
/// versa.
///
/// Branches that already carry their own `BumpUnits` (`CurveParallel`,
/// `CurveBucketed`, `VolShift`) require `bump_unit` to match the mapping's
/// declared `BumpUnits`; mismatches are rejected with
/// [`Error::Validation`] so the disagreement surfaces at bump construction
/// rather than propagating a silently mis-scaled shock.
///
/// `EquitySpot` and `FxRate` do not carry `BumpUnits` and use
/// `bump_unit.to_fraction(...)` to reduce any input unit to a common
/// fractional form before applying the multiplier / fxpct shock.
pub fn mapping_to_market_bumps(
    mapping: &MarketMapping,
    bump_size: f64,
    bump_unit: FactorBumpUnit,
    as_of: Date,
) -> Result<Vec<MarketBump>> {
    use finstack_quant_core::market_data::bumps::BumpUnits;

    // Enforce that `bump_unit` matches the mapping-level `BumpUnits` for
    // branches that declare one. This catches the misconfigured
    // rates-factor-into-percent-mapping case at bump construction time
    // instead of at P&L reconciliation.
    let require_unit_matches = |mapping_units: BumpUnits| -> Result<()> {
        let ok = matches!(
            (bump_unit, mapping_units),
            (FactorBumpUnit::BasisPoint, BumpUnits::RateBp)
                | (FactorBumpUnit::Percent, BumpUnits::Percent)
                | (FactorBumpUnit::Fraction, BumpUnits::Fraction)
                | (FactorBumpUnit::Multiplier, BumpUnits::Factor)
                // Absolute vol-points flow through as fractional magnitude
                // for VolShift surfaces that store fractional vols.
                | (FactorBumpUnit::Absolute, BumpUnits::Fraction)
        );
        if ok {
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "FactorBumpUnit::{bump_unit:?} incompatible with MarketMapping units {mapping_units:?}"
            )))
        }
    };

    match mapping {
        MarketMapping::CurveParallel { curve_ids, units } => {
            require_unit_matches(*units)?;
            Ok(curve_ids
                .iter()
                .cloned()
                .map(|id| MarketBump::Curve {
                    id,
                    spec: BumpSpec {
                        mode: BumpMode::Additive,
                        units: *units,
                        value: bump_size,
                        bump_type: BumpType::Parallel,
                    },
                })
                .collect())
        }
        MarketMapping::CurveBucketed {
            curve_id,
            tenor_weights,
        } => {
            // Bucketed bumps are always bp-scaled per
            // `BumpSpec::triangular_key_rate_bp`.
            if !matches!(bump_unit, FactorBumpUnit::BasisPoint) {
                return Err(Error::Validation(format!(
                    "MarketMapping::CurveBucketed requires BasisPoint bump_unit, got {bump_unit:?}"
                )));
            }
            let last_idx = tenor_weights.len().saturating_sub(1);
            Ok(tenor_weights
                .iter()
                .enumerate()
                .map(|(idx, &(target_bucket, weight))| {
                    // Use half-triangle wings (prev = None / next = None) at
                    // the first and last buckets so the bump-set partitions
                    // unity across the full curve and `Σ bucketed_dv01`
                    // matches the parallel bump. A finite `prev = 0.0` at
                    // the first bucket would produce a rising triangle from
                    // t=0 and understate short-end sensitivity.
                    let scaled_bp = bump_size * weight;
                    let spec = match (idx == 0, idx == last_idx) {
                        (true, true) => {
                            // Sole bucket: both wings flat ⇒ weight is 1 at every
                            // knot, equivalent to a parallel bump.
                            BumpSpec::parallel_bp(scaled_bp)
                        }
                        (true, false) => BumpSpec::triangular_key_rate_first_bp(
                            target_bucket,
                            tenor_weights[idx + 1].0,
                            scaled_bp,
                        ),
                        (false, true) => BumpSpec::triangular_key_rate_last_bp(
                            tenor_weights[idx - 1].0,
                            target_bucket,
                            scaled_bp,
                        ),
                        (false, false) => BumpSpec::triangular_key_rate_bp(
                            tenor_weights[idx - 1].0,
                            target_bucket,
                            tenor_weights[idx + 1].0,
                            scaled_bp,
                        ),
                    };
                    MarketBump::Curve {
                        id: curve_id.clone(),
                        spec,
                    }
                })
                .collect())
        }
        MarketMapping::EquitySpot { tickers } => {
            // Equity spot expects a *multiplicative* shock. Convert any
            // input unit to a fractional shift first, then wrap as
            // `1.0 + fraction`. This removes the previous hardcoded
            // `bump_size / 100.0` that assumed percent-only input.
            let fraction = bump_unit.to_fraction(bump_size);
            Ok(tickers
                .iter()
                .map(|ticker| MarketBump::Curve {
                    id: CurveId::new(ticker),
                    spec: BumpSpec::multiplier(1.0 + fraction),
                })
                .collect())
        }
        MarketMapping::FxRate { pair } => {
            // `MarketBump::FxPct::pct` is a *percent* scalar, so convert
            // any input unit to percent (fraction × 100) for consistency.
            let pct = bump_unit.to_fraction(bump_size) * 100.0;
            Ok(vec![MarketBump::FxPct {
                base: pair.0,
                quote: pair.1,
                pct,
                as_of,
            }])
        }
        MarketMapping::VolShift { surface_ids, units } => {
            require_unit_matches(*units)?;
            Ok(surface_ids
                .iter()
                .map(|surface_id| MarketBump::Curve {
                    id: CurveId::new(surface_id),
                    spec: BumpSpec {
                        mode: BumpMode::Additive,
                        units: *units,
                        value: bump_size,
                        bump_type: BumpType::Parallel,
                    },
                })
                .collect())
        }
    }
}

impl FactorSensitivityEngine for DeltaBasedEngine {
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix> {
        super::traits::validate_single_currency(positions, market, as_of)?;
        let position_ids = positions.iter().map(|(id, _, _)| id.clone()).collect();
        let factor_ids = factors.iter().map(|factor| factor.id.clone()).collect();
        let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);

        let columns: Result<Vec<Vec<f64>>> = factors
            .par_iter()
            .map(|factor| self.compute_factor_column(positions, factor, market, as_of))
            .collect();

        for (factor_idx, column) in columns?.iter().enumerate() {
            for (position_idx, value) in column.iter().enumerate() {
                matrix.set_delta(position_idx, factor_idx, *value);
            }
        }

        Ok(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::Attributes;
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use time::macros::date;

    #[derive(Clone)]
    enum MockKind {
        CurveZero { curve_id: CurveId, tenor_years: f64 },
        Spot { spot_id: String },
        NonFiniteRaw,
    }

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        kind: MockKind,
        scale: f64,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        MockInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl MockInstrument {
        fn curve_zero(id: &str, curve_id: &str, tenor_years: f64, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                kind: MockKind::CurveZero {
                    curve_id: CurveId::new(curve_id),
                    tenor_years,
                },
                scale,
            }
        }

        fn spot(id: &str, spot_id: &str, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                kind: MockKind::Spot {
                    spot_id: spot_id.to_string(),
                },
                scale,
            }
        }

        fn non_finite_raw(id: &str) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                kind: MockKind::NonFiniteRaw,
                scale: 1.0,
            }
        }

        fn raw_value(&self, market: &MarketContext) -> Result<f64> {
            match &self.kind {
                MockKind::CurveZero {
                    curve_id,
                    tenor_years,
                } => Ok(market.get_discount(curve_id.as_str())?.zero(*tenor_years) * self.scale),
                MockKind::Spot { spot_id } => {
                    let price = market.get_price(spot_id)?.clone();
                    let value = match price {
                        MarketScalar::Unitless(v) => v,
                        MarketScalar::Price(money) => money.amount(),
                    };
                    Ok(value * self.scale)
                }
                MockKind::NonFiniteRaw => Ok(f64::NAN),
            }
        }
    }

    impl Instrument for MockInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
            let amount = if matches!(self.kind, MockKind::NonFiniteRaw) {
                1.0
            } else {
                self.raw_value(market)?
            };
            Ok(Money::new(amount, Currency::USD))
        }

        fn base_value_raw(&self, market: &MarketContext, _as_of: Date) -> Result<f64> {
            self.raw_value(market)
        }
    }

    fn test_market(as_of: Date) -> Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .interp(InterpStyle::MonotoneConvex)
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.80), (10.0, 0.60)])
            .build()?;

        Ok(MarketContext::new()
            .insert(curve)
            .insert_price("SPOT", MarketScalar::Unitless(100.0)))
    }

    #[test]
    fn test_mapping_to_market_bumps_curve_parallel() -> Result<()> {
        let mapping = MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS")],
            units: BumpUnits::RateBp,
        };

        let bumps = mapping_to_market_bumps(
            &mapping,
            1.0,
            FactorBumpUnit::BasisPoint,
            date!(2025 - 01 - 01),
        )?;

        assert_eq!(bumps.len(), 1);
        assert!(matches!(bumps[0], MarketBump::Curve { .. }));
        if let MarketBump::Curve { id, spec } = &bumps[0] {
            assert_eq!(id.as_str(), "USD-OIS");
            assert_eq!(spec.value, 1.0);
            assert_eq!(spec.units, BumpUnits::RateBp);
        }
        Ok(())
    }

    #[test]
    fn test_mapping_to_market_bumps_curve_bucketed() -> Result<()> {
        let mapping = MarketMapping::CurveBucketed {
            curve_id: CurveId::new("USD-OIS"),
            tenor_weights: vec![(2.0, 0.5), (5.0, 1.0), (10.0, 0.5)],
        };

        let bumps = mapping_to_market_bumps(
            &mapping,
            1.0,
            FactorBumpUnit::BasisPoint,
            date!(2025 - 01 - 01),
        )?;

        assert_eq!(bumps.len(), 3);
        assert!(matches!(bumps[1], MarketBump::Curve { .. }));
        if let MarketBump::Curve { spec, .. } = &bumps[1] {
            assert_eq!(spec.value, 1.0);
            assert_eq!(
                spec.bump_type,
                BumpType::TriangularKeyRate {
                    prev_bucket: Some(2.0),
                    target_bucket: 5.0,
                    next_bucket: Some(10.0),
                }
            );
        }
        Ok(())
    }

    /// `mapping_to_market_bumps` must reject a factor-unit that
    /// disagrees with the mapping's declared `BumpUnits`, so a
    /// rates-bp magnitude routed into a percent-denominated mapping
    /// fails at bump construction instead of scaling the shock 100×
    /// silently.
    #[test]
    fn mapping_rejects_bump_unit_mismatch_on_curve_parallel() -> Result<()> {
        let mapping = MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS")],
            units: BumpUnits::RateBp,
        };
        let result = mapping_to_market_bumps(
            &mapping,
            1.0,
            FactorBumpUnit::Percent,
            date!(2025 - 01 - 01),
        );
        assert!(result.is_err(), "unit mismatch must be rejected");
        let msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("FactorBumpUnit") && msg.contains("incompatible"),
            "error should name the offending unit: {msg}"
        );
        Ok(())
    }

    /// `EquitySpot` must accept any unit and convert to a fractional
    /// multiplier, so a 2 % shock and a 0.02 fractional shock produce
    /// the same `1.02` multiplier (closing the old hardcoded
    /// `bump_size / 100.0` assumption that required callers to speak
    /// percent).
    #[test]
    fn equity_spot_converts_any_unit_to_fractional_multiplier() -> Result<()> {
        let mapping = MarketMapping::EquitySpot {
            tickers: vec!["SPX".into()],
        };
        let pct = mapping_to_market_bumps(
            &mapping,
            2.0,
            FactorBumpUnit::Percent,
            date!(2025 - 01 - 01),
        )?;
        let frac = mapping_to_market_bumps(
            &mapping,
            0.02,
            FactorBumpUnit::Fraction,
            date!(2025 - 01 - 01),
        )?;
        // Both should yield a multiplier bump of 1.02.
        for bumps in [&pct, &frac] {
            assert_eq!(bumps.len(), 1);
            let spec = match &bumps[0] {
                MarketBump::Curve { spec, .. } => spec,
                other => {
                    return Err(Error::Validation(format!(
                        "expected Curve bump, got {other:?}"
                    )))
                }
            };
            assert!(
                (spec.value - 1.02).abs() < 1e-12,
                "expected 1.02 multiplier, got {}",
                spec.value
            );
        }
        Ok(())
    }

    #[test]
    fn test_delta_based_engine_curve_parallel_sensitivity() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::curve_zero("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: finstack_quant_factor_model::FactorId::new("rates"),
            factor_type: finstack_quant_factor_model::FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let matrix = DeltaBasedEngine::new(BumpSizeConfig::default())
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert_eq!(matrix.n_positions(), 1);
        assert_eq!(matrix.n_factors(), 1);
        assert!((matrix.delta(0, 0) - 1.0).abs() < 1e-3);
        Ok(())
    }

    #[test]
    fn minor15_delta_engine_rejects_non_finite_bumped_pv() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::non_finite_raw("bad-inst");
        let positions = vec![("bad-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: finstack_quant_factor_model::FactorId::new("rates"),
            factor_type: finstack_quant_factor_model::FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let err = DeltaBasedEngine::new(BumpSizeConfig::default())
            .compute_sensitivities(&positions, &factors, &market, as_of)
            .expect_err("minor 15: non-finite bumped PV must fail fast");
        assert!(
            err.to_string().contains("minor 15"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn test_delta_based_engine_equity_spot_sensitivity() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::spot("spot-inst", "SPOT", 1.0);
        let positions = vec![("spot-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: finstack_quant_factor_model::FactorId::new("equity"),
            factor_type: finstack_quant_factor_model::FactorType::Equity,
            market_mapping: MarketMapping::EquitySpot {
                tickers: vec!["SPOT".to_string()],
            },
            description: None,
        }];

        let matrix = DeltaBasedEngine::new(BumpSizeConfig::default())
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert!((matrix.delta(0, 0) - 1.0).abs() < 1e-9);
        Ok(())
    }
}
