//! Risk factor extraction for VaR calculations.
//!
//! This module defines the types of risk factors that drive VaR calculations
//! and provides utilities to extract them from instruments based on their
//! market data dependencies.

use crate::instruments::common_impl::traits::{CurveDependencies, Instrument};
use crate::metrics::sensitivities::config::STANDARD_BUCKETS_YEARS;
use finstack_core::currency::Currency;
use finstack_core::market_data::context::MarketContext;
use finstack_core::types::CurveId;
use finstack_core::HashSet;
use finstack_core::Result;

/// Risk factor categories for VaR calculation.
///
/// Each risk factor represents a market variable that can shift and impact
/// portfolio valuations. Risk factors are bucketed at standard tenors/strikes
/// to enable historical simulation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RiskFactorType {
    /// Discount curve rate at a specific tenor (in years).
    DiscountRate {
        /// Curve identifier
        curve_id: CurveId,
        /// Tenor in years (e.g., 1.0, 5.0, 10.0)
        tenor_years: f64,
    },

    /// Forward curve rate at a specific tenor (in years).
    ForwardRate {
        /// Curve identifier
        curve_id: CurveId,
        /// Tenor in years
        tenor_years: f64,
    },

    /// Credit spread (hazard rate) at a specific tenor (in years).
    CreditSpread {
        /// Curve identifier
        curve_id: CurveId,
        /// Tenor in years
        tenor_years: f64,
    },

    /// Equity spot price.
    EquitySpot {
        /// Equity ticker or identifier
        ticker: String,
    },

    /// FX spot rate as quote currency per one unit of base currency.
    FxSpot {
        /// Base currency of the FX pair.
        base: Currency,
        /// Quote currency of the FX pair.
        quote: Currency,
    },

    /// Implied volatility at specific expiry and strike.
    ImpliedVol {
        /// Volatility surface identifier
        surface_id: CurveId,
        /// Expiry in years
        expiry_years: f64,
        /// Strike level (absolute or moneyness)
        strike: f64,
    },
}

impl RiskFactorType {
    /// Get a human-readable label for this risk factor.
    pub fn label(&self) -> String {
        match self {
            Self::DiscountRate {
                curve_id,
                tenor_years,
            } => format!("{}::disc::{:.1}y", curve_id.as_str(), tenor_years),
            Self::ForwardRate {
                curve_id,
                tenor_years,
            } => format!("{}::fwd::{:.1}y", curve_id.as_str(), tenor_years),
            Self::CreditSpread {
                curve_id,
                tenor_years,
            } => format!("{}::credit::{:.1}y", curve_id.as_str(), tenor_years),
            Self::EquitySpot { ticker } => format!("{}::spot", ticker),
            Self::FxSpot { base, quote } => format!("{}{}::fx_spot", base, quote),
            Self::ImpliedVol {
                surface_id,
                expiry_years,
                strike,
            } => format!(
                "{}::vol::{:.1}y::{}",
                surface_id.as_str(),
                expiry_years,
                strike
            ),
        }
    }
}

/// Extract risk factors from an instrument's market data dependencies.
///
/// This function inspects the instrument's curve dependencies and extracts
/// risk factors at standard tenor buckets (see `STANDARD_BUCKETS_YEARS`).
/// The risk factors can then be used
/// to apply historical market shifts for VaR calculation.
///
/// # Arguments
///
/// * `instrument` - The instrument to analyze
/// * `market` - Current market context (used to verify curve existence)
///
/// # Returns
///
/// Vector of risk factors that affect this instrument's valuation
///
/// # Examples
///
/// ```rust,no_run
/// use finstack_valuations::instruments::Bond;
/// use finstack_valuations::metrics::risk::extract_risk_factors;
/// use finstack_core::market_data::context::MarketContext;
///
/// # fn main() -> finstack_core::Result<()> {
/// let bond = Bond::example().unwrap();
/// let market = MarketContext::new();
/// let factors = extract_risk_factors(&bond, &market)?;
/// // factors will include discount curve rates at standard tenors
/// # let _ = factors;
/// # Ok(())
/// # }
/// ```
pub fn extract_risk_factors<I>(
    instrument: &I,
    market: &MarketContext,
) -> Result<Vec<RiskFactorType>>
where
    I: Instrument + CurveDependencies,
{
    let mut factors = Vec::new();
    let mut seen = HashSet::default();

    // Get instrument's curve dependencies
    let deps = instrument.curve_dependencies()?;

    // Standard tenors for IR/credit curve factors
    extract_curve_factors(
        &mut factors,
        &mut seen,
        &deps.discount_curves,
        market,
        &STANDARD_BUCKETS_YEARS,
        |m, id| m.get_discount(id).is_ok(),
        |curve_id, tenor_years| RiskFactorType::DiscountRate {
            curve_id: curve_id.clone(),
            tenor_years,
        },
    );

    extract_curve_factors(
        &mut factors,
        &mut seen,
        &deps.forward_curves,
        market,
        &STANDARD_BUCKETS_YEARS,
        |m, id| m.get_forward(id).is_ok(),
        |curve_id, tenor_years| RiskFactorType::ForwardRate {
            curve_id: curve_id.clone(),
            tenor_years,
        },
    );

    extract_curve_factors(
        &mut factors,
        &mut seen,
        &deps.credit_curves,
        market,
        &STANDARD_BUCKETS_YEARS,
        |m, id| m.get_hazard(id).is_ok(),
        |curve_id, tenor_years| RiskFactorType::CreditSpread {
            curve_id: curve_id.clone(),
            tenor_years,
        },
    );

    extract_instrument_specific_risk_factors(instrument, market, &mut factors, &mut seen)?;

    Ok(factors)
}

