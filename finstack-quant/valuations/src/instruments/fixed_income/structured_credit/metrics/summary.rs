//! Per-tranche risk/spread metrics bundle for structured-credit notes.
//!
//! The deal-level metric registry (z-spread, CS01, duration, …) operates on the
//! deal's *aggregate* cashflows, which is not meaningful for a multi-tranche
//! structure (mixing senior and equity flows into one stream). This module
//! assembles the same metrics **per tranche**, from that tranche's own projected
//! cashflows, reusing the standalone calculators the registry wraps.

use crate::constants::ONE_BASIS_POINT;
use crate::instruments::fixed_income::structured_credit::metrics::{
    calculate_tranche_convexity, calculate_tranche_cs01, calculate_tranche_duration,
    calculate_tranche_wal, calculate_tranche_z_spread,
};
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::instruments::Instrument;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};

/// Per-tranche risk and spread metrics, all computed from one tranche's own
/// projected cashflows — so they are meaningful per note, unlike the deal-level
/// aggregates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrancheMetrics {
    /// Identifier of the tranche.
    pub tranche_id: String,
    /// Present value of the tranche (currency units).
    pub pv: f64,
    /// Model price as a percentage of original balance.
    pub price_pct: f64,
    /// Weighted-average life (years).
    pub wal: f64,
    /// Z-spread to `target_price_pct` (basis points): the constant spread over the
    /// discount curve equating the tranche's PV to that price. Zero when solved
    /// against the tranche's own model price (no spread to its curve-discounted value).
    pub z_spread_bp: f64,
    /// Credit-spread DV01 — currency change for a +1 bp z-spread shock. Negative
    /// for a long tranche (wider spreads reduce PV).
    pub cs01: f64,
    /// Spread duration (years): `-CS01 / (PV · 1bp)`.
    pub spread_duration: f64,
    /// Modified (rate) duration of the projected cashflows (years).
    pub modified_duration: f64,
    /// Modified convexity of the projected cashflows (years²).
    pub convexity: f64,
    /// Price the z-spread/CS01 were solved against (% of original balance) —
    /// the supplied market price, or the model price when none was given.
    pub target_price_pct: f64,
}

/// Compute the per-tranche metrics bundle ([`TrancheMetrics`]).
///
/// All figures derive from the named tranche's own waterfall cashflows: PV and
/// price, WAL, the credit z-spread and CS01, spread duration, modified duration
/// and convexity. This is the meaningful per-note alternative to the deal-level
/// metric registry, which aggregates every tranche's flows into one stream.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal owning the requested tranche,
///   its waterfall, and the curve identifiers used for projection.
/// * `tranche_id` - Identifier of the tranche to summarize from its own
///   cashflows rather than a deal-level aggregate.
/// * `market` - Market context supplying the deal's discount curve and any
///   rate/index data needed for cashflow projection.
/// * `as_of` - Valuation date used to determine projected cashflows and their
///   discounting horizon.
/// * `market_price_pct` - quoted price (% of original balance) the z-spread and
///   CS01 are solved against. When `None`, the tranche's own model price is used,
///   giving a zero z-spread (a useful round-trip check) while CS01, duration and
///   convexity remain meaningful sensitivities.
///
/// # Errors
///
/// Returns an error if the tranche is missing, the discount curve is
/// unavailable, or the cashflows cannot be projected / the z-spread solved.
pub fn calculate_tranche_metrics(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: Date,
    market_price_pct: Option<f64>,
) -> Result<TrancheMetrics> {
    deal.validate_for_pricing()?;
    let tranche = deal
        .tranches
        .tranches
        .iter()
        .find(|t| t.id.as_str() == tranche_id)
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: format!("tranche:{tranche_id}"),
            })
        })?;
    let original_balance = tranche.original_balance.amount();

    let cashflows = deal.get_tranche_cashflows(tranche_id, market, as_of)?;
    let disc = market.get_discount(deal.discount_curve_id.as_str())?;
    let curve = disc.as_ref();

    // PV from the already-projected cashflows. (Calling `value_tranche` here would
    // re-run the full waterfall simulation a second time — the projection is the
    // expensive step — so discount the flows we already have instead.)
    let mut pv = 0.0_f64;
    for (date, amount) in &cashflows.cashflows {
        if *date > as_of {
            pv += amount.amount() * curve.df_between_dates(as_of, *date)?;
        }
    }
    let pv_money = Money::new(pv, deal.pool.base_currency());
    let price_pct = if original_balance > 0.0 {
        pv / original_balance * 100.0
    } else {
        0.0
    };

    let wal = calculate_tranche_wal(&cashflows, as_of)?;
    let modified_duration =
        calculate_tranche_duration(&cashflows.cashflows, curve, as_of, pv_money)?;
    let convexity = calculate_tranche_convexity(&cashflows.cashflows, curve, as_of)?;

    // Z-spread (and the CS01 measured at it) are solved against the supplied
    // market price, or the tranche's own model price when none is given.
    let target_price_pct = market_price_pct.unwrap_or(price_pct);
    let target_pv = Money::new(
        target_price_pct / 100.0 * original_balance,
        pv_money.currency(),
    );
    let z_spread_bp = calculate_tranche_z_spread(&cashflows.cashflows, curve, target_pv, as_of)?;
    let cs01 = calculate_tranche_cs01(&cashflows.cashflows, curve, z_spread_bp * 1e-4, as_of)?;
    let spread_duration = if pv != 0.0 {
        -cs01 / (pv * ONE_BASIS_POINT)
    } else {
        0.0
    };

    Ok(TrancheMetrics {
        tranche_id: tranche_id.to_string(),
        pv,
        price_pct,
        wal,
        z_spread_bp,
        cs01,
        spread_duration,
        modified_duration,
        convexity,
        target_price_pct,
    })
}
