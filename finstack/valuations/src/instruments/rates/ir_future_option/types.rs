//! IR Future Option types and implementation.
//!
//! Exchange-traded options on interest rate futures (e.g., SOFR futures options).
//! Priced using Black-76 on the futures price (100 - rate).
//!
//! # Pricing
//!
//! Forward = futures_price (no convexity adjustment needed since the option
//! is on the future itself). Premium is discounted from expiry to today.
//!
//! ```text
//! Call = DF × [F·N(d₁) - K·N(d₂)]
//! Put  = DF × [K·N(-d₂) - F·N(-d₁)]
//! ```
//!
//! where d₁ = [ln(F/K) + σ²T/2] / (σ√T), d₂ = d₁ - σ√T.
//!
//! # Market Conventions
//!
//! - **SOFR options**: quoted in price points (e.g., 0.25 = 25 ticks)
//! - **Tick sizes**: 0.0025 for 1M SOFR ($6.25), 0.0025 for 3M SOFR ($6.25)
//! - **Exercise**: American-style on CME, but priced as European (early exercise
//!   is rarely optimal for futures options)
//!
//! # References
//!
//! - Black, F. (1976). "The pricing of commodity contracts."
//!   *Journal of Financial Economics*, 3(1-2), 167-179.

use crate::cashflow::traits::CashflowProvider;
use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::models::volatility::black::{d1_black76, d1_d2_black76};
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Attributes;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::{norm_cdf, norm_pdf};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};

/// Exchange-traded option on an interest rate future (e.g., SOFR futures).
///
/// Priced using Black-76 on the futures price. The underlying is the futures
/// price itself (100 - rate), so no convexity adjustment is needed.
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
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
    /// Lognormal (Black) volatility, annualized
    pub volatility: f64,
    /// Discount curve ID for PV calculation
    pub discount_curve_id: CurveId,
    /// Pricing overrides
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    #[serde(default)]
    #[builder(default)]
    pub attributes: Attributes,
}

impl IrFutureOption {
    /// Time to expiry in years from `as_of`, using Act/365F.
    ///
    /// Returns `Ok(0.0)` for expired options (`as_of >= expiry`).
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure rather than swallowing it as `0.0`: a
    /// spurious zero would silently mis-classify a live option as expired and
    /// drop all of its time value with no diagnostic.
    fn time_to_expiry(&self, as_of: Date) -> finstack_core::Result<f64> {
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
    fn contract_point_value(&self) -> finstack_core::Result<f64> {
        if !self.tick_size.is_finite() || self.tick_size <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "IR future option tick_size must be positive and finite; got {}",
                self.tick_size
            )));
        }
        if !self.tick_value.is_finite() || self.tick_value <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "IR future option tick_value must be positive and finite; got {}",
                self.tick_value
            )));
        }
        Ok(self.tick_value / self.tick_size)
    }

    /// Black-76 option premium (undiscounted) and the discount factor.
    ///
    /// Returns `(undiscounted_premium, discount_factor, time_to_expiry)`.
    fn black76_components(
        &self,
        context: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<(f64, f64, f64)> {
        let t = self.time_to_expiry(as_of)?;
        let disc = context.get_discount(&self.discount_curve_id)?;
        let df = relative_df_discount_curve(disc.as_ref(), as_of, self.expiry)?;

        if t <= 0.0 || self.volatility <= 0.0 || !self.volatility.is_finite() {
            return Ok((self.intrinsic_value(), df, t));
        }

        if self.futures_price <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
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

        let premium = if self.is_call() {
            self.futures_price * norm_cdf(d1) - self.strike * norm_cdf(d2)
        } else {
            self.strike * norm_cdf(-d2) - self.futures_price * norm_cdf(-d1)
        };

        Ok((premium, df, t))
    }

    /// Present value of the option.
    pub fn npv(&self, context: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        let (premium, df, _t) = self.black76_components(context, as_of)?;
        let pv = df * premium * self.contract_point_value()?;
        if !pv.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "IR future option produced non-finite PV: F={}, K={}, σ={}, id={}",
                self.futures_price, self.strike, self.volatility, self.id,
            )));
        }
        Ok(pv)
    }

    /// Forward delta (sensitivity to futures price).
    ///
    /// Call: N(d₁), Put: N(d₁) - 1
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from [`time_to_expiry`](Self::time_to_expiry).
    pub fn delta(&self, as_of: Date) -> finstack_core::Result<f64> {
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
        let d1 = d1_black76(self.futures_price, self.strike, self.volatility, t);
        Ok(if self.is_call() {
            norm_cdf(d1)
        } else {
            norm_cdf(d1) - 1.0
        })
    }

    /// Gamma (second derivative w.r.t. futures price).
    ///
    /// Gamma = n(d₁) / (F × σ × √T)
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from [`time_to_expiry`](Self::time_to_expiry).
    pub fn gamma(&self, as_of: Date) -> finstack_core::Result<f64> {
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.volatility <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d1 = d1_black76(self.futures_price, self.strike, self.volatility, t);
        let denom = (self.futures_price * self.volatility * t.sqrt()).max(1e-12);
        Ok(norm_pdf(d1) / denom)
    }

    /// Vega per 1% absolute change in volatility.
    ///
    /// Vega = F × √T × n(d₁) / 100
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from [`time_to_expiry`](Self::time_to_expiry).
    pub fn vega_per_pct(&self, as_of: Date) -> finstack_core::Result<f64> {
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d1 = if self.volatility > 0.0 {
            d1_black76(self.futures_price, self.strike, self.volatility, t)
        } else {
            0.0
        };
        Ok(self.futures_price * t.sqrt() * norm_pdf(d1) / 100.0)
    }

    /// Theta (time decay per calendar day, undiscounted).
    ///
    /// Call theta = -F·σ·n(d₁) / (2√T) per year, divided by 365.25 for daily.
    ///
    /// # Errors
    ///
    /// Propagates a day-count failure from [`time_to_expiry`](Self::time_to_expiry).
    pub fn theta_daily(&self, as_of: Date) -> finstack_core::Result<f64> {
        let t = self.time_to_expiry(as_of)?;
        if t <= 0.0 || self.volatility <= 0.0 || self.futures_price <= 0.0 {
            return Ok(0.0);
        }
        let d1 = d1_black76(self.futures_price, self.strike, self.volatility, t);
        let annual_theta = -self.futures_price * self.volatility * norm_pdf(d1) / (2.0 * t.sqrt());
        Ok(annual_theta / 365.25)
    }

    /// Create a canonical example 3M SOFR futures option.
    pub fn example() -> finstack_core::Result<Self> {
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

    fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
        MarketDependencies::from_curve_dependencies(self)
    }

    fn base_value(&self, curves: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
        let pv = self.npv(curves, as_of)?;
        Ok(Money::new(pv, self.notional.currency()))
    }

    fn value_raw(&self, curves: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        self.npv(curves, as_of)
    }

    fn expiry(&self) -> Option<Date> {
        Some(self.expiry)
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}

