//! Cross-factor gamma calculators.
//!
//! Provides reusable `MetricCalculator` implementations for mixed second
//! derivatives between pairs of risk factors. The calculator resolves the
//! concrete market dependencies at runtime from `MetricContext`, then applies
//! a four-corner central mixed difference.

use crate::instruments::FxPair;
use crate::metrics::finite_difference::{
    active_minimum_vol, bump_scalar_price, bumped_volatility_override_dyn, central_mixed,
    volatility_override,
};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxQuery};
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

const NO_DEPS: &[MetricId] = &[];

/// Concrete risk-factor perturbation used by [`CrossFactorCalculator`].
#[derive(Clone)]
pub enum FactorBumper {
    /// Parallel basis-point shift across one or more yield curves.
    ParallelCurve {
        /// Discount and forward curve identifiers bumped in parallel.
        curve_ids: Vec<CurveId>,
        /// Parallel shift magnitude in basis points.
        bump_bp: f64,
    },
    /// Quote-space credit spread shift with hazard curve recalibration.
    CreditQuote {
        /// Hazard curve identifiers rebuilt under the par-spread bump.
        curve_ids: Vec<CurveId>,
        /// Parallel par-spread shock magnitude in basis points.
        bump_bp: f64,
        /// Unbumped base market context supplying original quotes.
        source_market: Arc<MarketContext>,
        /// Calibration service used to rebuild each hazard curve.
        provider: Arc<dyn crate::recalibration::RecalibrationProvider>,
    },
    /// Parallel additive volatility shift across surfaces, scalar vols, or quote overrides.
    VolParallel {
        /// Volatility surface identifiers shifted in parallel.
        vol_surface_ids: Vec<CurveId>,
        /// Unitless volatility scalar identifiers shifted additively.
        vol_scalar_ids: Vec<String>,
        /// Active instrument-level volatility override, if set.
        override_volatility: Option<f64>,
        /// Absolute annualized volatility shift magnitude.
        bump_abs: f64,
        /// True at zero volatility where a second-order forward stencil is used.
        one_sided: bool,
    },
    /// Relative bump to an equity or underlying scalar spot quote.
    Spot {
        /// Scalar price identifier shifted proportionally.
        price_id: String,
        /// Relative shift as a decimal fraction (`0.01` = 1%).
        bump_pct: f64,
    },
    /// Percentage bump to one or more FX pairs.
    Fx {
        /// Currency pairs bumped together.
        pairs: Vec<FxPair>,
        /// Relative shift as a decimal fraction (`0.01` = 1%).
        bump_pct: f64,
    },
}

type BumperFactoryFn = fn(&MetricContext) -> Result<Option<FactorBumper>>;

