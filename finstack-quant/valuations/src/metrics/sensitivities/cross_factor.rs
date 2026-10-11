//! Cross-factor gamma calculators.
//!
//! Provides reusable `MetricCalculator` implementations for mixed second
//! derivatives between pairs of risk factors. The calculator resolves the
//! concrete market dependencies at runtime from `MetricContext`, then applies
//! a four-corner central mixed difference.

use crate::instruments::common_impl::dependencies::FxPair;
use crate::metrics::core::finite_difference::{
    bump_scalar_price, central_mixed, min_grid_vol, volatility_override,
};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::Arc;

const NO_DEPS: &[MetricId] = &[];

/// Identifies which two risk factors are crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrossFactorPair {
    /// Parallel rates against parallel credit spreads.
    RatesCredit,
    /// Parallel rates against parallel volatility shifts.
    RatesVol,
    /// Spot/underlying against parallel volatility shifts.
    SpotVol,
    /// Spot/underlying against parallel credit spreads.
    SpotCredit,
    /// FX spot against parallel volatility shifts.
    FxVol,
    /// FX spot against parallel interest-rate shifts.
    FxRates,
    /// Parallel credit spreads against parallel volatility shifts.
    CreditVol,
}

impl CrossFactorPair {
    /// All currently supported cross-factor pairs.
    pub const ALL: &'static [Self] = &[
        Self::RatesCredit,
        Self::RatesVol,
        Self::SpotVol,
        Self::SpotCredit,
        Self::FxVol,
        Self::FxRates,
        Self::CreditVol,
    ];

    /// Returns the standard `MetricId` associated with this pair.
    pub fn metric_id(self) -> MetricId {
        match self {
            Self::RatesCredit => MetricId::CrossGammaRatesCredit,
            Self::RatesVol => MetricId::CrossGammaRatesVol,
            Self::SpotVol => MetricId::CrossGammaSpotVol,
            Self::SpotCredit => MetricId::CrossGammaSpotCredit,
            Self::FxVol => MetricId::CrossGammaFxVol,
            Self::FxRates => MetricId::CrossGammaFxRates,
            Self::CreditVol => MetricId::CrossGammaCreditVol,
        }
    }
}

pub(crate) trait FactorBumper: Send + Sync {
    fn bump_market(
        &self,
        market: &MarketContext,
        as_of: Date,
        direction: f64,
    ) -> Result<MarketContext>;

    fn bump_size(&self) -> f64;

    /// Normalized first-derivative weights. Volatility at zero uses a
    /// second-order forward stencil; all interior factors are central.
    fn stencil(&self) -> &'static [(f64, f64)] {
        &[(-1.0, -0.5), (1.0, 0.5)]
    }

    /// Credit quotes must rebuild after every other market shock.
    fn recalibrates_credit(&self) -> bool {
        false
    }

    fn is_applicable(&self, market: &MarketContext, as_of: Date) -> bool;

    /// Clone only when a factor is supplied by the instrument's active quote.
    fn bump_instrument(
        &self,
        _instrument: &dyn crate::instruments::Instrument,
        _direction: f64,
    ) -> Result<Option<Box<dyn crate::instruments::Instrument>>> {
        Ok(None)
    }
}

type BumperFactoryFn = fn(&MetricContext) -> Result<Option<Box<dyn FactorBumper>>>;

#[derive(Debug, Clone)]
struct ParallelCurveBumper {
    curve_ids: Vec<CurveId>,
    bump_bp: f64,
}

impl FactorBumper for ParallelCurveBumper {
    fn bump_market(
        &self,
        market: &MarketContext,
        _as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        let bumps: Vec<MarketBump> = self
            .curve_ids
            .iter()
            .map(|id| MarketBump::Curve {
                id: id.clone(),
                spec: BumpSpec::parallel_bp(self.bump_bp * direction),
            })
            .collect();
        market.bump(bumps)
    }

    fn bump_size(&self) -> f64 {
        self.bump_bp
    }

    fn is_applicable(&self, market: &MarketContext, _as_of: Date) -> bool {
        self.curve_ids.iter().any(|curve_id| {
            market.get_discount(curve_id.as_str()).is_ok()
                || market.get_forward(curve_id.as_str()).is_ok()
        })
    }
}

