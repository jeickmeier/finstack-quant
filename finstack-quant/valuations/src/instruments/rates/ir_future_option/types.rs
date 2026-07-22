//! IR Future Option types and implementation.
//!
//! Exchange-traded options on interest rate futures (e.g., SOFR futures options).
//! Priced on the futures price (100 - rate) under the configured
//! [`VolatilityModel`]: Black-76 (lognormal, the historical default) or
//! Bachelier (normal), which matches the normal (basis-point) vol quotation
//! standard for CME SOFR/STIR options.
//!
//! # Pricing
//!
//! Forward = futures_price (no convexity adjustment needed since the option
//! is on the future itself). Premium is discounted from expiry to today.
//!
//! Black-76 (lognormal vol, decimal of the futures price):
//!
//! ```text
//! Call = DF × [F·N(d₁) - K·N(d₂)]
//! Put  = DF × [K·N(-d₂) - F·N(-d₁)]
//! ```
//!
//! where d₁ = [ln(F/K) + σ²T/2] / (σ√T), d₂ = d₁ - σ√T.
//!
//! Bachelier (normal vol in **price points**, e.g. 0.60 ≈ 60 bp of rate vol):
//!
//! ```text
//! Call = DF × [(F - K)·N(d) + σ√T·n(d)],  d = (F - K) / (σ√T)
//! ```
//!
//! PV additionally scales by the contract count `notional / face_value`,
//! consistent with [`InterestRateFuture`](crate::instruments::rates::ir_future)
//! (notional = N × face means an N-contract position).
//!
//! # Market Conventions
//!
//! - **SOFR options**: quoted in price points (e.g., 0.25 = 25 ticks); vols
//!   quoted normal (basis points) — use [`VolatilityModel::Normal`]
//! - **Tick sizes**: 0.0025 for 1M SOFR ($6.25), 0.0025 for 3M SOFR ($6.25)
//! - **Exercise**: American-style on CME, but priced as European (early exercise
//!   is rarely optimal for futures options)
//!
//! # References
//!
//! - Black, F. (1976). "The pricing of commodity contracts."
//!   *Journal of Financial Economics*, 3(1-2), 167-179.
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapters 18 (options on futures) and 29 (normal model).

use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Attributes;
use crate::instruments::rates::swaption::VolatilityModel;
use crate::models::volatility::black::{d1_black76, d1_d2_black76};
use crate::models::volatility::normal::{bachelier_price, d_bachelier};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::{norm_cdf, norm_pdf};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};