impl FactorBumper {
    /// Produce a market context bumped along `direction` for this risk factor.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context to copy and perturb.
    /// * `as_of` - Valuation date used for FX percentage shifts.
    /// * `direction` - Signed stencil coordinate multiplier (`1.0`, `-1.0`, `2.0`, etc.).
    pub fn bump_market(
        &self,
        market: &MarketContext,
        as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        match self {
            Self::ParallelCurve { curve_ids, bump_bp } => {
                let bumps = curve_ids
                    .iter()
                    .map(|id| {
                        market.curve_target(id).map(|target| {
                            target.into_bump(BumpSpec::parallel_bp(bump_bp * direction))
                        })
                    })
                    .collect::<finstack_quant_core::Result<Vec<_>>>()?;
                market.bump(bumps)
            }
            Self::CreditQuote {
                curve_ids,
                bump_bp,
                source_market,
                provider,
            } => {
                use crate::recalibration::{
                    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump,
                };
                let mut target = market.clone();
                for id in curve_ids {
                    let hazard = source_market.get_hazard(id.as_str())?;
                    let discount_curve_id = provider.get_hazard_discount_curve_id(&hazard)?;
                    let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                        hazard,
                        source_market: Arc::clone(source_market),
                        target_market: Arc::new(target.clone()),
                        discount_curve_id,
                        doc_clause: None,
                        cds_valuation_convention: None,
                        deal_quote_override: None,
                        action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(
                            bump_bp * direction,
                        )),
                    })?;
                    target.try_insert_mut(rebuilt.as_ref().clone())?;
                }
                Ok(target)
            }
            Self::VolParallel {
                vol_surface_ids,
                vol_scalar_ids,
                override_volatility,
                bump_abs,
                ..
            } => {
                if override_volatility.is_some() {
                    return Ok(market.clone());
                }
                let mut bumped = market.bump(vol_surface_ids.iter().cloned().map(|id| {
                    MarketBump::VolSurface {
                        id,
                        spec: BumpSpec {
                            mode: BumpMode::Additive,
                            units: BumpUnits::Fraction,
                            value: bump_abs * direction,
                            bump_type: BumpType::Parallel,
                        },
                    }
                }))?;
                for id in vol_scalar_ids {
                    let Some(volatility) = market.get_price(id)?.get_unitless() else {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "volatility quote '{id}' must be a unitless scalar"
                        )));
                    };
                    let value = volatility + bump_abs * direction;
                    if !value.is_finite() || value < 0.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "bumped volatility quote '{id}' must be finite and nonnegative"
                        )));
                    }
                    bumped.insert_price_mut(id, MarketScalar::unitless(value)?);
                }
                Ok(bumped)
            }
            Self::Spot { price_id, bump_pct } => {
                bump_scalar_price(market, price_id, bump_pct * direction)
            }
            Self::Fx { pairs, bump_pct } => {
                let bumps: Vec<MarketBump> = pairs
                    .iter()
                    .map(|pair| MarketBump::FxPct {
                        base: pair.base,
                        quote: pair.quote,
                        pct: bump_pct * direction * 100.0,
                        as_of,
                        policy: FxConversionPolicy::CashflowDate,
                    })
                    .collect();
                market.bump(bumps)
            }
        }
    }

    /// Reporting-unit step size used to normalize the mixed finite difference.
    pub fn bump_size(&self) -> f64 {
        match self {
            Self::ParallelCurve { bump_bp, .. } | Self::CreditQuote { bump_bp, .. } => *bump_bp,
            Self::VolParallel { bump_abs, .. } => *bump_abs * 100.0,
            Self::Spot { bump_pct, .. } | Self::Fx { bump_pct, .. } => *bump_pct * 100.0,
        }
    }

    /// Normalized first-derivative stencil weights.
    ///
    /// Volatility at zero uses a second-order forward stencil; all interior factors are central.
    pub fn stencil(&self) -> &'static [(f64, f64)] {
        match self {
            Self::VolParallel {
                one_sided: true, ..
            } => &[(0.0, -1.5), (1.0, 2.0), (2.0, -0.5)],
            _ => &[(-1.0, -0.5), (1.0, 0.5)],
        }
    }

    fn recalibrates_credit(&self) -> bool {
        matches!(self, Self::CreditQuote { .. })
    }

    /// Return whether `market` supplies every dependency required by this factor.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context to check for the bumped curves, surfaces, spots, or FX pairs.
    /// * `as_of` - Valuation date used to probe FX pair availability.
    pub fn is_applicable(&self, market: &MarketContext, as_of: Date) -> bool {
        match self {
            Self::ParallelCurve { curve_ids, .. } => curve_ids.iter().any(|curve_id| {
                market.get_discount(curve_id.as_str()).is_ok()
                    || market.get_forward(curve_id.as_str()).is_ok()
            }),
            Self::CreditQuote { curve_ids, .. } => curve_ids
                .iter()
                .all(|id| market.get_hazard(id.as_str()).is_ok()),
            Self::VolParallel {
                vol_surface_ids,
                vol_scalar_ids,
                override_volatility,
                ..
            } => {
                override_volatility.is_some()
                    || (vol_surface_ids
                        .iter()
                        .all(|vol_surface_id| market.get_surface(vol_surface_id.as_str()).is_ok())
                        && vol_scalar_ids.iter().all(|id| {
                            market
                                .get_price(id)
                                .is_ok_and(|scalar| scalar.is_unitless())
                        }))
            }
            Self::Spot { price_id, .. } => market.get_price(price_id).is_ok(),
            Self::Fx { pairs, .. } => pairs.iter().any(|pair| {
                market
                    .fx()
                    .and_then(|fx| {
                        fx.rate(FxQuery::new(
                            pair.base,
                            pair.quote,
                            as_of,
                            FxConversionPolicy::CashflowDate,
                        ))
                        .ok()
                    })
                    .is_some()
            }),
        }
    }

    fn bump_instrument(
        &self,
        instrument: &dyn crate::instruments::Instrument,
        direction: f64,
    ) -> Result<Option<Box<dyn crate::instruments::Instrument>>> {
        match self {
            Self::VolParallel {
                override_volatility: Some(_),
                bump_abs,
                ..
            } => bumped_volatility_override_dyn(instrument, bump_abs * direction, true),
            _ => Ok(None),
        }
    }
}