struct CreditQuoteBumper {
    curve_ids: Vec<CurveId>,
    bump_bp: f64,
    source_market: Arc<MarketContext>,
    provider: Arc<dyn crate::recalibration::RecalibrationProvider>,
}

impl FactorBumper for CreditQuoteBumper {
    fn bump_market(
        &self,
        market: &MarketContext,
        _as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        use crate::recalibration::{
            HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump,
        };
        let mut target = market.clone();
        for id in &self.curve_ids {
            let hazard = self.source_market.get_hazard(id.as_str())?;
            let discount_curve_id = self.provider.get_hazard_discount_curve_id(&hazard)?;
            let rebuilt = self
                .provider
                .rebuild_hazard_curve(&HazardRecalibrationRequest {
                    hazard,
                    source_market: Arc::clone(&self.source_market),
                    target_market: Arc::new(target.clone()),
                    discount_curve_id,
                    doc_clause: None,
                    cds_valuation_convention: None,
                    deal_quote_override: None,
                    action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(
                        self.bump_bp * direction,
                    )),
                })?;
            target = target.insert(rebuilt.as_ref().clone());
        }
        Ok(target)
    }
    fn bump_size(&self) -> f64 {
        self.bump_bp
    }
    fn is_applicable(&self, market: &MarketContext, _as_of: Date) -> bool {
        self.curve_ids
            .iter()
            .all(|id| market.get_hazard(id.as_str()).is_ok())
    }
    fn recalibrates_credit(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
struct VolParallelBumper {
    vol_surface_ids: Vec<CurveId>,
    vol_scalar_ids: Vec<String>,
    override_volatility: Option<f64>,
    bump_abs: f64,
    one_sided: bool,
}

impl FactorBumper for VolParallelBumper {
    fn stencil(&self) -> &'static [(f64, f64)] {
        if self.one_sided {
            &[(0.0, -1.5), (1.0, 2.0), (2.0, -0.5)]
        } else {
            &[(-1.0, -0.5), (1.0, 0.5)]
        }
    }

    fn bump_market(
        &self,
        market: &MarketContext,
        _as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        if self.override_volatility.is_some() {
            return Ok(market.clone());
        }
        let mut bumped =
            market.bump(
                self.vol_surface_ids
                    .iter()
                    .cloned()
                    .map(|id| MarketBump::Curve {
                        id,
                        spec: BumpSpec {
                            mode: finstack_quant_core::market_data::bumps::BumpMode::Additive,
                            units: finstack_quant_core::market_data::bumps::BumpUnits::Fraction,
                            value: self.bump_abs * direction,
                            bump_type: finstack_quant_core::market_data::bumps::BumpType::Parallel,
                        },
                    }),
            )?;
        for id in &self.vol_scalar_ids {
            let MarketScalar::Unitless(volatility) = market.get_price(id)? else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "volatility quote '{id}' must be a unitless scalar"
                )));
            };
            let value = volatility + self.bump_abs * direction;
            if !value.is_finite() || value < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "bumped volatility quote '{id}' must be finite and nonnegative"
                )));
            }
            bumped.insert_price_mut(id, MarketScalar::Unitless(value));
        }
        Ok(bumped)
    }

    fn bump_size(&self) -> f64 {
        self.bump_abs * 100.0
    }

    fn is_applicable(&self, market: &MarketContext, _as_of: Date) -> bool {
        self.override_volatility.is_some()
            || (self
                .vol_surface_ids
                .iter()
                .all(|vol_surface_id| market.get_surface(vol_surface_id.as_str()).is_ok())
                && self
                    .vol_scalar_ids
                    .iter()
                    .all(|id| matches!(market.get_price(id), Ok(MarketScalar::Unitless(_)))))
    }

    fn bump_instrument(
        &self,
        instrument: &dyn crate::instruments::Instrument,
        direction: f64,
    ) -> Result<Option<Box<dyn crate::instruments::Instrument>>> {
        let Some(volatility) = self.override_volatility else {
            return Ok(None);
        };
        let value = volatility + self.bump_abs * direction;
        if !value.is_finite() || value < 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "bumped volatility override must be finite and nonnegative".into(),
            ));
        }
        let mut bumped = instrument.clone_box();
        bumped
            .get_instrument_pricing_overrides_mut()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "active volatility override is not mutable".into(),
                )
            })?
            .market_quotes
            .implied_volatility = Some(value);
        Ok(Some(bumped))
    }
}

