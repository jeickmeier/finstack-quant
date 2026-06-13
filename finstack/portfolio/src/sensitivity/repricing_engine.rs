use super::delta_engine::mapping_to_market_bumps;
use super::traits::FactorSensitivityEngine;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::{Error, Result};
use finstack_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_factor_model::{BumpSizeConfig, FactorDefinition, FactorId};
use finstack_valuations::instruments::Instrument;

/// P&L profile for one factor across a scenario grid.
#[derive(Debug, Clone, PartialEq)]
pub struct FactorPnlProfile {
    /// Identifier of the shocked factor.
    pub factor_id: FactorId,
    /// Scenario shift coordinates in bump-size units.
    pub shifts: Vec<f64>,
    /// Per-shift P&L vectors indexed as `[shift_idx][position_idx]`.
    pub position_pnls: Vec<Vec<f64>>,
}

/// Symmetric grid of scenario shifts used by the full repricing engine.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioGrid {
    shifts: Vec<f64>,
}

impl ScenarioGrid {
    /// Minimum number of grid points required for central-difference delta
    /// extraction (need at least -1, 0, +1).
    pub const MIN_POINTS: usize = 3;

    /// Create a grid centered on zero, e.g. `5 -> [-2, -1, 0, 1, 2]`.
    ///
    /// # Panics
    ///
    /// Panics when `n_points` is invalid. Use [`Self::try_new`] when the value
    /// comes from a user-controlled boundary.
    #[must_use]
    pub fn new(n_points: usize) -> Self {
        assert!(
            n_points >= Self::MIN_POINTS,
            "ScenarioGrid requires at least {} points for central-difference delta extraction, got {n_points}",
            Self::MIN_POINTS,
        );
        assert!(
            !n_points.is_multiple_of(2),
            "ScenarioGrid requires an odd number of points for a symmetric grid, got {n_points}"
        );
        let half = (n_points / 2) as f64;
        let shifts = (0..n_points).map(|idx| idx as f64 - half).collect();
        Self { shifts }
    }

    /// Try to create a symmetric grid centered on zero.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `n_points < 3` or when `n_points` is
    /// even, because an even number of points cannot include a center point and
    /// symmetric `-1` / `+1` shocks.
    pub fn try_new(n_points: usize) -> Result<Self> {
        if n_points < Self::MIN_POINTS {
            return Err(Error::Validation(format!(
                "ScenarioGrid requires at least {} points for central-difference delta extraction, got {n_points}",
                Self::MIN_POINTS,
            )));
        }
        if n_points.is_multiple_of(2) {
            return Err(Error::Validation(format!(
                "ScenarioGrid requires an odd number of points for a symmetric grid, got {n_points}"
            )));
        }
        let half = (n_points / 2) as f64;
        let shifts = (0..n_points).map(|idx| idx as f64 - half).collect();
        Ok(Self { shifts })
    }

    /// Return the ordered shift coordinates.
    #[must_use]
    pub fn shifts(&self) -> &[f64] {
        &self.shifts
    }
}

/// Scenario-grid sensitivity engine that reprices across multiple factor shocks.
#[derive(Debug, Clone)]
pub struct FullRepricingEngine {
    bump_config: BumpSizeConfig,
    scenario_grid: ScenarioGrid,
}

impl FullRepricingEngine {
    /// Create a repricing engine using `n_scenario_points` around the base market.
    #[must_use]
    pub fn new(bump_config: BumpSizeConfig, n_scenario_points: usize) -> Self {
        Self {
            bump_config,
            scenario_grid: ScenarioGrid::new(n_scenario_points),
        }
    }

    /// Try to create a repricing engine using `n_scenario_points` around the base market.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the scenario grid cannot support
    /// central-difference delta extraction.
    pub fn try_new(bump_config: BumpSizeConfig, n_scenario_points: usize) -> Result<Self> {
        Ok(Self {
            bump_config,
            scenario_grid: ScenarioGrid::try_new(n_scenario_points)?,
        })
    }