fn push_factor(
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
    factor: RiskFactorType,
) {
    let label = factor.label();
    if seen.insert(label) {
        factors.push(factor);
    }
}

fn extract_curve_factors<FExists, FMk>(
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
    curve_ids: &[CurveId],
    market: &MarketContext,
    tenors: &[f64],
    exists: FExists,
    mk_factor: FMk,
) where
    FExists: Fn(&MarketContext, &str) -> bool,
    FMk: Fn(&CurveId, f64) -> RiskFactorType,
{
    for curve_id in curve_ids {
        if exists(market, curve_id.as_str()) {
            for &tenor_years in tenors {
                push_factor(factors, seen, mk_factor(curve_id, tenor_years));
            }
        }
    }
}

trait InstrumentRiskFactorProvider {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()>;
}

impl InstrumentRiskFactorProvider for crate::instruments::equity::Equity {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        for price_id in self.price_id_candidates() {
            if market.get_price(&price_id).is_ok() {
                push_factor(
                    factors,
                    seen,
                    RiskFactorType::EquitySpot { ticker: price_id },
                );
                break;
            }
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider
    for crate::instruments::fixed_income::convertible::ConvertibleBond
{
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if let Some(ticker) = self.underlying_equity_id.as_ref() {
            if market.get_price(ticker).is_ok() {
                push_factor(
                    factors,
                    seen,
                    RiskFactorType::EquitySpot {
                        ticker: ticker.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::equity::equity_option::EquityOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if market.get_price(&self.spot_id).is_ok() {
            push_factor(
                factors,
                seen,
                RiskFactorType::EquitySpot {
                    ticker: self.spot_id.to_string(),
                },
            );
        }

        append_vol_surface_factor(
            market,
            factors,
            seen,
            &self.vol_surface_id,
            0.0,
            self.strike,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::fx::fx_option::FxOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        append_fx_spot_factor(
            market,
            factors,
            seen,
            self.base_currency,
            self.quote_currency,
        );
        append_vol_surface_factor(
            market,
            factors,
            seen,
            &self.vol_surface_id,
            0.0,
            self.strike,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxSpot {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        append_fx_spot_factor(
            market,
            factors,
            seen,
            self.base_currency,
            self.quote_currency,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxForward {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.spot_rate_override.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.quote_currency,
            );
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxSwap {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.near_rate.is_none() || self.far_rate.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.quote_currency,
            );
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::Ndf {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.spot_rate_override.is_none() || self.forward_rate_override.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.settlement_currency,
            );
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxBarrierOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.fx_spot_id.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.quote_currency,
            );
        }
        append_vol_surface_factor(
            market,
            factors,
            seen,
            &self.vol_surface_id,
            0.0,
            self.strike,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxDigitalOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        append_fx_spot_factor(
            market,
            factors,
            seen,
            self.base_currency,
            self.quote_currency,
        );
        append_vol_surface_factor(
            market,
            factors,
            seen,
            &self.vol_surface_id,
            0.0,
            self.strike,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxTouchOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        append_fx_spot_factor(
            market,
            factors,
            seen,
            self.base_currency,
            self.quote_currency,
        );
        append_vol_surface_factor(
            market,
            factors,
            seen,
            &self.vol_surface_id,
            0.0,
            self.barrier_level,
        );
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::FxVarianceSwap {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.spot_id.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.quote_currency,
            );
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::QuantoOption {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        if self.payoff_fx_rate.is_none() {
            append_fx_spot_factor(
                market,
                factors,
                seen,
                self.base_currency,
                self.quote_currency,
            );
        }
        Ok(())
    }
}

impl InstrumentRiskFactorProvider for crate::instruments::XccySwap {
    fn append_risk_factors(
        &self,
        market: &MarketContext,
        factors: &mut Vec<RiskFactorType>,
        seen: &mut HashSet<String>,
    ) -> Result<()> {
        append_fx_spot_factor(
            market,
            factors,
            seen,
            self.leg1.currency,
            self.leg2.currency,
        );
        Ok(())
    }
}

fn append_fx_spot_factor(
    market: &MarketContext,
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
    base: Currency,
    quote: Currency,
) {
    if base != quote && market.fx().is_some() {
        push_factor(factors, seen, RiskFactorType::FxSpot { base, quote });
    }
}

fn append_vol_surface_factor(
    market: &MarketContext,
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
    surface_id: &CurveId,
    expiry_years: f64,
    strike: f64,
) {
    if market.get_surface(surface_id.as_str()).is_ok() {
        push_factor(
            factors,
            seen,
            RiskFactorType::ImpliedVol {
                surface_id: surface_id.clone(),
                expiry_years,
                strike,
            },
        );
    }
}

fn append_if_instrument<T>(
    instrument: &dyn Instrument,
    market: &MarketContext,
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
) -> Result<()>
where
    T: InstrumentRiskFactorProvider + 'static,
{
    if let Some(typed) = instrument.as_any().downcast_ref::<T>() {
        typed.append_risk_factors(market, factors, seen)?;
    }
    Ok(())
}

fn extract_instrument_specific_risk_factors<I>(
    instrument: &I,
    market: &MarketContext,
    factors: &mut Vec<RiskFactorType>,
    seen: &mut HashSet<String>,
) -> Result<()>
where
    I: Instrument + CurveDependencies,
{
    let instrument = instrument as &dyn Instrument;

    append_if_instrument::<crate::instruments::equity::Equity>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::fixed_income::convertible::ConvertibleBond>(
        instrument, market, factors, seen,
    )?;
    append_if_instrument::<crate::instruments::equity::equity_option::EquityOption>(
        instrument, market, factors, seen,
    )?;
    append_if_instrument::<crate::instruments::fx::fx_option::FxOption>(
        instrument, market, factors, seen,
    )?;
    append_if_instrument::<crate::instruments::FxSpot>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxForward>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxSwap>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::Ndf>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxBarrierOption>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxDigitalOption>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxTouchOption>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::FxVarianceSwap>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::QuantoOption>(instrument, market, factors, seen)?;
    append_if_instrument::<crate::instruments::XccySwap>(instrument, market, factors, seen)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::Money;
    use time::macros::date;

    #[test]
    fn test_risk_factor_label() {
        let factor = RiskFactorType::DiscountRate {
            curve_id: CurveId::from("USD-OIS"),
            tenor_years: 5.0,
        };
        assert_eq!(factor.label(), "USD-OIS::disc::5.0y");

        let factor = RiskFactorType::CreditSpread {
            curve_id: CurveId::from("AAPL"),
            tenor_years: 10.0,
        };
        assert_eq!(factor.label(), "AAPL::credit::10.0y");

        let factor = RiskFactorType::ForwardRate {
            curve_id: CurveId::from("USD-LIBOR-3M"),
            tenor_years: 2.0,
        };
        assert_eq!(factor.label(), "USD-LIBOR-3M::fwd::2.0y");

        let factor = RiskFactorType::FxSpot {
            base: Currency::EUR,
            quote: Currency::USD,
        };
        assert_eq!(factor.label(), "EURUSD::fx_spot");
    }

    #[test]
    fn test_extract_discount_factors_from_bond() -> Result<()> {
        use crate::instruments::Bond;

        let as_of = date!(2024 - 01 - 01);

        // Use Bond::fixed factory method
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(100_000.0, Currency::USD),
            0.05,
            as_of,
            date!(2029 - 01 - 01),
            "USD-OIS",
        )?;

        // Create market with discount curve
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (1.0, 0.96), (5.0, 0.85), (10.0, 0.70)])
            .build()?;

        let market = MarketContext::new().insert(curve);

        // Extract risk factors
        let factors = extract_risk_factors(&bond, &market)?;

        // Should have discount rate factors at standard tenors
        assert!(!factors.is_empty(), "Should extract risk factors");

        // Verify we have discount rate factors
        let discount_factors: Vec<_> = factors
            .iter()
            .filter_map(|f| match f {
                RiskFactorType::DiscountRate {
                    curve_id,
                    tenor_years,
                } => Some((curve_id.as_str(), *tenor_years)),
                _ => None,
            })
            .collect();

        assert!(
            !discount_factors.is_empty(),
            "Should have discount rate factors"
        );
        assert!(
            discount_factors.iter().all(|(cid, _)| *cid == "USD-OIS"),
            "All factors should be for USD-OIS curve"
        );

        // Verify we're using standard tenors
        for (_, tenor) in &discount_factors {
            assert!(
                STANDARD_BUCKETS_YEARS.contains(tenor),
                "Tenor {} should be in standard bucket list",
                tenor
            );
        }

        Ok(())
    }

    #[test]
    fn test_extract_factors_empty_market() -> Result<()> {
        use crate::instruments::Bond;

        let as_of = date!(2024 - 01 - 01);
        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(100_000.0, Currency::USD),
            0.05,
            as_of,
            date!(2029 - 01 - 01),
            "USD-OIS",
        )?;

        // Empty market - curve exists in instrument but not in market
        let market = MarketContext::new();

        let factors = extract_risk_factors(&bond, &market)?;

        // Should return empty vector when curves don't exist
        assert!(
            factors.is_empty(),
            "Should have no factors for empty market"
        );

        Ok(())
    }

    #[test]
    fn test_extract_equity_and_vol_factors() -> Result<()> {
        use crate::instruments::equity::equity_option::EquityOption;

        let expiry = date!(2025 - 06 - 01);
        let option = EquityOption::builder()
            .id(finstack_core::types::InstrumentId::new("EQO"))
            .underlying_ticker("AAPL".to_string())
            .strike(100.0)
            .option_type(crate::instruments::OptionType::Call)
            .exercise_style(crate::instruments::ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(crate::instruments::SettlementType::Cash)
            .discount_curve_id(finstack_core::types::CurveId::new("USD-OIS"))
            .spot_id("EQUITY-SPOT".into())
            .vol_surface_id(finstack_core::types::CurveId::new("EQUITY-VOL"))
            .div_yield_id_opt(Some(finstack_core::types::CurveId::new("EQUITY-DIVYIELD")))
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(crate::instruments::Attributes::new())
            .build()?;

        let base_date = date!(2024 - 01 - 01);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (1.0, 0.98)])
            .build()?;

        let market = MarketContext::new()
            .insert(curve)
            .insert_price(&option.spot_id, MarketScalar::Unitless(150.0))
            .insert_surface(
                finstack_core::market_data::surfaces::VolSurface::builder(
                    option.vol_surface_id.clone(),
                )
                .expiries(&[0.5, 1.0])
                .strikes(&[90.0, 100.0])
                .row(&[0.24, 0.25])
                .row(&[0.26, 0.27])
                .build()?,
            );

        let factors = extract_risk_factors(&option, &market)?;

        assert!(
            factors.iter().any(
                |f| matches!(f, RiskFactorType::EquitySpot { ticker } if ticker == option.spot_id.as_str())
            ),
            "should include equity spot factor"
        );
        assert!(
            factors.iter().any(|f| matches!(f, RiskFactorType::ImpliedVol { surface_id, .. } if surface_id == &option.vol_surface_id)),
            "should include vol surface factor"
        );

        Ok(())
    }

    #[test]
    fn test_extract_vol_factors_from_fx_option() -> Result<()> {
        use crate::instruments::fx::fx_option::FxOption;
        use crate::instruments::{
            Attributes, ExerciseStyle, OptionType, PricingOverrides, SettlementType,
        };

        let as_of = date!(2024 - 01 - 01);
        let option = FxOption::builder()
            .id(finstack_core::types::InstrumentId::new("FXO"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .strike(1.10)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(date!(2025 - 01 - 01))
            .day_count(DayCount::Act365F)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .settlement(SettlementType::Cash)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()?;

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (1.0, 0.98)])
            .build()?;
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots(vec![(0.0, 1.0), (1.0, 0.99)])
            .build()?;
        let market = MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_surface(
                finstack_core::market_data::surfaces::VolSurface::builder("EURUSD-VOL")
                    .expiries(&[1.0])
                    .strikes(&[1.10])
                    .row(&[0.12])
                    .build()?,
            );

        let factors = extract_risk_factors(&option, &market)?;

        assert!(
            factors.iter().any(|f| matches!(f, RiskFactorType::ImpliedVol { surface_id, .. } if surface_id == &option.vol_surface_id)),
            "FX option should include its volatility surface factor"
        );
        Ok(())
    }
}