#[derive(Debug, Clone)]
struct SpotBumper {
    price_id: String,
    bump_pct: f64,
}

impl FactorBumper for SpotBumper {
    fn bump_market(
        &self,
        market: &MarketContext,
        _as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        bump_scalar_price(market, &self.price_id, self.bump_pct * direction)
    }

    fn bump_size(&self) -> f64 {
        self.bump_pct * 100.0
    }

    fn is_applicable(&self, market: &MarketContext, _as_of: Date) -> bool {
        market.get_price(&self.price_id).is_ok()
    }
}

#[derive(Debug, Clone)]
struct FxBumper {
    pairs: Vec<FxPair>,
    bump_pct: f64,
}

impl FactorBumper for FxBumper {
    fn bump_market(
        &self,
        market: &MarketContext,
        as_of: Date,
        direction: f64,
    ) -> Result<MarketContext> {
        let bumps: Vec<MarketBump> = self
            .pairs
            .iter()
            .map(|pair| MarketBump::FxPct {
                base: pair.base,
                quote: pair.quote,
                pct: self.bump_pct * direction * 100.0,
                as_of,
            })
            .collect();
        market.bump(bumps)
    }

    fn bump_size(&self) -> f64 {
        self.bump_pct * 100.0
    }

    fn is_applicable(&self, market: &MarketContext, as_of: Date) -> bool {
        self.pairs.iter().any(|pair| {
            market
                .fx()
                .and_then(|fx| fx.rate(FxQuery::new(pair.base, pair.quote, as_of)).ok())
                .is_some()
        })
    }
}

/// Create a rate bumper from runtime instrument dependencies.
pub(crate) fn make_rates_bumper(context: &MetricContext) -> Result<Option<Box<dyn FactorBumper>>> {
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

    let defaults = sens_config::from_context_or_default(
        context.get_config(),
        context.get_metric_pricing_overrides(),
    )?;
    Ok(Some(Box::new(ParallelCurveBumper {
        curve_ids,
        bump_bp: defaults.rate_bump_bp,
    })))
}

/// Create a credit bumper from runtime instrument dependencies.
pub(crate) fn make_credit_bumper(context: &MetricContext) -> Result<Option<Box<dyn FactorBumper>>> {
    let deps = context.instrument.market_dependencies()?;
    if deps.curves.credit_curves.is_empty() {
        return Ok(None);
    }

    let defaults = sens_config::from_context_or_default(
        context.get_config(),
        context.get_metric_pricing_overrides(),
    )?;
    for id in &deps.curves.credit_curves {
        super::cs01::require_hazard_replay(
            context.curves.get_hazard(id.as_str())?.as_ref(),
            "credit cross-gamma",
        )?;
    }
    Ok(Some(Box::new(CreditQuoteBumper {
        curve_ids: deps.curves.credit_curves.to_vec(),
        bump_bp: defaults.credit_spread_bump_bp,
        source_market: Arc::clone(&context.curves),
        provider: context.recalibration_provider("credit_cross_gamma")?,
    })))
}

