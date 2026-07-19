//! Finite-difference and repricing utilities for portfolio sensitivities.
//!
use super::delta_engine::mapping_to_market_bumps;
use super::traits::{FactorRepricingPlan, FactorSensitivityEngine};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::{Error, Result};
use finstack_quant_factor_model::sensitivity_matrix::SensitivityMatrix;
use finstack_quant_factor_model::{BumpSizeConfig, FactorDefinition, FactorId};
use finstack_quant_valuations::instruments::Instrument;

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

    /// Collect base PVs while validating a common pricing currency.
    ///
    /// The combined raw-value API retains the high-precision finite-difference
    /// convention and returns the reporting currency from the same pricing
    /// call. The ordered traversal preserves first-error semantics.
    fn collect_validated_base_pvs(
        positions: &[(String, &dyn Instrument, f64)],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<f64>> {
        let mut first_currency = None;
        positions
            .iter()
            .map(|(position_id, instrument, _)| {
                let (pv, currency) = instrument.value_raw_with_currency(market, as_of)?;
                if let Some((first_id, first_currency)) = first_currency {
                    if first_currency != currency {
                        return Err(Error::Validation(format!(
                            "Factor sensitivity engine requires a single pricing currency: \
                             position '{first_id}' prices in {first_currency} but position \
                             '{position_id}' prices in {currency}; convert positions to a \
                             common base currency before computing factor sensitivities"
                        )));
                    }
                } else {
                    first_currency = Some((position_id.as_str(), currency));
                }

                if !pv.is_finite() {
                    return Err(Error::Validation(format!(
                        "minor 15: non-finite base PV for position '{position_id}' ({pv})"
                    )));
                }
                Ok(pv)
            })
            .collect()
    }

    /// Compute full-repricing scenario P&L profiles for every factor.
    ///
    /// Each factor is evaluated over this engine's ordered scenario grid. A
    /// profile row holds `weight * (PV_bumped - PV_base)` for each input
    /// position, so rows retain position order and values use the positions'
    /// common pricing currency.
    ///
    /// # Errors
    ///
    /// Returns validation errors for mixed pricing currencies, non-finite base
    /// or bumped PVs, or a non-finite/zero bump. Propagates market-bump and
    /// instrument-pricing failures.
    pub fn compute_pnl_profiles(
        &self,
        positions: &[(String, &dyn Instrument, f64)],
        factors: &[FactorDefinition],
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Vec<FactorPnlProfile>> {
        let base_pvs = Self::collect_validated_base_pvs(positions, market, as_of)?;
        let repricing_plan = FactorRepricingPlan::build(positions, factors, market);

        // Each factor's profile is an independent, side-effect-free function of
        // the read-only base market, the base PVs, and the factor definition, so
        // fanning out across factors is deterministic: `par_iter().collect()`
        // preserves factor order and produces identical results regardless of
        // scheduling. This mirrors `DeltaBasedEngine`, which already
        // parallelizes the equivalent (and cheaper) work over factors.
        use rayon::prelude::*;
        let profile_results: Vec<Result<FactorPnlProfile>> = factors
            .par_iter()
            .enumerate()
            .map(|(factor_index, factor)| {
                let (bump_size, bump_unit) = self
                    .bump_config
                    .bump_size_with_unit_for_factor(&factor.id, &factor.factor_type);
                Self::validate_bump_size(factor, bump_size)?;
                let affected_positions = repricing_plan.affected(factor_index);
                let mut position_pnls = Vec::with_capacity(self.scenario_grid.shifts().len());

                for &shift in self.scenario_grid.shifts() {
                    if shift == 0.0 {
                        position_pnls.push(vec![0.0; positions.len()]);
                        continue;
                    }
                    let bumped_market = market.bump(mapping_to_market_bumps(
                        &factor.market_mapping,
                        bump_size * shift,
                        bump_unit,
                        as_of,
                    )?)?;

                    let pnl_row: Vec<f64> = positions
                        .iter()
                        .zip(affected_positions)
                        .enumerate()
                        .map(
                            |(position_idx, ((_, instrument, weight), affected))| {
                                if !affected {
                                    return Ok(0.0);
                                }
                            let pv = instrument.value_raw(&bumped_market, as_of)?;
                            if !pv.is_finite() {
                                let position_id = &positions[position_idx].0;
                                return Err(Error::Validation(format!(
                                    "minor 15: non-finite bumped PV for position '{position_id}' on factor '{}' at shift {shift} ({pv})",
                                    factor.id.as_str()
                                )));
                            }
                            Ok((pv - base_pvs[position_idx]) * *weight)
                            },
                        )
                        .collect::<Result<_>>()?;
                    position_pnls.push(pnl_row);
                }

                Ok(FactorPnlProfile {
                    factor_id: factor.id.clone(),
                    shifts: self.scenario_grid.shifts().to_vec(),
                    position_pnls,
                })
            })
            .collect();
        profile_results.into_iter().collect()
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
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::fx::{FxMatrix, FxQuery, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_factor_model::{FactorType, MarketMapping};
    use finstack_quant_valuations::instruments::{Attributes, MarketDependencies};
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::macros::date;

    #[derive(Clone)]
    struct MockInstrument {
        id: String,
        attributes: Attributes,
        curve_id: CurveId,
        tenor_years: f64,
        scale: f64,
        currency: Currency,
        base_value_override: Option<f64>,
        money_value_calls: Option<Arc<AtomicUsize>>,
        raw_value_calls: Option<Arc<AtomicUsize>>,
        dependencies_fail: bool,
        fx_pair: Option<(Currency, Currency)>,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        MockInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl MockInstrument {
        fn new(id: &str, curve_id: &str, tenor_years: f64, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                curve_id: CurveId::new(curve_id),
                tenor_years,
                scale,
                currency: Currency::USD,
                base_value_override: None,
                money_value_calls: None,
                raw_value_calls: None,
                dependencies_fail: false,
                fx_pair: None,
            }
        }

        fn non_finite_raw(id: &str, curve_id: &str) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                curve_id: CurveId::new(curve_id),
                tenor_years: 5.0,
                scale: f64::NAN,
                currency: Currency::USD,
                base_value_override: Some(1.0),
                money_value_calls: None,
                raw_value_calls: None,
                dependencies_fail: false,
                fx_pair: None,
            }
        }

        fn fx_cross(id: &str, base: Currency, quote: Currency, scale: f64) -> Self {
            Self {
                id: id.to_string(),
                attributes: Attributes::new(),
                curve_id: CurveId::new("UNUSED"),
                tenor_years: 0.0,
                scale,
                currency: Currency::USD,
                base_value_override: None,
                money_value_calls: None,
                raw_value_calls: None,
                dependencies_fail: false,
                fx_pair: Some((base, quote)),
            }
        }

        fn with_dependency_failure(mut self) -> Self {
            self.dependencies_fail = true;
            self
        }

        fn raw_value(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
            if let Some((base, quote)) = self.fx_pair {
                return Ok(market
                    .fx_required()?
                    .rate(FxQuery::new(base, quote, as_of))?
                    .rate
                    * self.scale);
            }
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

        fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
            if let Some(calls) = &self.money_value_calls {
                calls.fetch_add(1, Ordering::Relaxed);
            }
            let amount = if let Some(amount) = self.base_value_override {
                amount
            } else {
                self.raw_value(market, as_of)?
            };
            Ok(Money::new(amount, self.currency))
        }

        fn base_value_raw(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
            if let Some(calls) = &self.raw_value_calls {
                calls.fetch_add(1, Ordering::Relaxed);
            }
            self.raw_value(market, as_of)
        }

        fn base_value_raw_with_currency(
            &self,
            market: &MarketContext,
            as_of: Date,
        ) -> Result<(f64, Currency)> {
            if let Some(calls) = &self.raw_value_calls {
                calls.fetch_add(1, Ordering::Relaxed);
            }
            Ok((self.raw_value(market, as_of)?, self.currency))
        }

        fn market_dependencies(&self) -> Result<MarketDependencies> {
            if self.dependencies_fail {
                return Err(Error::Validation(
                    "mock dependency introspection failed".to_string(),
                ));
            }
            let mut dependencies = MarketDependencies::new();
            if let Some((base, quote)) = self.fx_pair {
                dependencies.add_fx_pair(base, quote);
            } else {
                dependencies
                    .curves
                    .discount_curves
                    .push(self.curve_id.clone());
            }
            Ok(dependencies)
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

    fn test_market_with_other_curve(as_of: Date) -> Result<MarketContext> {
        let other_curve = DiscountCurve::builder("USD-OTHER")
            .base_date(as_of)
            .interp(InterpStyle::MonotoneConvex)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.75), (10.0, 0.55)])
            .build()?;
        Ok(test_market(as_of)?.insert(other_curve))
    }

    fn test_fx_market() -> Result<MarketContext> {
        let provider = Arc::new(SimpleFxProvider::new());
        provider.set_quotes(&[
            (Currency::EUR, Currency::USD, 1.10),
            (Currency::USD, Currency::JPY, 150.0),
        ])?;
        Ok(MarketContext::new().insert_fx(FxMatrix::new(provider)))
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
    fn full_repricing_reprices_triangulated_fx_cross_for_any_leg_change() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_fx_market()?;
        let instrument = MockInstrument::fx_cross("eur-jpy", Currency::EUR, Currency::JPY, 1.0);
        let positions = vec![("eur-jpy".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: FactorId::new("usd-eur"),
            factor_type: FactorType::FX,
            market_mapping: finstack_quant_factor_model::MarketMapping::FxRate {
                pair: (Currency::USD, Currency::EUR),
            },
            description: None,
        }];

        let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert!(
            matrix.delta(0, 0).abs() > 1e-12,
            "a USD/EUR leg bump must move the triangulated EUR/JPY cross"
        );
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
    fn full_repricing_reports_factor_errors_in_input_order() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        let positions = vec![("curve-pos".to_string(), &instrument as &dyn Instrument, 1.0)];
        let first_id = FactorId::new("first-invalid");
        let second_id = FactorId::new("second-invalid");
        let factors = vec![
            FactorDefinition {
                id: first_id.clone(),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
            FactorDefinition {
                id: second_id.clone(),
                factor_type: FactorType::Rates,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![CurveId::new("USD-OIS")],
                    units: BumpUnits::RateBp,
                },
                description: None,
            },
        ];
        let mut bump_config = BumpSizeConfig::default();
        bump_config.overrides.insert(first_id, 0.0);
        bump_config.overrides.insert(second_id, 0.0);

        let error = FullRepricingEngine::new(bump_config, 5)
            .compute_pnl_profiles(&positions, &factors, &market, as_of)
            .expect_err("both factors are invalid");
        let message = error.to_string();
        assert!(
            message.contains("first-invalid"),
            "logical first factor must win: {message}"
        );
        assert!(
            !message.contains("second-invalid"),
            "later factor error must not win: {message}"
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

    #[test]
    fn full_repricing_reuses_base_pv_for_zero_shift() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let raw_calls = Arc::new(AtomicUsize::new(0));
        let money_calls = Arc::new(AtomicUsize::new(0));
        let mut instrument = MockInstrument::new("curve-inst", "USD-OIS", 5.0, 10_000.0);
        instrument.raw_value_calls = Some(Arc::clone(&raw_calls));
        instrument.money_value_calls = Some(Arc::clone(&money_calls));
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

        assert_eq!(profiles[0].position_pnls[2], vec![0.0]);
        assert_eq!(
            raw_calls.load(Ordering::Relaxed),
            5,
            "one combined base raw PV plus four non-zero grid shocks should be priced"
        );
        assert_eq!(
            money_calls.load(Ordering::Relaxed),
            0,
            "currency validation must not trigger a second Money pricing call"
        );
        Ok(())
    }

    #[test]
    fn full_repricing_combined_base_pricing_rejects_mixed_currencies() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market(as_of)?;
        let usd = MockInstrument::new("usd-inst", "USD-OIS", 5.0, 10_000.0);
        let mut eur = MockInstrument::new("eur-inst", "USD-OIS", 5.0, 10_000.0);
        eur.currency = Currency::EUR;
        let positions = vec![
            ("usd-pos".to_string(), &usd as &dyn Instrument, 1.0),
            ("eur-pos".to_string(), &eur as &dyn Instrument, 1.0),
        ];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let error = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_pnl_profiles(&positions, &factors, &market, as_of)
            .expect_err("mixed raw-PV currencies must be rejected");
        let message = error.to_string();
        assert!(message.contains("usd-pos"), "unexpected error: {message}");
        assert!(message.contains("eur-pos"), "unexpected error: {message}");
        assert!(message.contains("USD"), "unexpected error: {message}");
        assert!(message.contains("EUR"), "unexpected error: {message}");
        Ok(())
    }

    #[test]
    fn full_repricing_skips_proven_unaffected_positions_with_parity() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market_with_other_curve(as_of)?;
        let affected_calls = Arc::new(AtomicUsize::new(0));
        let unaffected_calls = Arc::new(AtomicUsize::new(0));
        let mut affected = MockInstrument::new("affected", "USD-OIS", 5.0, 10_000.0);
        affected.raw_value_calls = Some(Arc::clone(&affected_calls));
        let mut unaffected = MockInstrument::new("unaffected", "USD-OTHER", 5.0, 10_000.0);
        unaffected.raw_value_calls = Some(Arc::clone(&unaffected_calls));
        let positions = vec![
            ("affected".to_string(), &affected as &dyn Instrument, 1.0),
            (
                "unaffected".to_string(),
                &unaffected as &dyn Instrument,
                1.0,
            ),
        ];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_sensitivities(&positions, &factors, &market, as_of)?;
        let reference = MockInstrument::new("reference", "USD-OIS", 5.0, 10_000.0);
        let reference_positions =
            vec![("reference".to_string(), &reference as &dyn Instrument, 1.0)];
        let reference_matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_sensitivities(&reference_positions, &factors, &market, as_of)?;

        assert!((matrix.delta(0, 0) - reference_matrix.delta(0, 0)).abs() < 1e-12);
        assert_eq!(matrix.delta(1, 0), 0.0);
        assert_eq!(
            affected_calls.load(Ordering::Relaxed),
            5,
            "one base endpoint plus four non-zero grid shifts must be priced"
        );
        assert_eq!(
            unaffected_calls.load(Ordering::Relaxed),
            1,
            "a resolved position on another curve needs only base currency validation"
        );
        Ok(())
    }

    #[test]
    fn full_repricing_reprices_dependency_failures_conservatively() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market_with_other_curve(as_of)?;
        let calls = Arc::new(AtomicUsize::new(0));
        let mut unresolved =
            MockInstrument::new("unresolved", "USD-OTHER", 5.0, 10_000.0).with_dependency_failure();
        unresolved.raw_value_calls = Some(Arc::clone(&calls));
        let positions = vec![(
            "unresolved".to_string(),
            &unresolved as &dyn Instrument,
            1.0,
        )];
        let factors = vec![FactorDefinition {
            id: FactorId::new("rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert_eq!(matrix.delta(0, 0), 0.0);
        assert_eq!(
            calls.load(Ordering::Relaxed),
            5,
            "failed dependency introspection must fall back to every grid endpoint"
        );
        Ok(())
    }

    #[test]
    fn full_repricing_routes_curve_mapping_by_actual_market_storage() -> Result<()> {
        let as_of = date!(2025 - 01 - 01);
        let market = test_market_with_other_curve(as_of)?;
        let calls = Arc::new(AtomicUsize::new(0));
        let mut instrument = MockInstrument::new("custom", "USD-OTHER", 5.0, 10_000.0);
        instrument.raw_value_calls = Some(Arc::clone(&calls));
        let positions = vec![("custom".to_string(), &instrument as &dyn Instrument, 1.0)];
        let factors = vec![FactorDefinition {
            id: FactorId::new("custom"),
            factor_type: FactorType::Custom("curve".to_string()),
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: None,
        }];

        FullRepricingEngine::new(BumpSizeConfig::default(), 5)
            .compute_sensitivities(&positions, &factors, &market, as_of)?;

        assert_eq!(
            calls.load(Ordering::Relaxed),
            1,
            "declared factor type must not override the exact stored curve role"
        );
        Ok(())
    }
}