/// Create a rate bumper from runtime instrument dependencies.
///
/// # Arguments
///
/// * `context` - Metric context whose instrument dependencies and bump config are queried.
pub fn make_rates_bumper(context: &MetricContext) -> Result<Option<FactorBumper>> {
    let deps = context.instrument.market_dependencies()?;
    let mut curve_ids = Vec::new();
    for curve_id in deps
        .curves
        .discount_curves
        .iter()
        .chain(&deps.curves.forward_curves)
    {
        if !curve_ids.contains(curve_id) {
            curve_ids.push(curve_id.clone());
        }
    }
    if curve_ids.is_empty() {
        return Ok(None);
    }

    let defaults = sens_config::resolve(context)?;
    Ok(Some(FactorBumper::ParallelCurve {
        curve_ids,
        bump_bp: defaults.rate_bump_bp,
    }))
}

/// Create a credit bumper from runtime instrument dependencies.
///
/// # Arguments
///
/// * `context` - Metric context whose instrument dependencies, credit curves, and recalibration
///   provider are resolved.
pub fn make_credit_bumper(context: &MetricContext) -> Result<Option<FactorBumper>> {
    let deps = context.instrument.market_dependencies()?;
    if deps.curves.credit_curves.is_empty() {
        return Ok(None);
    }

    let defaults = sens_config::resolve(context)?;
    for id in &deps.curves.credit_curves {
        super::cs01::require_hazard_replay(
            context.curves.get_hazard(id.as_str())?.as_ref(),
            "credit cross-gamma",
        )?;
    }
    Ok(Some(FactorBumper::CreditQuote {
        curve_ids: deps.curves.credit_curves.to_vec(),
        bump_bp: defaults.credit_spread_bump_bp,
        source_market: Arc::clone(&context.curves),
        provider: context.recalibration_provider("credit_cross_gamma")?,
    }))
}