/// Exchange-traded option on an interest rate future (e.g., SOFR futures).
///
/// Priced using Black-76 on the futures price. The underlying is the futures
/// price itself (100 - rate), so no convexity adjustment is needed.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    finstack_quant_valuations_macros::FocusedPricingOverrides,
)]
#[builder(validate = IrFutureOption::validate)]
#[serde(deny_unknown_fields)]
pub struct IrFutureOption {
    /// Unique identifier
    pub id: InstrumentId,
    /// Underlying futures price (e.g., 95.50 for a 4.50% implied rate)
    pub futures_price: f64,
    /// Option strike price (in futures price terms, e.g., 95.00)
    pub strike: f64,
    /// Option expiry date
    #[schemars(with = "String")]
    pub expiry: Date,
    /// Call or Put
    pub option_type: OptionType,
    /// Notional amount per contract
    pub notional: Money,
    /// Tick size (e.g., 0.0025 for SOFR options)
    pub tick_size: f64,
    /// Tick value in currency units (e.g., $6.25 for 1M SOFR, $25 for 3M)
    pub tick_value: f64,
    /// Face value of one underlying futures contract (e.g., $1,000,000 for
    /// SOFR futures). The position size is `notional / face_value` contracts,
    /// consistent with [`InterestRateFuture`](crate::instruments::rates::ir_future).
    #[builder(default = 1_000_000.0)]
    #[serde(default = "default_face_value")]
    pub face_value: f64,
    /// Annualized volatility. Units follow `vol_model`: a decimal lognormal
    /// vol of the futures price for [`VolatilityModel::Black`] (e.g. `0.006`),
    /// or a normal vol in **price points** for [`VolatilityModel::Normal`]
    /// (e.g. `0.60` ≈ 60 bp of annualized rate vol).
    pub volatility: f64,
    /// Volatility model the quote refers to. CME SOFR/STIR options are quoted
    /// in normal (basis-point) vols — use [`VolatilityModel::Normal`] for
    /// those. Defaults to Black (lognormal) for backward compatibility.
    #[builder(default)]
    #[serde(default)]
    pub vol_model: VolatilityModel,
    /// Discount curve ID for PV calculation
    pub discount_curve_id: CurveId,
    /// Pricing overrides
    #[serde(default)]
    #[builder(default)]
    /// Instrument-owned pricing inputs.
    pub instrument_pricing_overrides: crate::instruments::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[serde(default)]
    #[builder(default)]
    pub metric_pricing_overrides: crate::instruments::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[serde(default)]
    #[builder(default)]
    pub scenario_pricing_overrides: crate::instruments::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

/// Default face value of one futures contract ($1,000,000, CME SOFR standard).
fn default_face_value() -> f64 {
    1_000_000.0
}

impl IrFutureOption {
    /// Validate pricing inputs and contract scaling.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let context = format!("IR future option '{}'", self.id.as_str());
        for (field, value) in [
            ("futures_price", self.futures_price),
            ("strike", self.strike),
            ("notional", self.notional.amount()),
            ("tick_size", self.tick_size),
            ("tick_value", self.tick_value),
            ("face_value", self.face_value),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} {field} must be positive and finite"
                )));
            }
        }
        if !self.volatility.is_finite() || self.volatility < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} volatility must be non-negative and finite"
            )));
        }
        if self.discount_curve_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty discount_curve_id"
            )));
        }
        Ok(())
    }

    /// Time to expiry in years from `as_of`, using Act/365F.
    ///
    /// Returns `Ok(0.0)` for expired options (`as_of >= expiry`).
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure rather than swallowing it as `0.0`: a
    /// spurious zero would silently mis-classify a live option as expired and
    /// drop all of its time value with no diagnostic.
    fn time_to_expiry(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        if as_of >= self.expiry {
            return Ok(0.0);
        }
        DayCount::Act365F.year_fraction(as_of, self.expiry, DayCountContext::default())
    }

    /// Whether this is a call option.
    fn is_call(&self) -> bool {
        matches!(self.option_type, OptionType::Call)
    }

    /// Compute intrinsic value of the option (no discounting).
    fn intrinsic_value(&self) -> f64 {
        if self.is_call() {
            (self.futures_price - self.strike).max(0.0)
        } else {
            (self.strike - self.futures_price).max(0.0)
        }
    }

    /// Currency PV per 1.0 futures price point for one contract.
    fn contract_point_value(&self) -> finstack_quant_core::Result<f64> {
        if !self.tick_size.is_finite() || self.tick_size <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IR future option tick_size must be positive and finite; got {}",
                self.tick_size
            )));
        }
        if !self.tick_value.is_finite() || self.tick_value <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IR future option tick_value must be positive and finite; got {}",
                self.tick_value
            )));
        }
        Ok(self.tick_value / self.tick_size)
    }

    /// Number of futures contracts the position represents
    /// (`notional / face_value`), mirroring `InterestRateFuture`.
    fn contracts_scale(&self) -> f64 {
        self.notional.amount() / self.face_value
    }

    /// Undiscounted option premium under the configured vol model, plus the
    /// discount factor and time to expiry.
    ///
    /// Returns `(undiscounted_premium, discount_factor, time_to_expiry)`.
    fn premium_components(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, f64, f64)> {
        let t = self.time_to_expiry(as_of)?;
        let disc = context.get_discount(&self.discount_curve_id)?;
        let df = relative_df_discount_curve(disc.as_ref(), as_of, self.expiry)?;

        if t <= 0.0 || self.volatility <= 0.0 || !self.volatility.is_finite() {
            return Ok((self.intrinsic_value(), df, t));
        }

        let premium = match self.vol_model {
            VolatilityModel::Black => {
                if self.futures_price <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Black-76 requires positive futures price; got {}",
                        self.futures_price
                    )));
                }

                let (d1, d2) = d1_d2_black76(self.futures_price, self.strike, self.volatility, t);

                if !d1.is_finite() || !d2.is_finite() {
                    tracing::warn!(
                        futures_price = self.futures_price,
                        strike = self.strike,
                        sigma = self.volatility,
                        t = t,
                        "Black-76 d1/d2 non-finite; falling back to intrinsic"
                    );
                    return Ok((self.intrinsic_value(), df, t));
                }

                if self.is_call() {
                    self.futures_price * norm_cdf(d1) - self.strike * norm_cdf(d2)
                } else {
                    self.strike * norm_cdf(-d2) - self.futures_price * norm_cdf(-d1)
                }
            }
            // Normal (Bachelier) model on the futures price; annuity 1.0 —
            // discounting and contract scaling are applied by the caller.
            VolatilityModel::Normal => bachelier_price(
                self.option_type,
                self.futures_price,
                self.strike,
                self.volatility,
                t,
                1.0,
            ),
        };

        Ok((premium, df, t))
    }

    /// Present value of the position (all contracts).
    pub fn npv(&self, context: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of > self.expiry {
            return Ok(0.0);
        }
        let (premium, df, _t) = self.premium_components(context, as_of)?;
        let pv = df * premium * self.contract_point_value()? * self.contracts_scale();
        if !pv.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "IR future option produced non-finite PV: F={}, K={}, σ={}, id={}",
                self.futures_price, self.strike, self.volatility, self.id,
            )));
        }
        Ok(pv)
    }

    /// The model's `d` argument at the current market state: `d₁` under Black,
    /// `d = (F − K)/(σ√T)` under Bachelier. Callers must ensure `t > 0` and
    /// `volatility > 0`.
    fn model_d(&self, t: f64) -> f64 {
        match self.vol_model {
            VolatilityModel::Black => {
                d1_black76(self.futures_price, self.strike, self.volatility, t)
            }
            VolatilityModel::Normal => {
                d_bachelier(self.futures_price, self.strike, self.volatility, t)
            }
        }
    }

    /// Forward delta (sensitivity to futures price).
    ///
    /// Call: N(d), Put: N(d) − 1, where `d` is `d₁` (Black) or the Bachelier
    /// `d` (Normal) — the delta expression is the same in both models.
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from internal time-to-expiry calculation.
    pub fn delta(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of > self.expiry {
            return Ok(0.0);
        }
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.volatility <= 0.0 {
            if self.is_call() {
                return Ok(if self.futures_price > self.strike {
                    1.0
                } else {
                    0.0
                });
            } else {
                return Ok(if self.futures_price < self.strike {
                    -1.0
                } else {
                    0.0
                });
            }
        }
        let d = self.model_d(t);
        Ok(if self.is_call() {
            norm_cdf(d)
        } else {
            norm_cdf(d) - 1.0
        })
    }

    /// Gamma (second derivative w.r.t. futures price).
    ///
    /// Black: n(d₁) / (F·σ·√T). Normal: n(d) / (σ·√T).
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from internal time-to-expiry calculation.
    pub fn gamma(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.volatility <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d = self.model_d(t);
        let denom = match self.vol_model {
            VolatilityModel::Black => self.futures_price * self.volatility * t.sqrt(),
            VolatilityModel::Normal => self.volatility * t.sqrt(),
        }
        .max(1e-12);
        Ok(norm_pdf(d) / denom)
    }

    /// Vega per 0.01 absolute change in volatility (1 vol point).
    ///
    /// Black: F·√T·n(d₁) / 100. Normal: √T·n(d) / 100.
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from internal time-to-expiry calculation.
    pub fn vega_per_pct(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d = if self.volatility > 0.0 {
            self.model_d(t)
        } else {
            0.0
        };
        let dprice_dvol = match self.vol_model {
            VolatilityModel::Black => self.futures_price * t.sqrt() * norm_pdf(d),
            VolatilityModel::Normal => t.sqrt() * norm_pdf(d),
        };
        Ok(dprice_dvol / 100.0)
    }

    /// Volatility time-decay component of theta, per calendar day
    /// (undiscounted; the discount-decay term `r·PV` is added in
    /// [`OptionGreeksProvider::option_theta`]).
    ///
    /// Black: −F·σ·n(d₁)/(2√T) per year. Normal: −σ·n(d)/(2√T) per year.
    /// Divided by 365.25 for daily.
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from internal time-to-expiry calculation.
    pub fn theta_daily(&self, as_of: Date) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.volatility <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d = self.model_d(t);
        let annual_theta = match self.vol_model {
            VolatilityModel::Black => {
                -self.futures_price * self.volatility * norm_pdf(d) / (2.0 * t.sqrt())
            }
            VolatilityModel::Normal => -self.volatility * norm_pdf(d) / (2.0 * t.sqrt()),
        };
        Ok(annual_theta / 365.25)
    }

    /// Analytic parallel rate DV01: PV change for a +1bp parallel shift of
    /// rates, including **both** channels a curve bump moves:
    ///
    /// - the futures-price delta channel: price = 100 − rate, so +1bp shifts
    ///   the futures price by −0.01 price points;
    /// - the premium discounting channel: DF(expiry) shrinks by `t·1e-4`.
    ///
    /// The generic curve-bump DV01 cannot see the first (dominant) channel
    /// because `futures_price` is an exogenous market quote, not derived from
    /// a curve — which is why this instrument registers an analytic DV01.
    ///
    /// # Errors
    ///
    /// Propagates discount-curve lookup and day-count failures.
    pub fn analytic_rate_dv01(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if as_of > self.expiry {
            return Ok(0.0);
        }
        let t = self.time_to_expiry(as_of)?;
        let npv = self.npv(context, as_of)?;
        let disc = context.get_discount(&self.discount_curve_id)?;
        let df = relative_df_discount_curve(disc.as_ref(), as_of, self.expiry)?;

        // Futures-price channel: dF = −0.01 price points per +1bp.
        let delta_channel = self.delta(as_of)?
            * (-0.01)
            * df
            * self.contract_point_value()?
            * self.contracts_scale();
        // Discounting channel: d(DF) = −t·1e-4·DF per +1bp.
        let discount_channel = -t * 1e-4 * npv;
        Ok(delta_channel + discount_channel)
    }

    /// Create a canonical example 3M SOFR futures option.
    pub fn example() -> finstack_quant_core::Result<Self> {
        use time::macros::date;
        let futures_specs = crate::instruments::rates::ir_future::FutureContractSpecs::default();
        IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-SOFR-3M-CALL-9550"))
            .futures_price(95.50)
            .strike(95.50)
            .expiry(date!(2025 - 06 - 16))
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(futures_specs.tick_size)
            .tick_value(futures_specs.tick_value)
            .volatility(0.20)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
    }
}