/// Create a volatility bumper from runtime instrument dependencies.
pub(crate) fn make_vol_bumper(context: &MetricContext) -> Result<Option<Box<dyn FactorBumper>>> {
    let deps = context.instrument.market_dependencies()?;
    let override_volatility = volatility_override(context.instrument.as_ref());
    let vol_surface_ids = if override_volatility.is_some() {
        Vec::new()
    } else {
        deps.present_vol_surface_ids(&context.curves)
    };
    let mut vol_scalar_ids = Vec::new();
    if override_volatility.is_none() {
        for dependency in &deps.volatility_dependencies {
            let id = dependency.vol_surface_id.as_str();
            if matches!(context.curves.get_price(id), Ok(MarketScalar::Unitless(_)))
                && !vol_scalar_ids.iter().any(|existing| existing == id)
            {
                vol_scalar_ids.push(id.to_string());
            }
        }
    }
    if vol_surface_ids.is_empty() && vol_scalar_ids.is_empty() && override_volatility.is_none() {
        return Ok(None);
    }

    let defaults = sens_config::from_context_or_default(
        context.get_config(),
        context.get_metric_pricing_overrides(),
    )?;
    let mut minimum = override_volatility.unwrap_or(f64::INFINITY);
    for id in &vol_surface_ids {
        let surface = context.curves.get_surface(id.as_str())?;
        minimum = minimum.min(min_grid_vol(&surface).ok_or_else(|| {
            finstack_quant_core::Error::Validation("volatility surface has no finite grid".into())
        })?);
    }
    for id in &vol_scalar_ids {
        if let MarketScalar::Unitless(vol) = context.curves.get_price(id)? {
            minimum = minimum.min(*vol);
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
    Ok(Some(Box::new(VolParallelBumper {
        vol_surface_ids,
        vol_scalar_ids,
        override_volatility,
        bump_abs,
        one_sided,
    })))
}

/// Create a spot bumper from runtime instrument dependencies.
pub(crate) fn make_spot_bumper(context: &MetricContext) -> Result<Option<Box<dyn FactorBumper>>> {
    let deps = context.instrument.market_dependencies()?;
    let Some(price_id) = deps.market_scalar_ids.first() else {
        return Ok(None);
    };

    let defaults = sens_config::from_context_or_default(
        context.get_config(),
        context.get_metric_pricing_overrides(),
    )?;
    Ok(Some(Box::new(SpotBumper {
        price_id: price_id.clone(),
        bump_pct: defaults.spot_bump_decimal,
    })))
}

/// Create an FX bumper from runtime instrument dependencies.
pub(crate) fn make_fx_bumper(context: &MetricContext) -> Result<Option<Box<dyn FactorBumper>>> {
    let deps = context.instrument.market_dependencies()?;
    if deps.fx_pairs.is_empty() {
        return Ok(None);
    }

    let defaults = sens_config::from_context_or_default(
        context.get_config(),
        context.get_metric_pricing_overrides(),
    )?;
    Ok(Some(Box::new(FxBumper {
        pairs: deps.fx_pairs,
        bump_pct: defaults.spot_bump_decimal,
    })))
}

/// Generic cross-factor calculator using runtime bumper factories.
pub struct CrossFactorCalculator {
    factory_a: BumperFactoryFn,
    factory_b: BumperFactoryFn,
}

impl CrossFactorCalculator {
    /// Creates a reusable cross-factor calculator from two bumper factories.
    pub(crate) fn new(factory_a: BumperFactoryFn, factory_b: BumperFactoryFn) -> Self {
        Self {
            factory_a,
            factory_b,
        }
    }
}

impl MetricCalculator for CrossFactorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        // NOTE: There is deliberately NO short-circuit here that returns a
        // pre-computed `Vanna` metric in place of `CrossGammaSpotVol`.
        //
        // `Vanna` is defined as ∂²V/(∂S_abs × ∂σ) — per unit spot, per vol
        // point. `CrossGammaSpotVol` (computed below via four-corner FD) is
        // normalised by percentage-point moves in both factors (per 1 pct-pt spot,
        // per 1 pct-pt vol), which differs by a factor of S₀ / 100.  Returning
        // `Vanna` directly would silently mis-scale every cross-gamma attribution
        // that uses `CrossGammaSpotVol × avg_spot_shift_pct × avg_vol_shift_pct`.
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
                || reprice_corner(context, market, as_of, &*bumper_a, &*bumper_b, 1.0, 1.0),
                || reprice_corner(context, market, as_of, &*bumper_a, &*bumper_b, 1.0, -1.0),
                || reprice_corner(context, market, as_of, &*bumper_a, &*bumper_b, -1.0, 1.0),
                || reprice_corner(context, market, as_of, &*bumper_a, &*bumper_b, -1.0, -1.0),
                h_abs,
                k_abs,
            );
        }
        let mut value = finstack_quant_core::math::NeumaierAccumulator::new();
        for &(direction_a, weight_a) in bumper_a.stencil() {
            for &(direction_b, weight_b) in bumper_b.stencil() {
                value.add(
                    weight_a
                        * weight_b
                        * reprice_corner(
                            context,
                            market,
                            as_of,
                            &*bumper_a,
                            &*bumper_b,
                            direction_a,
                            direction_b,
                        )?,
                );
            }
        }
        Ok(value.total() / (h_abs * k_abs))
    }

    fn dependencies(&self) -> &[MetricId] {
        NO_DEPS
    }
}