/// Create a volatility bumper from runtime instrument dependencies.
///
/// # Arguments
///
/// * `context` - Metric context whose instrument volatility dependencies, override, and market
///   surfaces/scalars are resolved.
pub fn make_vol_bumper(context: &MetricContext) -> Result<Option<FactorBumper>> {
    let deps = context.instrument.market_dependencies()?;
    let override_volatility = volatility_override(context.instrument.as_ref());
    let mut vol_surface_ids = Vec::new();
    let mut vol_scalar_ids = Vec::new();
    if override_volatility.is_none() {
        for id in deps.unique_vol_surface_ids() {
            let name = id.as_str();
            if context.curves.get_vol_cube(name).is_err()
                && context.curves.get_fx_delta_vol_surface(name).is_err()
                && context.curves.get_surface(name).is_err()
                && context
                    .curves
                    .get_price(name)
                    .is_ok_and(|scalar| scalar.is_unitless())
            {
                vol_scalar_ids.push(name.to_owned());
            } else {
                crate::instruments::dependencies::require_volatility_grid(&context.curves, &id)?;
                vol_surface_ids.push(id);
            }
        }
    }
    if vol_surface_ids.is_empty() && vol_scalar_ids.is_empty() && override_volatility.is_none() {
        return Ok(None);
    }

    let defaults = sens_config::resolve(context)?;
    let mut minimum = active_minimum_vol(
        context.instrument.as_ref(),
        &context.curves,
        &vol_surface_ids,
    )?
    .unwrap_or(f64::INFINITY);
    for id in &vol_scalar_ids {
        if let Some(vol) = context.curves.get_price(id)?.get_unitless() {
            minimum = minimum.min(vol);
        }
    }
    if !minimum.is_finite() || minimum < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "cross-gamma volatility must be finite and nonnegative".into(),
        ));
    }
    let one_sided = minimum == 0.0;
    let bump_abs = if one_sided {
        defaults.vol_bump_decimal
    } else {
        defaults.vol_bump_decimal.min(minimum)
    };
    Ok(Some(FactorBumper::VolParallel {
        vol_surface_ids,
        vol_scalar_ids,
        override_volatility,
        bump_abs,
        one_sided,
    }))
}

/// Create a spot bumper from runtime instrument dependencies.
///
/// # Arguments
///
/// * `context` - Metric context whose spot scalar dependency and bump config are resolved.
pub fn make_spot_bumper(context: &MetricContext) -> Result<Option<FactorBumper>> {
    let deps = context.instrument.market_dependencies()?;
    let Some(price_id) = deps.market_scalar_ids.first() else {
        return Ok(None);
    };

    let defaults = sens_config::resolve(context)?;
    Ok(Some(FactorBumper::Spot {
        price_id: price_id.clone(),
        bump_pct: defaults.spot_bump_decimal,
    }))
}

/// Create an FX bumper from runtime instrument dependencies.
///
/// # Arguments
///
/// * `context` - Metric context whose FX pair dependencies and bump config are resolved.
pub fn make_fx_bumper(context: &MetricContext) -> Result<Option<FactorBumper>> {
    let deps = context.instrument.market_dependencies()?;
    if deps.fx_pairs.is_empty() {
        return Ok(None);
    }

    let defaults = sens_config::resolve(context)?;
    Ok(Some(FactorBumper::Fx {
        pairs: deps.fx_pairs,
        bump_pct: defaults.spot_bump_decimal,
    }))
}

/// Generic cross-factor calculator using runtime bumper factories.
pub struct CrossFactorCalculator {
    factory_a: BumperFactoryFn,
    factory_b: BumperFactoryFn,
}

impl CrossFactorCalculator {
    /// Creates a reusable cross-factor calculator from two bumper factories.
    ///
    /// # Arguments
    ///
    /// * `factory_a` - Constructor resolving the first risk-factor perturbation.
    /// * `factory_b` - Constructor resolving the second risk-factor perturbation.
    pub fn new(factory_a: BumperFactoryFn, factory_b: BumperFactoryFn) -> Self {
        Self {
            factory_a,
            factory_b,
        }
    }
}