impl crate::instruments::common_impl::traits::Instrument for IrFutureOption {
    impl_instrument_base!(crate::pricer::InstrumentType::IrFutureOption);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let mut deps = MarketDependencies::new();
        deps.add_discount_curve(self.discount_curve_id.clone());
        Ok(deps)
    }

    fn base_value(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        let pv = self.npv(curves, as_of)?;
        Ok(Money::new(pv, self.notional.currency()))
    }

    fn base_value_raw(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        self.npv(curves, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(f64, finstack_quant_core::currency::Currency)> {
        Ok((self.npv(curves, as_of)?, self.notional.currency()))
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    crate::impl_focused_pricing_overrides!();
}

impl finstack_quant_cashflows::CashflowScheduleSource for IrFutureOption {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                meta: crate::cashflow::builder::CashFlowMeta {
                    representation: crate::cashflow::builder::CashflowRepresentation::Placeholder,
                    ..Default::default()
                },
            },
        ))
    }
}

impl IrFutureOption {
    fn cash_scale(&self, market: &MarketContext, as_of: Date) -> finstack_quant_core::Result<f64> {
        let df = if as_of >= self.expiry {
            1.0
        } else {
            market
                .get_discount(&self.discount_curve_id)?
                .df_between_dates(as_of, self.expiry)?
        };
        Ok(df * self.contract_point_value()? * self.contracts_scale())
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for IrFutureOption {
    fn option_delta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.delta(as_of)? * self.cash_scale(market, as_of)?))
    }

    fn option_gamma(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(self.gamma(as_of)? * self.cash_scale(market, as_of)?))
    }

    fn option_vega(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(Some(
            self.vega_per_pct(as_of)? * self.cash_scale(market, as_of)?,
        ))
    }

    fn option_theta(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        // Full Black-76-style theta: DF·(vol decay) + r·DF·premium (Hull
        // Ch. 18). The second term is the discount-decay of the premium: as
        // the valuation date rolls forward the premium is discounted over a
        // shorter horizon, so PV drifts up at the financing rate.
        let vol_decay = self.theta_daily(as_of)? * self.cash_scale(market, as_of)?;
        let t = self.time_to_expiry(as_of)?;
        let discount_decay = if t > 0.0 {
            let df = market
                .get_discount(&self.discount_curve_id)?
                .df_between_dates(as_of, self.expiry)?;
            if df > 0.0 && df.is_finite() {
                let r = -df.ln() / t;
                r * self.npv(market, as_of)? / 365.25
            } else {
                0.0
            }
        } else {
            0.0
        };
        Ok(Some(vol_decay + discount_decay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn example_constructs_successfully() {
        let opt = IrFutureOption::example().expect("IrFutureOption example is valid");
        assert_eq!(opt.id.as_str(), "IRFO-SOFR-3M-CALL-9550");
        assert_eq!(opt.futures_price, 95.50);
        assert_eq!(opt.strike, 95.50);
    }

    #[test]
    fn atm_call_delta_near_half() {
        let opt = IrFutureOption::example().expect("IrFutureOption example is valid");
        let delta = opt.delta(date!(2025 - 01 - 15)).expect("delta");
        // ATM call delta should be close to 0.5
        assert!((delta - 0.5).abs() < 0.1, "ATM call delta = {delta}");
    }

    #[test]
    fn put_delta_negative() {
        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-PUT"))
            .futures_price(95.50)
            .strike(95.50)
            .expiry(date!(2025 - 06 - 16))
            .option_type(OptionType::Put)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(0.20)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        let delta = opt.delta(date!(2025 - 01 - 15)).expect("delta");
        assert!(delta < 0.0, "Put delta should be negative: {delta}");
    }

    #[test]
    fn deep_itm_call_delta_near_one() {
        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-DITM"))
            .futures_price(96.00)
            .strike(90.00)
            .expiry(date!(2025 - 06 - 16))
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(0.01) // low vol to make moneyness dominant
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        let delta = opt.delta(date!(2025 - 01 - 15)).expect("delta");
        assert!(delta > 0.95, "Deep ITM call delta = {delta}");
    }

    #[test]
    fn gamma_is_non_negative() {
        let opt = IrFutureOption::example().expect("IrFutureOption example is valid");
        let gamma = opt.gamma(date!(2025 - 01 - 15)).expect("gamma");
        assert!(gamma >= 0.0, "Gamma should be non-negative: {gamma}");
    }

    #[test]
    fn vega_is_non_negative() {
        let opt = IrFutureOption::example().expect("IrFutureOption example is valid");
        let vega = opt.vega_per_pct(date!(2025 - 01 - 15)).expect("vega");
        assert!(vega >= 0.0, "Vega should be non-negative: {vega}");
    }

    #[test]
    fn expired_option_has_zero_delta() {
        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-EXPIRED"))
            .futures_price(96.00)
            .strike(95.00)
            .expiry(date!(2025 - 01 - 01))
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(0.20)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        // as_of is after expiry
        let delta = opt.delta(date!(2025 - 03 - 01)).expect("delta");
        assert_eq!(delta, 0.0, "Expired option should have no remaining delta");
    }

    #[test]
    fn npv_scales_by_tick_economics_and_contract_count() {
        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("flat zero-rate curve"),
        );

        let build = |notional: f64| {
            IrFutureOption::builder()
                .id(InstrumentId::new("IRFO-CONTRACT-SCALE"))
                .futures_price(96.0)
                .strike(95.0)
                .expiry(expiry)
                .option_type(OptionType::Call)
                .notional(Money::new(notional, Currency::USD))
                .tick_size(0.0025)
                .tick_value(6.25)
                .volatility(0.0)
                .discount_curve_id(CurveId::new("USD-OIS"))
                .build()
                .expect("build")
        };

        // One contract (notional == default face_value of $1MM): pure tick economics.
        let pv_one = build(1_000_000.0).npv(&market, as_of).expect("pv");
        let expected = (96.0 - 95.0) * (6.25 / 0.0025);
        assert!(
            (pv_one - expected).abs() < 1e-9,
            "single-contract PV must equal tick economics: expected {expected}, got {pv_one}"
        );

        // Five contracts (notional 5x face): PV must scale by the contract
        // count, consistent with `InterestRateFuture`'s notional/face scaling.
        let pv_five = build(5_000_000.0).npv(&market, as_of).expect("pv");
        assert!(
            (pv_five - 5.0 * pv_one).abs() < 1e-9,
            "5x-notional book must carry 5x the single-contract PV: got {pv_five} vs 5x{pv_one}"
        );
    }

    /// Put-call parity `C − P = DF·(F − K)·point_value·contracts` must hold for
    /// both volatility models (Hull Ch. 18; model-free identity).
    #[test]
    fn put_call_parity_holds_for_black_and_normal_models() {
        use crate::instruments::rates::swaption::VolatilityModel;

        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let flat_rate = 0.04_f64;
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (5.0, (-flat_rate * 5.0).exp())])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .build()
                .expect("curve"),
        );

        for (model, vol) in [
            (VolatilityModel::Black, 0.006),
            // Normal vol in price points: 0.60 price points ≈ 60 bp rate vol.
            (VolatilityModel::Normal, 0.60),
        ] {
            let build = |option_type: OptionType| {
                IrFutureOption::builder()
                    .id(InstrumentId::new("IRFO-PARITY"))
                    .futures_price(95.50)
                    .strike(95.00)
                    .expiry(expiry)
                    .option_type(option_type)
                    .notional(Money::new(1_000_000.0, Currency::USD))
                    .tick_size(0.0025)
                    .tick_value(6.25)
                    .volatility(vol)
                    .vol_model(model)
                    .discount_curve_id(CurveId::new("USD-OIS"))
                    .build()
                    .expect("build")
            };
            let call = build(OptionType::Call).npv(&market, as_of).expect("call");
            let put = build(OptionType::Put).npv(&market, as_of).expect("put");

            let t = DayCount::Act365F
                .year_fraction(as_of, expiry, DayCountContext::default())
                .expect("t");
            let df = (-flat_rate * t).exp();
            let parity = df * (95.50 - 95.00) * (6.25 / 0.0025);
            assert!(
                (call - put - parity).abs() < 1e-6 * parity.abs().max(1.0),
                "put-call parity violated for {model}: C={call}, P={put}, DF(F-K)PV={parity}"
            );
        }
    }

    /// ATM Bachelier premium has the closed form `σ_N·√(T/2π)` (Bachelier 1900;
    /// Hull Ch. 29): an independent reference for the Normal branch.
    #[test]
    fn normal_model_matches_closed_form_atm_bachelier_premium() {
        use crate::instruments::rates::swaption::VolatilityModel;

        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("flat zero-rate curve"),
        );

        let sigma_n = 0.55_f64; // price points
        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-BACHELIER-ATM"))
            .futures_price(95.50)
            .strike(95.50)
            .expiry(expiry)
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(sigma_n)
            .vol_model(VolatilityModel::Normal)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        let t = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("t");
        let expected = sigma_n * (t / (2.0 * std::f64::consts::PI)).sqrt() * (6.25 / 0.0025);
        let pv = opt.npv(&market, as_of).expect("pv");
        assert!(
            (pv - expected).abs() < 1e-9 * expected,
            "ATM Bachelier closed form: expected {expected}, got {pv}"
        );
    }

    /// Option price must be strictly increasing in volatility for both models.
    #[test]
    fn price_is_monotone_increasing_in_vol_for_both_models() {
        use crate::instruments::rates::swaption::VolatilityModel;

        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let market = MarketContext::new().insert(
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("curve"),
        );

        for (model, vols) in [
            (VolatilityModel::Black, [0.05, 0.10, 0.20, 0.40]),
            (VolatilityModel::Normal, [0.10, 0.30, 0.60, 1.20]),
        ] {
            let mut prev = f64::NEG_INFINITY;
            for vol in vols {
                let pv = IrFutureOption::builder()
                    .id(InstrumentId::new("IRFO-VOL-MONO"))
                    .futures_price(95.50)
                    .strike(95.25)
                    .expiry(expiry)
                    .option_type(OptionType::Call)
                    .notional(Money::new(1_000_000.0, Currency::USD))
                    .tick_size(0.0025)
                    .tick_value(6.25)
                    .volatility(vol)
                    .vol_model(model)
                    .discount_curve_id(CurveId::new("USD-OIS"))
                    .build()
                    .expect("build")
                    .npv(&market, as_of)
                    .expect("pv");
                assert!(
                    pv > prev,
                    "{model} price must increase in vol: pv({vol}) = {pv} <= {prev}"
                );
                prev = pv;
            }
        }
    }

    /// Theta must include the discount-decay term `r·PV` (Hull Ch. 18: Black-76
    /// theta is `−df·F·σ·n(d₁)/(2√T) + r·df·premium`). At zero vol on a
    /// deep-ITM call the vol-decay term vanishes, so daily theta must equal the
    /// finite-difference one-day PV change (premium is frozen intrinsic; only
    /// discounting rolls) — not zero.
    #[test]
    fn theta_includes_discount_decay_matching_one_day_pv_roll() {
        let as_of = date!(2025 - 01 - 15);
        let next_day = date!(2025 - 01 - 16);
        let expiry = date!(2025 - 06 - 16);
        let flat_rate = 0.04_f64;
        let curve =
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (5.0, (-flat_rate * 5.0).exp())])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .build()
                .expect("curve");
        let market = MarketContext::new().insert(curve);

        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-THETA"))
            .futures_price(97.00)
            .strike(92.00)
            .expiry(expiry)
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(0.0)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        use crate::instruments::common_impl::traits::OptionGreeksProvider;
        let theta = opt
            .option_theta(&market, as_of)
            .expect("theta")
            .expect("some");
        let fd = opt.npv(&market, next_day).expect("pv+1d") - opt.npv(&market, as_of).expect("pv");

        assert!(
            fd > 0.0,
            "deep-ITM zero-vol call PV must roll up as discounting shrinks: fd={fd}"
        );
        assert!(
            (theta - fd).abs() < 0.02 * fd,
            "daily theta must match the one-day PV roll (discount decay): theta={theta}, fd={fd}"
        );
    }

    /// Rate DV01 must include the futures-price delta channel: a +1bp parallel
    /// rate move shifts the futures price by −0.01 (price = 100 − rate) and
    /// shrinks the premium discount factor. Verified against full finite
    /// difference (bump futures price −0.01 AND discount curve +1bp).
    #[test]
    fn analytic_dv01_matches_full_finite_difference() {
        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let flat_rate = 0.04_f64;
        let curve = |bump_bp: f64| {
            finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([
                    (0.0, 1.0),
                    (5.0, (-(flat_rate + bump_bp * 1e-4) * 5.0).exp()),
                ])
                .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
                .build()
                .expect("curve")
        };
        let base_market = MarketContext::new().insert(curve(0.0));
        let bumped_market = MarketContext::new().insert(curve(1.0));

        let build = |futures_price: f64| {
            IrFutureOption::builder()
                .id(InstrumentId::new("IRFO-DV01"))
                .futures_price(futures_price)
                .strike(95.25)
                .expiry(expiry)
                .option_type(OptionType::Call)
                .notional(Money::new(1_000_000.0, Currency::USD))
                .tick_size(0.0025)
                .tick_value(6.25)
                .volatility(0.20)
                .discount_curve_id(CurveId::new("USD-OIS"))
                .build()
                .expect("build")
        };

        let opt = build(95.50);
        let dv01 = opt.analytic_rate_dv01(&base_market, as_of).expect("dv01");

        let pv_base = opt.npv(&base_market, as_of).expect("pv");
        let pv_bumped = build(95.50 - 0.01)
            .npv(&bumped_market, as_of)
            .expect("pv bumped");
        let fd = pv_bumped - pv_base;

        assert!(
            dv01 < 0.0,
            "long call on the future is long rates-down; +1bp must cost money: dv01={dv01}"
        );
        assert!(
            (dv01 - fd).abs() < 0.01 * fd.abs(),
            "analytic DV01 must match full FD (delta + discount channels): dv01={dv01}, fd={fd}"
        );
    }
}