impl CashflowProvider for IrFutureOption {
    fn notional(&self) -> Option<Money> {
        Some(self.notional)
    }

    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            Vec::new(),
            DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: self.notional(),
                representation: crate::cashflow::builder::CashflowRepresentation::Placeholder,
                ..Default::default()
            },
        ))
    }
}

impl crate::instruments::common_impl::traits::OptionDeltaProvider for IrFutureOption {
    fn option_delta(&self, _market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        Ok(self.delta(as_of)? * self.contract_point_value()?)
    }
}

impl crate::instruments::common_impl::traits::OptionGammaProvider for IrFutureOption {
    fn option_gamma(&self, _market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        Ok(self.gamma(as_of)? * self.contract_point_value()?)
    }
}

impl crate::instruments::common_impl::traits::OptionVegaProvider for IrFutureOption {
    fn option_vega(&self, _market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        Ok(self.vega_per_pct(as_of)? * self.contract_point_value()?)
    }
}

impl crate::instruments::common_impl::traits::OptionThetaProvider for IrFutureOption {
    fn option_theta(&self, _market: &MarketContext, as_of: Date) -> finstack_core::Result<f64> {
        Ok(self.theta_daily(as_of)? * self.contract_point_value()?)
    }
}

impl crate::instruments::common_impl::traits::OptionGreeksProvider for IrFutureOption {
    fn option_greeks(
        &self,
        market: &MarketContext,
        as_of: Date,
        request: &crate::instruments::common_impl::traits::OptionGreeksRequest,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::OptionGreeks> {
        use crate::instruments::common_impl::traits::{
            OptionDeltaProvider, OptionGammaProvider, OptionGreekKind, OptionGreeks,
            OptionThetaProvider, OptionVegaProvider,
        };

        match request.greek {
            OptionGreekKind::Delta => Ok(OptionGreeks {
                delta: Some(OptionDeltaProvider::option_delta(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Gamma => Ok(OptionGreeks {
                gamma: Some(OptionGammaProvider::option_gamma(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Vega => Ok(OptionGreeks {
                vega: Some(OptionVegaProvider::option_vega(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            OptionGreekKind::Theta => Ok(OptionGreeks {
                theta: Some(OptionThetaProvider::option_theta(self, market, as_of)?),
                ..OptionGreeks::default()
            }),
            _ => Ok(OptionGreeks::default()),
        }
    }
}

impl crate::instruments::common_impl::traits::CurveDependencies for IrFutureOption {
    fn curve_dependencies(
        &self,
    ) -> finstack_core::Result<crate::instruments::common_impl::traits::InstrumentCurves> {
        crate::instruments::common_impl::traits::InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
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
    fn expired_option_returns_intrinsic_delta() {
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
        assert_eq!(delta, 1.0, "Expired ITM call delta should be 1.0");
    }

    #[test]
    fn npv_uses_contract_tick_economics_not_notional_amount() {
        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 06 - 16);
        let market = MarketContext::new().insert(
            finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .build()
                .expect("flat zero-rate curve"),
        );

        let opt = IrFutureOption::builder()
            .id(InstrumentId::new("IRFO-CONTRACT-SCALE"))
            .futures_price(96.0)
            .strike(95.0)
            .expiry(expiry)
            .option_type(OptionType::Call)
            .notional(Money::new(1_000_000.0, Currency::USD))
            .tick_size(0.0025)
            .tick_value(6.25)
            .volatility(0.0)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .expect("build");

        let pv = opt.npv(&market, as_of).expect("pv");
        let expected = (96.0 - 95.0) * (6.25 / 0.0025);
        assert!(
            (pv - expected).abs() < 1e-9,
            "PV must be scaled by tick economics: expected {expected}, got {pv}"
        );
    }
}