    fn validate_bump_size(factor: &FactorDefinition, bump_size: f64) -> Result<()> {
        if bump_size.is_finite() && bump_size.abs() >= f64::EPSILON {
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "FullRepricingEngine: factor {:?} has invalid bump size {bump_size}; bump size must be finite and non-zero",
                factor.id.as_str()
            )))
        }
    }

    /// Compute the full scenario P&L profile for each factor.
    pub fn compute_pnl_profiles(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<FactorPnlProfile>> {
        super::traits::validate_single_currency(positions, market, as_of)?;
        let base_pvs: Vec<f64> = positions
            .iter()
            .map(|(position_id, instrument, _)| {
                let pv = instrument.value_raw(market, as_of)?;
                if !pv.is_finite() {
                    return Err(Error::Validation(format!(
                        "minor 15: non-finite base PV for position '{position_id}' ({pv})"
                    )));
                }
                Ok(pv)
            })
            .collect::<Result<_>>()?;

        let mut profiles = Vec::with_capacity(factors.len());
        for factor in factors {
            let (bump_size, bump_unit) = self
                .bump_config
                .bump_size_with_unit_for_factor(&factor.id, &factor.factor_type);
            Self::validate_bump_size(factor, bump_size)?;
            let mut position_pnls = Vec::with_capacity(self.scenario_grid.shifts().len());

            for &shift in self.scenario_grid.shifts() {
                let bumped_market = market.bump(mapping_to_market_bumps(
                    &factor.market_mapping,
                    bump_size * shift,
                    bump_unit,
                    as_of,
                )?)?;

                let pnl_row: Vec<f64> = positions
                    .iter()
                    .enumerate()
                    .map(|(position_idx, (_, instrument, weight))| {
                        let pv = instrument.value_raw(&bumped_market, as_of)?;
                        if !pv.is_finite() {
                            let position_id = &positions[position_idx].0;
                            return Err(Error::Validation(format!(
                                "minor 15: non-finite bumped PV for position '{position_id}' on factor '{}' at shift {shift} ({pv})",
                                factor.id.as_str()
                            )));
                        }
                        Ok((pv - base_pvs[position_idx]) * *weight)
                    })
                    .collect::<Result<_>>()?;
                position_pnls.push(pnl_row);
            }

            profiles.push(FactorPnlProfile {
                factor_id: factor.id.clone(),
                shifts: self.scenario_grid.shifts().to_vec(),
                position_pnls,
            });
        }

        Ok(profiles)
    }
}

impl FactorSensitivityEngine for FullRepricingEngine {
    fn compute_sensitivities(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<SensitivityMatrix> {
        let profiles = self.compute_pnl_profiles(positions, factors, market, as_of)?;
        let position_ids = positions.iter().map(|(id, _, _)| id.clone()).collect();
        let factor_ids = factors.iter().map(|factor| factor.id.clone()).collect();
        let mut matrix = SensitivityMatrix::zeros(position_ids, factor_ids);

        for (factor_idx, (profile, factor)) in profiles.iter().zip(factors).enumerate() {
            let down_idx = profile
                .shifts
                .iter()
                .position(|shift| (*shift - (-1.0)).abs() < 1e-12);
            let up_idx = profile
                .shifts
                .iter()
                .position(|shift| (*shift - 1.0).abs() < 1e-12);

            if let (Some(down_idx), Some(up_idx)) = (down_idx, up_idx) {
                let bump_size = self
                    .bump_config
                    .bump_size_for_factor(&factor.id, &factor.factor_type);
                for position_idx in 0..positions.len() {
                    let delta = (profile.position_pnls[up_idx][position_idx]
                        - profile.position_pnls[down_idx][position_idx])
                        / (2.0 * bump_size);
                    matrix.set_delta(position_idx, factor_idx, delta);
                }
            }
        }

        Ok(matrix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use finstack_factor_model::{FactorType, MarketMapping};
    use finstack_valuations::instruments::Attributes;
    use finstack_valuations::pricer::InstrumentType;
    use std::any::Any;
    use time::macros::date;

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        curve_id: CurveId,
        tenor_years: f64,
        scale: f64,
        base_value_override: Option<f64>,
    }

    finstack_valuations::impl_empty_cashflow_provider!(
        MockInstrument,
        finstack_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl MockInstrument {
        fn new(id: &str, curve_id: &str, tenor_years: f64, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                curve_id: CurveId::new(curve_id),
                tenor_years,
                scale,
                base_value_override: None,
            }
        }

        fn non_finite_raw(id: &str, curve_id: &str) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                curve_id: CurveId::new(curve_id),
                tenor_years: 5.0,
                scale: f64::NAN,
                base_value_override: Some(1.0),
            }
        }

        fn raw_value(&self, market: &MarketContext) -> Result<f64> {
            Ok(market
                .get_discount(self.curve_id.as_str())?
                .zero(self.tenor_years)
                * self.scale)
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
            let amount = if let Some(amount) = self.base_value_override {
                amount
            } else {
                self.raw_value(market)?
            };
            Ok(Money::new(amount, Currency::USD))
        }

        fn value_raw(&self, market: &MarketContext, _as_of: Date) -> Result<f64> {
            self.raw_value(market)
        }
    }

