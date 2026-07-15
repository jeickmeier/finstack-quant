//! Risk factor extraction for VaR calculations.
//!
//! This module defines the types of risk factors that drive VaR calculations
//! and provides utilities to extract them from instruments based on their
//! market data dependencies.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::config::STANDARD_BUCKETS_YEARS;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashSet;
use finstack_quant_core::Result;

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
/// ```ignore
/// use finstack_quant_valuations::instruments::Bond;
/// use finstack_quant_valuations::metrics::risk::extract_risk_factors;
/// use finstack_quant_core::market_data::context::MarketContext;
///
/// # fn main() -> finstack_quant_core::Result<()> {
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
    I: Instrument,
{
    let mut factors = Vec::new();
    let mut seen = HashSet::default();

    let dependencies = instrument.market_dependencies()?;
    let curves = &dependencies.curves;

    // Standard tenors for IR/credit curve factors
    extract_curve_factors(
        &mut factors,
        &mut seen,
        &curves.discount_curves,
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
        &curves.forward_curves,
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
        &curves.credit_curves,
        market,
        &STANDARD_BUCKETS_YEARS,
        |m, id| m.get_hazard(id).is_ok(),
        |curve_id, tenor_years| RiskFactorType::CreditSpread {
            curve_id: curve_id.clone(),
            tenor_years,
        },
    );

    for spot_id in &dependencies.spot_ids {
        if market.get_price(spot_id).is_ok() {
            push_factor(
                &mut factors,
                &mut seen,
                RiskFactorType::EquitySpot {
                    ticker: spot_id.clone(),
                },
            );
        }
    }
    for pair in &dependencies.fx_pairs {
        if pair.base != pair.quote && market.fx().is_some() {
            push_factor(
                &mut factors,
                &mut seen,
                RiskFactorType::FxSpot {
                    base: pair.base,
                    quote: pair.quote,
                },
            );
        }
    }
    for dependency in &dependencies.volatility_dependencies {
        if market.get_surface(dependency.surface_id.as_str()).is_ok() {
            push_factor(
                &mut factors,
                &mut seen,
                RiskFactorType::ImpliedVol {
                    surface_id: dependency.surface_id.clone(),
                    expiry_years: 0.0,
                    strike: dependency.reference_strike.unwrap_or(0.0),
                },
            );
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
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
            .id(finstack_quant_core::types::InstrumentId::new("EQO"))
            .underlying_ticker("AAPL".to_string())
            .strike(100.0)
            .option_type(crate::instruments::OptionType::Call)
            .exercise_style(crate::instruments::ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(crate::instruments::SettlementType::Cash)
            .discount_curve_id(finstack_quant_core::types::CurveId::new("USD-OIS"))
            .spot_id("EQUITY-SPOT".into())
            .vol_surface_id(finstack_quant_core::types::CurveId::new("EQUITY-VOL"))
            .div_yield_id_opt(Some(finstack_quant_core::types::CurveId::new(
                "EQUITY-DIVYIELD",
            )))
            .instrument_pricing_overrides(crate::instruments::InstrumentPricingOverrides::default())
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
                finstack_quant_core::market_data::surfaces::VolSurface::builder(
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
            Attributes, ExerciseStyle, InstrumentPricingOverrides, OptionType, SettlementType,
        };

        let as_of = date!(2024 - 01 - 01);
        let option = FxOption::builder()
            .id(finstack_quant_core::types::InstrumentId::new("FXO"))
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
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
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
                finstack_quant_core::market_data::surfaces::VolSurface::builder("EURUSD-VOL")
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
