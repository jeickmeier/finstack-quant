//! Pricing, Greeks, and [`Instrument`] trait implementations for [`CommoditySpotOption`].

use super::types::*;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_models::trees::binomial_tree::BinomialTree;
use finstack_quant_models::types::OptionMarketParams;
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::SettlementType;

pub(super) struct CommoditySpotOptionInputs {
    pub(super) forward: f64,
    pub(super) spot: f64,
    pub(super) r: f64,
    pub(super) q: f64,
    pub(super) sigma: f64,
    pub(super) t: f64,
    pub(super) df: f64,
}

pub(super) fn implied_carry(spot: f64, forward: f64, r: f64, t: f64) -> f64 {
    // Guard against near-zero time: ln(F/S)/t amplifies noise when t is tiny.
    // For t < 1e-5 (~5 seconds), the carry estimate is unreliable.
    //
    // Also guard against non-finite `forward` / `spot` (W-13): a NaN or
    // infinite input passes the `<= 0.0` checks (NaN comparisons are false)
    // and would otherwise feed `(forward/spot).ln()/t` — and hence `q` — a
    // non-finite value straight into the binomial tree. Fall back to the
    // risk-free rate (zero cost-of-carry) when any input is unusable.
    if t < 1e-5
        || !t.is_finite()
        || !spot.is_finite()
        || !forward.is_finite()
        || spot <= 0.0
        || forward <= 0.0
    {
        return r;
    }
    let carry = (forward / spot).ln() / t;
    if carry.is_finite() { r - carry } else { r }
}

impl Instrument for CommoditySpotOption {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    impl_instrument_base!(
        finstack_quant_valuations_core::pricer::InstrumentType::CommoditySpotOption,
        crate::EXOTICS
    );

    fn default_model(&self) -> finstack_quant_valuations_core::pricer::ModelKey {
        finstack_quant_valuations_core::pricer::ModelKey::Black76
    }

    fn validate_invariants(&self) -> Result<()> {
        self.validate()
    }

    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::MarketDependencies>
    {
        let mut deps = finstack_quant_valuations_core::instruments::MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        deps.add_forward_curve(self.forward_curve_id.clone());
        let underlying_id = self
            .spot_id
            .as_deref()
            .map(finstack_quant_core::types::PriceId::new);
        if let Some(spot_id) = self.spot_id.as_deref() {
            deps.add_market_scalar_id(spot_id);
        }
        deps.add_volatility_dependency(
            finstack_quant_valuations_core::instruments::VolatilityDependency::new(
                self.vol_surface_id.clone(),
                underlying_id,
                Some(self.strike),
            ),
        );
        Ok(deps)
    }

    #[tracing::instrument(
        name = "commodity_spot_option.base_value",
        level = "debug",
        skip(self, market),
        fields(inst_id = %self.id(), as_of = %as_of),
        err,
    )]
    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.validate()?;
        if let Some(claim) = self.resolved_claim(market, as_of)? {
            return Ok(claim);
        }

        let inputs = self.collect_inputs(market, as_of)?;

        let unit_price = match self.exercise_style {
            ExerciseStyle::European => {
                if !inputs.forward.is_finite() || inputs.forward <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "CommoditySpotOption '{}' Black-76 forward must be finite and positive, got {}",
                        self.id, inputs.forward
                    )));
                }
                finstack_quant_models::closed_form::black76_price(
                    inputs.forward,
                    self.strike,
                    inputs.df,
                    inputs.sigma,
                    inputs.t,
                    self.option_type,
                )?
            }
            ExerciseStyle::American => {
                let steps = self
                    .instrument_pricing_overrides
                    .model_config
                    .tree_steps
                    .unwrap_or(201);
                let tree = BinomialTree::new(
                    steps,
                    finstack_quant_models::trees::binomial_tree::TreeType::LeisenReimer,
                );
                let params = OptionMarketParams {
                    spot: inputs.spot,
                    strike: self.strike,
                    rate: inputs.r,
                    dividend_yield: inputs.q,
                    volatility: inputs.sigma,
                    time_to_expiry: inputs.t,
                    option_type: self.option_type,
                };
                params.validate()?;
                tree.price_american(&params, &[])?
            }
            ExerciseStyle::Bermudan => {
                let schedule = self.exercise_dates.as_ref().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "Commodity Bermudan option requires exercise_dates".to_string(),
                    )
                })?;
                let steps = self
                    .instrument_pricing_overrides
                    .model_config
                    .tree_steps
                    .unwrap_or(201);
                let tree = BinomialTree::new(
                    steps,
                    finstack_quant_models::trees::binomial_tree::TreeType::LeisenReimer,
                );
                let params = OptionMarketParams {
                    spot: inputs.spot,
                    strike: self.strike,
                    rate: inputs.r,
                    dividend_yield: inputs.q,
                    volatility: inputs.sigma,
                    time_to_expiry: inputs.t,
                    option_type: self.option_type,
                };
                params.validate()?;
                let exercise_times: Vec<f64> = schedule
                    .iter()
                    .filter_map(|date| {
                        let yf = self
                            .day_count
                            .year_fraction(as_of, *date, DayCountContext::default())
                            .ok()?;
                        if yf > 0.0 && yf <= inputs.t {
                            Some(yf)
                        } else {
                            None
                        }
                    })
                    .collect();
                if exercise_times.is_empty() {
                    return Err(finstack_quant_core::Error::Validation(
                        "Commodity Bermudan option requires at least one future exercise date"
                            .to_string(),
                    ));
                }
                tree.price_bermudan(&params, &exercise_times, &[])?
            }
        };

        Money::from_model_value(
            unit_price * self.quantity * self.multiplier,
            self.underlying.currency,
        )
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}