/// Reprice after applying two sequential market bumps.
///
/// Uses `reprice_raw` (not `reprice_money`) because scenario overrides cancel
/// in the four-corner finite-difference stencil and avoiding the `Money` wrapper
/// is consistent with other sensitivity calculators (DV01, CS01).
///
/// Keep this copy-returning path until all factor types have reversible
/// in-place bump tokens. Curves, surfaces, and prices do, but FX currently
/// goes through `MarketContext::bump`, so a partial scratch abstraction would
/// add branching without eliminating the clone path for FX cross-gammas.
fn reprice_corner(
    context: &MetricContext,
    market: &MarketContext,
    as_of: Date,
    bumper_a: &dyn FactorBumper,
    bumper_b: &dyn FactorBumper,
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
    use crate::metrics::MetricId;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::types::CurveId;

    use super::{CrossFactorPair, FactorBumper, VolParallelBumper};

    fn flat_surface(id: &str, vol: f64) -> VolSurface {
        VolSurface::builder(id)
            .expiries(&[1.0])
            .strikes(&[100.0])
            .row(&[vol])
            .build()
            .expect("surface")
    }

    #[test]
    fn parallel_vol_bumper_moves_every_resolved_surface() {
        let market = MarketContext::new()
            .insert_surface(flat_surface("VOL-A", 0.20))
            .insert_surface(flat_surface("VOL-B", 0.30));
        let bumper = VolParallelBumper {
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

    #[test]
    fn cross_factor_pair_metric_id_roundtrip() {
        let expected = [
            (
                CrossFactorPair::RatesCredit,
                MetricId::CrossGammaRatesCredit,
            ),
            (CrossFactorPair::RatesVol, MetricId::CrossGammaRatesVol),
            (CrossFactorPair::SpotVol, MetricId::CrossGammaSpotVol),
            (CrossFactorPair::SpotCredit, MetricId::CrossGammaSpotCredit),
            (CrossFactorPair::FxVol, MetricId::CrossGammaFxVol),
            (CrossFactorPair::FxRates, MetricId::CrossGammaFxRates),
            (CrossFactorPair::CreditVol, MetricId::CrossGammaCreditVol),
        ];

        for (pair, metric_id) in expected {
            assert_eq!(pair.metric_id(), metric_id);
            assert!(!pair.metric_id().is_custom());
        }
    }

    #[test]
    fn cross_factor_pair_labels_are_nonempty() {
        for pair in CrossFactorPair::ALL {
            assert!(!pair.metric_id().as_str().is_empty());
        }
    }
    #[test]
    fn near_zero_and_zero_volatility_preserve_bilinear_cross_derivative() {
        use crate::instruments::EquityOption;
        use crate::metrics::MetricContext;
        use finstack_quant_core::{currency::Currency, money::Money};
        use std::sync::Arc;
        for vol in [0.0, 0.005, 0.02] {
            let option = EquityOption::example().unwrap();
            let market = MarketContext::new()
                .insert_surface(flat_surface(option.vol_surface_id.as_str(), vol));
            let context = MetricContext::new(
                Arc::new(option),
                Arc::new(market.clone()),
                Date::MIN,
                Money::new(0.0, Currency::USD).unwrap(),
                MetricContext::default_config(),
            );
            let bumper = super::make_vol_bumper(&context).unwrap().unwrap();
            let mut result = 0.0;
            for (ds, ws) in [(-1.0, -0.5), (1.0, 0.5)] {
                for &(dv, wv) in bumper.stencil() {
                    let shifted = bumper.bump_market(&market, Date::MIN, dv).unwrap();
                    let sigma = shifted.get_surface("EQUITY-VOL").unwrap().vols()[0];
                    // V(S,sigma) = S*sigma, expressed per spot and vol point.
                    result += ws * wv * (100.0 + ds) * sigma;
                }
            }
            result /= bumper.bump_size();
            assert!((result - 0.01).abs() < 1e-12, "vol={vol}: {result}");
        }
    }
}
