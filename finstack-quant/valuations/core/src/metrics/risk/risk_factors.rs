//! Risk factor extraction for VaR calculations.
//!
//! This module defines the types of risk factors that drive VaR calculations
//! and provides utilities to extract them from instruments based on their
//! market data dependencies.

use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::STANDARD_BUCKETS_YEARS;
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
        vol_surface_id: CurveId,
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
            } => format!("{}::disc::{}y", curve_id.as_str(), tenor_years),
            Self::ForwardRate {
                curve_id,
                tenor_years,
            } => format!("{}::fwd::{}y", curve_id.as_str(), tenor_years),
            Self::CreditSpread {
                curve_id,
                tenor_years,
            } => format!("{}::credit::{}y", curve_id.as_str(), tenor_years),
            Self::EquitySpot { ticker } => format!("{}::spot", ticker),
            Self::FxSpot { base, quote } => format!("{}{}::fx_spot", base, quote),
            Self::ImpliedVol {
                vol_surface_id,
                expiry_years,
                strike,
            } => format!(
                "{}::vol::{}y::{}",
                vol_surface_id.as_str(),
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
/// Volatility extraction includes every stored surface node, never an interpolated
/// or synthetic point. Historical replay requires those exact grid coordinates.
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
/// For a fixed-rate bond the factors are its discount curve's rates at the
/// standard tenors; the `core_framework` integration tests
/// (`tests/core/risk_factors.rs`) show the extraction over the built-in
/// instruments.
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

    for spot_id in &dependencies.market_scalar_ids {
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
    // Every actual node is a distinct replayable factor, including nodes
    // whose expiries differ below display-label precision.
    for dependency in &dependencies.volatility_dependencies {
        if let Ok(surface) = market.get_surface(dependency.vol_surface_id.as_str()) {
            for &expiry_years in surface.expiries() {
                for &strike in surface.strikes() {
                    push_factor(
                        &mut factors,
                        &mut seen,
                        RiskFactorType::ImpliedVol {
                            vol_surface_id: dependency.vol_surface_id.clone(),
                            expiry_years,
                            strike,
                        },
                    );
                }
            }
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