impl finstack_quant_valuations_core::instruments::OptionGreeksProvider for CommoditySpotOption {
    fn selected_model_greek(
        &self,
        kind: finstack_quant_valuations_core::instruments::OptionGreekKind,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ) -> finstack_quant_core::Result<Option<f64>> {
        // Once exercise resolves the spot option, every pricing model owns the
        // same remaining delivery claim and its contractual forward exposure.
        if context.as_of >= self.expiry
            || self
                .exercise
                .is_some_and(|event| event.date <= context.as_of)
        {
            return Ok(None);
        }
        super::metrics::selected_model::selected_commodity_greek(kind, context).map(Some)
    }

    fn option_greeks(
        &self,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        _request: &finstack_quant_valuations_core::instruments::OptionGreeksRequest,
    ) -> finstack_quant_core::Result<finstack_quant_valuations_core::instruments::OptionGreeks>
    {
        if self.resolved_claim(market, as_of)?.is_some() {
            let delta = if let Some(exercise) = self.exercise.filter(|event| event.date <= as_of) {
                if exercise.exercised
                    && as_of <= exercise.settlement_date
                    && self.settlement == SettlementType::Physical
                {
                    let df = market
                        .get_discount(&self.discount_curve_id)?
                        .df_between_dates(as_of, exercise.settlement_date)?;
                    (if self.option_type == OptionType::Call {
                        1.0
                    } else {
                        -1.0
                    }) * df
                        * self.quantity
                        * self.multiplier
                } else {
                    0.0
                }
            } else {
                let spot = if self.spot_id.is_some() {
                    self.required_spot(market)?
                } else {
                    self.forward_price(market, as_of)?
                };
                let sign = if self.option_type == OptionType::Call {
                    1.0
                } else {
                    -1.0
                };
                if sign * (spot - self.strike) > 0.0 {
                    sign * self.quantity * self.multiplier
                } else {
                    0.0
                }
            };
            return Ok(finstack_quant_valuations_core::instruments::OptionGreeks {
                delta: Some(delta),
                gamma: Some(0.0),
                vega: Some(0.0),
                theta: None,
                rho_bp: None,
                foreign_rho_bp: None,
                vanna: Some(0.0),
                volga: Some(0.0),
            });
        }
        if as_of > self.expiry {
            return Ok(finstack_quant_valuations_core::instruments::OptionGreeks {
                delta: Some(0.0),
                gamma: Some(0.0),
                vega: Some(0.0),
                theta: None,
                rho_bp: None,
                foreign_rho_bp: None,
                vanna: Some(0.0),
                volga: Some(0.0),
            });
        }
        if !matches!(self.exercise_style, ExerciseStyle::European) {
            return Err(finstack_quant_core::Error::Validation(
                "CommoditySpotOption analytical Greeks support European exercise only".to_string(),
            ));
        }

        let t = self
            .day_count
            .year_fraction(as_of, self.expiry, DayCountContext::default())?
            .max(0.0);
        let forward = self.forward_price(market, as_of)?;
        if t <= 0.0 {
            return crate::commodity::black76_greeks::black76_option_greeks(
                forward,
                self.strike,
                1.0,
                0.0,
                0.0,
                self.option_type,
                self.quantity * self.multiplier,
            );
        }

        let sigma =
            finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
                &self.instrument_pricing_overrides.market_quotes,
                market,
                self.vol_surface_id.as_str(),
                t,
                self.strike,
            )?;
        let df = market
            .get_discount(self.discount_curve_id.as_str())?
            .df_between_dates(as_of, self.expiry)?;
        crate::commodity::black76_greeks::black76_option_greeks(
            forward,
            self.strike,
            df,
            sigma,
            t,
            self.option_type,
            self.quantity * self.multiplier,
        )
    }
}

finstack_quant_valuations_core::impl_empty_cashflow_provider!(
    CommoditySpotOption,
    finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
);