impl MetricCalculator for CrossFactorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let Some(bumper_a) = (self.factory_a)(context)? else {
            return Ok(0.0);
        };
        let Some(bumper_b) = (self.factory_b)(context)? else {
            return Ok(0.0);
        };

        let market = context.curves.as_ref();
        let as_of = context.as_of;
        if !bumper_a.is_applicable(market, as_of) || !bumper_b.is_applicable(market, as_of) {
            return Ok(0.0);
        }

        let h_abs = bumper_a.bump_size();
        let k_abs = bumper_b.bump_size();

        if bumper_a.stencil().len() == 2 && bumper_b.stencil().len() == 2 {
            return central_mixed(
                || reprice_corner(context, market, as_of, &bumper_a, &bumper_b, 1.0, 1.0),
                || reprice_corner(context, market, as_of, &bumper_a, &bumper_b, 1.0, -1.0),
                || reprice_corner(context, market, as_of, &bumper_a, &bumper_b, -1.0, 1.0),
                || reprice_corner(context, market, as_of, &bumper_a, &bumper_b, -1.0, -1.0),
                h_abs,
                k_abs,
            );
        }
        let mut value = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for &(direction_a, weight_a) in bumper_a.stencil() {
            for &(direction_b, weight_b) in bumper_b.stencil() {
                value.add(
                    weight_a
                        * weight_b
                        * reprice_corner(
                            context,
                            market,
                            as_of,
                            &bumper_a,
                            &bumper_b,
                            direction_a,
                            direction_b,
                        )?,
                )?;
            }
        }
        Ok(value.get_total()? / (h_abs * k_abs))
    }

    fn dependencies(&self) -> &[MetricId] {
        NO_DEPS
    }
}

fn reprice_corner(
    context: &MetricContext,
    market: &MarketContext,
    as_of: Date,
    bumper_a: &FactorBumper,
    bumper_b: &FactorBumper,
    direction_a: f64,
    direction_b: f64,
) -> Result<f64> {
    let bumped_ab = if bumper_a.recalibrates_credit() {
        let other = bumper_b.bump_market(market, as_of, direction_b)?;
        bumper_a.bump_market(&other, as_of, direction_a)?
    } else {
        let other = bumper_a.bump_market(market, as_of, direction_a)?;
        bumper_b.bump_market(&other, as_of, direction_b)?
    };
    let instrument_a = bumper_a.bump_instrument(context.instrument.as_ref(), direction_a)?;
    let instrument_a = instrument_a
        .as_deref()
        .unwrap_or(context.instrument.as_ref());
    let instrument_ab = bumper_b.bump_instrument(instrument_a, direction_b)?;
    context.reprice_instrument_raw(
        instrument_ab.as_deref().unwrap_or(instrument_a),
        &bumped_ab,
        as_of,
    )
}

#[cfg(test)]
mod tests {
    use super::FactorBumper;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::{
        VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::types::CurveId;

    fn flat_surface(id: &str, vol: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[1.0])
            .secondary_values(&[100.0])
            .secondary_axis(VolSurfaceAxis::Strike)
            .quote_type(VolQuoteType::BlackLognormal)
            .interpolation_mode(VolInterpolationMode::Vol)
            .row(&[vol])
            .build()
            .expect("surface")
    }

    #[test]
    fn parallel_vol_bumper_moves_every_resolved_surface() {
        let market = MarketContext::new()
            .insert_surface(flat_surface("VOL-A", 0.20))
            .insert_surface(flat_surface("VOL-B", 0.30));
        let bumper = FactorBumper::VolParallel {
            vol_surface_ids: vec![CurveId::new("VOL-A"), CurveId::new("VOL-B")],
            vol_scalar_ids: Vec::new(),
            override_volatility: None,
            bump_abs: 0.01,
            one_sided: false,
        };

        assert!(bumper.is_applicable(&market, Date::MIN));
        let bumped = bumper
            .bump_market(&market, Date::MIN, 1.0)
            .expect("parallel bump");
        let surface_a = bumped.get_surface("VOL-A").expect("VOL-A");
        let bumped_a = finstack_quant_models::volatility::get_surface_vol(&surface_a, 1.0, 100.0)
            .expect("VOL-A value");
        let surface_b = bumped.get_surface("VOL-B").expect("VOL-B");
        let bumped_b = finstack_quant_models::volatility::get_surface_vol(&surface_b, 1.0, 100.0)
            .expect("VOL-B value");
        assert!((bumped_a - 0.21).abs() < 1e-12);
        assert!((bumped_b - 0.31).abs() < 1e-12);
    }
}