    fn test_market(as_of: Date) -> Result<MarketContext> {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .interp(InterpStyle::MonotoneConvex)
            .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.80), (10.0, 0.60)])
            .build()?;
        Ok(MarketContext::new().insert(curve))
    }

    #[test]
    fn test_scenario_grid_construction() {
        let grid = ScenarioGrid::new(5);
        assert_eq!(grid.shifts().len(), 5);
        assert!((grid.shifts()[2]).abs() < 1e-12);
    }

    #[test]
    fn test_scenario_grid_minimum_points() {
        let grid = ScenarioGrid::new(3);
        assert_eq!(grid.shifts(), &[-1.0, 0.0, 1.0]);
    }

    #[test]
    #[should_panic(expected = "ScenarioGrid requires at least 3 points")]
    fn test_scenario_grid_rejects_too_few_points() {
        let _ = ScenarioGrid::new(2);
    }

    #[test]
    fn scenario_grid_try_new_rejects_even_point_counts() {
        let result = ScenarioGrid::try_new(4);
        assert!(
            result.is_err(),
            "scenario grid must reject even point counts"
        );
    }

    #[test]
    fn full_repricing_try_new_rejects_too_few_points() {
        let result = FullRepricingEngine::try_new(BumpSizeConfig::default(), 2);
        assert!(
            result.is_err(),
            "full repricing engine must return a validation error instead of panicking"
        );
    }

    #[test]
    fn test_full_repricing_engine_extracts_delta_from_profile() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let engine = FullRepricingEngine::new(BumpSizeConfig::default(), 5);
        let matrix = engine.compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert!((matrix.delta(0, 0) - 1.0).abs() < 1e-3);
        Ok(())
    }

    #[test]
    fn test_full_repricing_delta_normalizes_bump_size_override() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factor_id = FactorId::new("rates");
        let factors = vec![FactorDefinition {
            id: factor_id.clone(),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let mut bump_config = BumpSizeConfig::default();
        bump_config.overrides.insert(factor_id, 5.0);
        let matrix = FullRepricingEngine::new(bump_config, 5)
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert!(
            (matrix.delta(0, 0) - 1.0).abs() < 1e-3,
            "linear delta should be per bp, not scaled by the 5 bp override"
        );
        Ok(())
    }

    #[test]
    fn test_full_repricing_rejects_zero_bump_size() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factor_id = FactorId::new("rates");
        let factors = vec![FactorDefinition {
            id: factor_id.clone(),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let mut bump_config = BumpSizeConfig::default();
        bump_config.overrides.insert(factor_id, 0.0);
        let result = FullRepricingEngine::new(bump_config, 5)
            .compute_sensitivities(&positions, &factors, &market, as_of);

        assert!(
            result.is_err(),
            "zero bump size must return an error instead of a NaN delta"
        );
        Ok(())
    }

    #[test]
    fn minor15_full_repricing_rejects_non_finite_base_pv() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::non_finite_raw("bad-inst", "USD-OIS");
        let positions = vec![("bad-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let err = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_pnl_profiles(&positions, &factors, &market, as_of)
            .expect_err("minor 15: non-finite base PV must fail fast");
        assert!(
            err.to_string().contains("minor 15"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn test_full_repricing_engine_pnl_profiles_include_center_scenario() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let profiles = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_pnl_profiles(&positions, &factors, &market, as_of)?;

        assert_eq!(profiles.len(), 1);
        let profile = &profiles[0];
        assert_eq!(profile.shifts, vec![-2.0, -1.0, 0.0, 1.0, 2.0]);
        assert!((profile.position_pnls[2][0]).abs() < 1e-12);
        Ok(())
    }
}
