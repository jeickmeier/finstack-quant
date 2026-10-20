//! Per-tranche risk/spread metrics bundle for structured-credit notes.
//!
//! The deal-level metric registry (z-spread, CS01, duration, …) operates on the
//! deal's *aggregate* cashflows, which is not meaningful for a multi-tranche
//! structure (mixing senior and equity flows into one stream). This module
//! assembles the same metrics **per tranche**, from that tranche's own projected
//! cashflows, reusing the standalone calculators the registry wraps.

use crate::structured_credit::metrics::{
    calculate_tranche_cs01, calculate_tranche_spread_convexity, calculate_tranche_wal,
    calculate_tranche_z_spread,
};
use crate::structured_credit::{
    CallAssumption, CallScope, StructuredCredit, TrancheAccrualPeriod, TrancheCashflows,
    TrancheSeniority,
};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::math::units::ONE_BASIS_POINT;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::helpers::ValidatedPricingLifecycle;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-tranche valuation, yield, risk and spread metrics, all computed from
/// one tranche's own projected cashflows — so they are meaningful per note,
/// unlike the deal-level aggregates.
///
/// Prices are quoted per the tranche's CURRENT balance (the factor-adjusted
/// secondary-market basis; `factor` converts to original face). An impaired
/// note — no current balance, or no positive settlement value in the
/// projection — reports zero prices, spreads and sensitivities and no yield
/// instead of failing the solves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheMetrics {
    /// Identifier of the tranche.
    pub tranche_id: String,
    /// ISO-4217 code of the currency `pv`, `accrued` and `cs01` are denominated in.
    pub currency: Currency,
    /// Present value of the tranche at valuation (currency units).
    pub pv: f64,
    /// Model clean settlement price as a percentage of the tranche's CURRENT
    /// balance.
    pub clean_price_pct: f64,
    /// Model dirty settlement price (clean plus accrued) as a percentage of
    /// the tranche's CURRENT balance.
    pub dirty_price_pct: f64,
    /// Accrued coupon at buyer settlement (currency units).
    pub accrued: f64,
    /// Pool factor of the note: `current_balance / original_balance`, so a
    /// price on original face is `clean_price_pct * factor`.
    pub factor: f64,
    /// Weighted-average life (years).
    pub wal: f64,
    /// Decimal yield reproducing the `target_price_pct` dirty settlement
    /// value, compounded at the note's coupon frequency with time in the
    /// note's day count (a par note yields its coupon); `None` for an
    /// impaired note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ytm: Option<f64>,
    /// Z-spread to `target_price_pct` (basis points): the constant spread over the
    /// discount curve equating the tranche's PV to that price. Zero when solved
    /// against the tranche's own model price (no spread to its curve-discounted value).
    pub z_spread_bp: f64,
    /// Credit-spread DV01 — currency change for a +1 bp z-spread shock. Negative
    /// for a long tranche (wider spreads reduce PV).
    pub cs01: f64,
    /// Spread duration (years): `-CS01 / (dirty settlement target · 1bp)`.
    pub spread_duration: f64,
    /// Spread convexity (years²): second-order z-spread sensitivity of the
    /// projected cashflows at the solved z-spread, on the same kernel as
    /// `spread_duration`.
    pub spread_convexity: f64,
    /// Effective (rate) duration (years): the tranche is re-projected with
    /// every rate curve in the market (discount and forward) bumped ±1 bp in
    /// parallel and the dirty settlement values differenced, so a floater's
    /// coupon resets move with the curve and its duration is short, while a
    /// fixed-coupon note's equals its modified duration.
    pub effective_duration: f64,
    /// Effective convexity (years²) from the same ±1 bp re-projection.
    pub effective_convexity: f64,
    /// Price the yield, z-spread and CS01 were solved against (% of current
    /// balance) — the supplied market price, else the deal's quote override,
    /// else the model price.
    pub target_price_pct: f64,
    /// Weighted-average life to the deal's assumed call (years); `None`
    /// without a `call_assumption` covering this tranche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wal_to_call: Option<f64>,
    /// Z-spread to the assumed call at `target_price_pct` (basis points).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_spread_to_call_bp: Option<f64>,
    /// Discount margin to the assumed call (basis points); floating-rate
    /// tranches only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_to_call_bp: Option<f64>,
    /// Discount margin to maturity at `target_price_pct` (basis points): the
    /// constant spread over the deal discount curve that reprices the
    /// floater's projected cashflows; `None` for fixed-rate tranches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_bp: Option<f64>,
    /// Registry metrics requested by the caller, valued on this note: every
    /// reprice (DV01, theta, Default01, Prepayment01, Recovery01,
    /// Severity01, …) values the tranche, not the deal, so the senior and
    /// the equity carry their own sensitivities that sum to the deal's.
    /// Units follow each metric's registry definition.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metrics: BTreeMap<MetricId, f64>,
}

/// Every discount and forward curve in `market` bumped `bp` basis points in
/// parallel (hazard, price, vol and other curves untouched).
fn bump_rate_curves(market: &MarketContext, bp: f64) -> Result<MarketContext> {
    let bumps: Vec<MarketBump> = market
        .iter_curves()
        .filter(|(_, storage)| {
            matches!(
                storage,
                CurveStorage::Discount(_) | CurveStorage::Forward(_)
            )
        })
        .map(|(id, _)| MarketBump::Curve {
            id: id.clone(),
            spec: BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::RateBp,
                value: bp,
                bump_type: BumpType::Parallel,
            },
        })
        .collect();
    market.bump(bumps)
}

/// Effective duration and convexity of one tranche: its cashflows are
/// re-projected with every rate curve bumped ±1 bp and the dirty settlement
/// values (`quote`'s settlement and accrued) differenced against
/// `base_dirty`. Both are `0.0` when the base value is not positive.
fn effective_rate_sensitivities(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: Date,
    quote: &super::quote::SettlementQuote,
    base_dirty: f64,
) -> Result<(f64, f64)> {
    if base_dirty <= 0.0 {
        return Ok((0.0, 0.0));
    }
    let dirty_under = |bp: f64| -> Result<f64> {
        let bumped = bump_rate_curves(market, bp)?;
        let flows = (deal).tranche_cashflows(tranche_id, &bumped, as_of)?;
        let disc = bumped.get_discount(deal.discount_curve_id.as_str())?;
        quote.model_dirty(&flows.cashflows, disc.as_ref())
    };
    let up = dirty_under(1.0)?;
    let down = dirty_under(-1.0)?;
    let duration = (down - up) / (2.0 * base_dirty * ONE_BASIS_POINT);
    let convexity =
        (up + down - 2.0 * base_dirty) / (base_dirty * ONE_BASIS_POINT * ONE_BASIS_POINT);
    Ok((duration, convexity))
}

/// One class's cashflows truncated at its assumed call: every flow before
/// the call payment date, then the stub accrued interest and the redemption
/// at `price_pct` on that date. `None` when the call falls after the last
/// payment.
fn truncate_at_call(
    flows: &TrancheCashflows,
    call: &CallAssumption,
) -> Result<Option<TrancheCashflows>> {
    let Some(period) = flows
        .accrual_periods
        .iter()
        .find(|period| period.payment_date >= call.date)
    else {
        return Ok(None);
    };
    let call_date = period.payment_date;
    let currency = period.opening_balance.currency();
    let opening = period.opening_balance.amount();
    let accrual = period.day_count.year_fraction(
        period.start,
        call_date,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    let interest = Money::new(opening * period.coupon_rate * accrual, currency)?;
    let principal = Money::new(opening * call.price_pct / 100.0, currency)?;

    let before = |dated: &[(Date, Money)]| -> Vec<(Date, Money)> {
        dated
            .iter()
            .filter(|(date, _)| *date < call_date)
            .copied()
            .collect()
    };
    let sum = |dated: &[(Date, Money)]| -> Result<Money> {
        dated
            .iter()
            .try_fold(Money::from((0_i64, currency)), |acc, (_, amount)| {
                acc.checked_add(*amount)
            })
    };
    let mut out = flows.clone();
    out.cashflows = before(&flows.cashflows);
    out.cashflows
        .push((call_date, interest.checked_add(principal)?));
    out.detailed_flows.retain(|flow| flow.date < call_date);
    out.accrual_periods
        .retain(|accrued| accrued.payment_date < call_date);
    out.accrual_periods.push(TrancheAccrualPeriod {
        start: period.start,
        end: call_date,
        payment_date: call_date,
        opening_balance: period.opening_balance,
        coupon_rate: period.coupon_rate,
        day_count: period.day_count,
    });
    out.interest_flows = before(&flows.interest_flows);
    out.interest_flows.push((call_date, interest));
    out.principal_flows = before(&flows.principal_flows);
    out.principal_flows.push((call_date, principal));
    out.pik_flows = before(&flows.pik_flows);
    out.deferred_flows = before(&flows.deferred_flows);
    out.writedown_flows = before(&flows.writedown_flows);
    out.final_balance = Money::from((0_i64, currency));
    out.total_interest = sum(&out.interest_flows)?;
    out.total_principal = sum(&out.principal_flows)?;
    out.total_pik = sum(&out.pik_flows)?;
    out.total_deferred = sum(&out.deferred_flows)?;
    out.total_writedown = sum(&out.writedown_flows)?;
    Ok(Some(out))
}

/// Compute the per-tranche metrics bundle ([`TrancheMetrics`]).
///
/// All named figures derive from the named tranche's own to-maturity
/// waterfall cashflows (the deal's `call_assumption` is lifted; the
/// `*_to_call` twins re-project or truncate at the call): PV, clean and
/// dirty price, accrued, WAL, the yield, the credit z-spread and CS01, spread
/// duration and convexity, effective duration and convexity (re-projected
/// under ±1 bp rate bumps) and, for floaters, the discount margin. This is
/// the per-note alternative to the deal-level metric registry, which
/// aggregates every tranche's flows into one stream.
///
/// `metrics` adds registry metrics valued on the note as the deal stands
/// (call assumption included), so a tranche sensitivity and the deal's
/// `price_with_metrics` figure describe the same deal. The registry pass runs
/// only when `metrics` is non-empty.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal owning the requested tranche,
///   its waterfall, and the curve identifiers used for projection.
/// * `tranche_id` - Identifier of the tranche to summarize from its own
///   cashflows rather than a deal-level aggregate.
/// * `market` - Market context supplying the deal's discount curve and any
///   rate/index data needed for cashflow projection.
/// * `as_of` - Valuation date; the deal's pricing-date override, when set,
///   moves discounting and the settlement quote, and scenario value
///   overrides apply to `pv`.
/// * `market_price_pct` - Clean settlement quote (% of the tranche's CURRENT
///   balance, the factor-adjusted secondary-market basis) the yield,
///   z-spread and CS01 are solved against. When `None`, the deal's
///   clean/dirty quote override is used, else the tranche's own model price;
///   the model target gives a zero z-spread while CS01, duration and
///   convexity remain meaningful sensitivities.
/// * `metrics` - Additional registry metrics to compute for this note
///   (empty for none). A yield is never requested from the registry for an
///   impaired note.
///
/// # Errors
///
/// Returns an error if the deal fails pricing validation, the tranche is
/// missing, the discount curve is unavailable, the cashflows cannot be
/// projected, a solve fails, a requested metric fails, or a figure is
/// non-finite.
pub fn calculate_tranche_metrics(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: Date,
    market_price_pct: Option<f64>,
    metrics: &[MetricId],
) -> Result<TrancheMetrics> {
    let lifecycle = ValidatedPricingLifecycle::new(deal)?;
    let valuation_date = lifecycle.effective_as_of(market, as_of);
    let tranche = deal.tranches.require(tranche_id)?;
    // Prices are per CURRENT face: the secondary market quotes a seasoned note
    // on its factor-adjusted balance, and `Tranche.current_balance` is the
    // balance the projection starts from.
    let current_balance = tranche.current_balance.amount();
    let original_balance = tranche.original_balance.amount();
    let factor = if original_balance > 0.0 {
        current_balance / original_balance
    } else {
        0.0
    };

    // To-maturity figures are projected without the call assumption; the
    // `*_to_call` twins re-project (deal call) or truncate (tranche call).
    let to_maturity = deal.call_assumption.as_ref().map(|_| {
        let mut deal = deal.clone();
        deal.call_assumption = None;
        deal
    });
    let projection_deal = to_maturity.as_ref().unwrap_or(deal);
    let cashflows = (projection_deal).tranche_cashflows(tranche_id, market, as_of)?;
    let disc = market.get_discount(deal.discount_curve_id.as_str())?;
    let curve = disc.as_ref();

    let pv = deal.value_tranche_cashflows(&cashflows, market, valuation_date)?;
    let quote = super::quote::SettlementQuote::for_tranche(
        deal,
        valuation_date,
        current_balance,
        &cashflows,
    )?;
    let model_dirty = quote.model_dirty(&cashflows.cashflows, curve)?;
    let wal = calculate_tranche_wal(&cashflows, valuation_date)?;
    let registry = registry_metrics(
        deal,
        tranche_id,
        market,
        as_of,
        valuation_date,
        metrics,
        &cashflows,
    )?;

    // An impaired note — no current face or no positive settlement value
    // (fully written down in the projection) — prices at zero and carries no
    // yield, spread or sensitivity instead of failing the solves.
    if current_balance <= 0.0 || model_dirty <= 0.0 {
        let result = TrancheMetrics {
            tranche_id: tranche_id.to_string(),
            currency: pv.currency(),
            pv: pv.amount(),
            clean_price_pct: 0.0,
            dirty_price_pct: 0.0,
            accrued: quote.accrued,
            factor,
            wal,
            ytm: None,
            z_spread_bp: 0.0,
            cs01: 0.0,
            spread_duration: 0.0,
            spread_convexity: 0.0,
            effective_duration: 0.0,
            effective_convexity: 0.0,
            target_price_pct: 0.0,
            wal_to_call: None,
            z_spread_to_call_bp: None,
            dm_to_call_bp: None,
            dm_bp: None,
            metrics: registry,
        };
        super::finite::ensure_tranche_metrics_finite(&result)?;
        return Ok(result);
    }

    let (effective_duration, effective_convexity) = effective_rate_sensitivities(
        projection_deal,
        tranche_id,
        market,
        as_of,
        &quote,
        model_dirty,
    )?;

    // Yield, z-spread (and the CS01 measured at it) are solved against the
    // supplied market price, the deal's quote override, or the model price.
    let target_value = match market_price_pct {
        Some(clean) => quote.clean_target(clean)?,
        None => quote.external_target(deal)?.unwrap_or(model_dirty),
    };
    let target_price_pct = quote.clean_price(target_value);
    let target_pv = Money::new(target_value, pv.currency())?;
    let ytm = super::risk::ytm::solve_yield(
        &cashflows.cashflows,
        quote.settlement,
        target_value,
        super::risk::ytm::note_periods_per_year(tranche),
        tranche.day_count,
    )?;
    let z_spread_bp =
        calculate_tranche_z_spread(&cashflows.cashflows, curve, target_pv, quote.settlement)?;
    let cs01 = calculate_tranche_cs01(
        &cashflows.cashflows,
        curve,
        z_spread_bp * ONE_BASIS_POINT,
        quote.settlement,
    )?;
    let spread_duration = -cs01 / (target_value * ONE_BASIS_POINT);
    let spread_convexity = calculate_tranche_spread_convexity(
        &cashflows.cashflows,
        curve,
        z_spread_bp * ONE_BASIS_POINT,
        quote.settlement,
    )?;
    // The discount margin is the z-spread of a floater's projected flows, solved
    // above against the same target and settlement.
    let dm_bp = matches!(tranche.coupon, RateSpec::Floating(_))
        .then(|| super::risk::spreads::checked_discount_margin_bp(z_spread_bp))
        .transpose()?;

    let call_flows = match deal
        .call_assumption
        .as_ref()
        .map(|call| (&call.scope, call))
    {
        Some((CallScope::Deal, _)) => Some((deal).tranche_cashflows(tranche_id, market, as_of)?),
        Some((CallScope::Tranche(id), call)) if id == tranche_id => {
            truncate_at_call(&cashflows, call)?
        }
        None | Some((CallScope::Tranche(_), _)) => None,
    };
    let (wal_to_call, z_spread_to_call_bp, dm_to_call_bp) = match call_flows {
        None => (None, None, None),
        Some(flows) => {
            let wal = calculate_tranche_wal(&flows, valuation_date)?;
            let z_spread =
                calculate_tranche_z_spread(&flows.cashflows, curve, target_pv, quote.settlement)?;
            // The discount margin is the z-spread of a floater's projected flows.
            let dm = matches!(tranche.coupon, RateSpec::Floating(_)).then_some(z_spread);
            (Some(wal), Some(z_spread), dm)
        }
    };

    let result = TrancheMetrics {
        tranche_id: tranche_id.to_string(),
        currency: pv.currency(),
        pv: pv.amount(),
        clean_price_pct: quote.clean_price(model_dirty),
        dirty_price_pct: quote.dirty_price(model_dirty),
        accrued: quote.accrued,
        factor,
        wal,
        ytm: Some(ytm),
        z_spread_bp,
        cs01,
        spread_duration,
        spread_convexity,
        effective_duration,
        effective_convexity,
        target_price_pct,
        wal_to_call,
        z_spread_to_call_bp,
        dm_to_call_bp,
        dm_bp,
        metrics: registry,
    };
    super::finite::ensure_tranche_metrics_finite(&result)?;
    Ok(result)
}

/// Registry `metrics` valued on the note: the priced instrument in the metric
/// context is the tranche view, so every reprice values this tranche, and the
/// context carries the tranche's flows (as the deal stands, call assumption
/// included) for the calculators that read them. Returns only the requested
/// ids; an empty request computes nothing.
fn registry_metrics(
    deal: &StructuredCredit,
    tranche_id: &str,
    market: &MarketContext,
    as_of: Date,
    valuation_date: Date,
    metrics: &[MetricId],
    to_maturity: &TrancheCashflows,
) -> Result<BTreeMap<MetricId, f64>> {
    if metrics.is_empty() {
        return Ok(BTreeMap::new());
    }
    let flows = match deal.call_assumption {
        Some(_) => deal.tranche_cashflows(tranche_id, market, as_of)?,
        None => to_maturity.clone(),
    };
    let tranche = deal.tranches.require(tranche_id)?;
    let pv = deal.value_tranche_cashflows(&flows, market, valuation_date)?;
    let mut context = MetricContext::new(
        std::sync::Arc::new(
            crate::structured_credit::types::StructuredCreditTranche::new(deal.clone(), tranche_id),
        ),
        std::sync::Arc::new(market.clone()),
        valuation_date,
        pv,
        MetricContext::default_config(),
    );
    context.cashflows = Some(flows.cashflows.clone());
    context.tagged_cashflows = Some(flows.detailed_flows.clone());
    context.discount_curve_id = Some(deal.discount_curve_id.to_owned());
    context.notional = Some(tranche.current_balance);
    context.set_ext(flows);
    let computed = deal.family().metrics().compute(metrics, &mut context)?;
    Ok(metrics
        .iter()
        .filter_map(|id| computed.get(id).map(|value| (id.clone(), *value)))
        .collect())
}

#[cfg(test)]
mod currency_stamp_tests {
    use super::*;

    /// Monetary outputs carry their ISO-4217 currency.
    #[test]
    fn tranche_metrics_carry_their_currency() {
        let metrics = TrancheMetrics {
            tranche_id: "A".to_string(),
            currency: Currency::EUR,
            pv: 1_000.0,
            clean_price_pct: 100.0,
            dirty_price_pct: 100.0,
            accrued: 0.0,
            factor: 1.0,
            wal: 3.0,
            ytm: None,
            z_spread_bp: 0.0,
            cs01: -1.0,
            spread_duration: 3.0,
            spread_convexity: 12.0,
            effective_duration: 3.0,
            effective_convexity: 12.0,
            target_price_pct: 100.0,
            wal_to_call: None,
            z_spread_to_call_bp: None,
            dm_to_call_bp: None,
            dm_bp: None,
            metrics: BTreeMap::new(),
        };
        let json = serde_json::to_string(&metrics).expect("serialize");
        assert!(
            json.contains("\"currency\":\"EUR\""),
            "the currency must reach the wire alongside pv/cs01; got {json}"
        );

        let parsed: TrancheMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.currency, Currency::EUR);
    }

    #[test]
    fn metrics_require_currency() {
        let incomplete = r#"{
            "tranche_id": "A",
            "pv": 1000.0,
            "clean_price_pct": 100.0,
            "dirty_price_pct": 100.0,
            "accrued": 0.0,
            "factor": 1.0,
            "wal": 3.0,
            "z_spread_bp": 0.0,
            "cs01": -1.0,
            "spread_duration": 3.0,
            "spread_convexity": 12.0,
            "effective_duration": 3.0,
            "effective_convexity": 12.0,
            "target_price_pct": 100.0
        }"#;
        let error = serde_json::from_str::<TrancheMetrics>(incomplete)
            .expect_err("currency is required by the canonical metrics contract");
        assert!(error.to_string().contains("currency"));
    }

    /// The currency stamp is a typed ISO-4217 code: unknown codes fail to load.
    #[test]
    fn metrics_reject_unknown_currency_codes() {
        let mut json = serde_json::to_value(TrancheMetrics {
            tranche_id: "A".to_string(),
            currency: Currency::USD,
            pv: 1_000.0,
            clean_price_pct: 100.0,
            dirty_price_pct: 100.0,
            accrued: 0.0,
            factor: 1.0,
            wal: 3.0,
            ytm: None,
            z_spread_bp: 0.0,
            cs01: -1.0,
            spread_duration: 3.0,
            spread_convexity: 12.0,
            effective_duration: 3.0,
            effective_convexity: 12.0,
            target_price_pct: 100.0,
            wal_to_call: None,
            z_spread_to_call_bp: None,
            dm_to_call_bp: None,
            dm_bp: None,
            metrics: BTreeMap::new(),
        })
        .expect("serialize");
        json["currency"] = serde_json::json!("NOT-A-CODE");
        assert!(serde_json::from_value::<TrancheMetrics>(json).is_err());
    }
}

/// Equity analytics from the residual class's projected cashflows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityMetrics {
    /// Identifier of the equity tranche.
    pub tranche_id: String,
    /// ISO-4217 code of the currency the amounts are denominated in.
    pub currency: Currency,
    /// Cash invested on the valuation date: current balance × purchase price.
    pub invested: f64,
    /// Annualized IRR of `−invested` on the valuation date against every
    /// projected distribution (XIRR); `None` when no rate solves.
    pub irr: Option<f64>,
    /// Multiple on invested capital: total distributions ÷ `invested`.
    pub moic: f64,
    /// Present value of the distributions on the deal's discount curve, as a
    /// percent of the equity's current balance.
    pub nav_pct: f64,
    /// Each distribution as a fraction of `invested`, by payment date.
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<(String, f64)>"))]
    pub cash_on_cash: Vec<(Date, f64)>,
}

/// Equity analytics for the deal's residual class.
///
/// # Arguments
///
/// * `deal` - Validated structured-credit deal with an equity tranche.
/// * `market` - Market context used to project the waterfall and discount the
///   distributions on `deal.discount_curve_id`.
/// * `as_of` - Valuation date; the investment is made on this date and only
///   later distributions count.
/// * `purchase_price_pct` - Purchase price as a percent of the equity's
///   current balance; `None` invests at par.
///
/// # Returns
///
/// IRR, MOIC, NAV and the cash-on-cash series; distributions include reserve
/// interest routed directly to the equity class.
///
/// # Errors
///
/// Returns an error when the deal fails validation, has no equity tranche,
/// or the projection or discount curve is unavailable.
pub fn calculate_equity_metrics(
    deal: &StructuredCredit,
    market: &MarketContext,
    as_of: Date,
    purchase_price_pct: Option<f64>,
) -> Result<EquityMetrics> {
    deal.validate_for_pricing()?;
    let equity = deal
        .tranches
        .tranches
        .iter()
        .find(|t| t.seniority == TrancheSeniority::Equity)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation("deal has no equity tranche".to_string())
        })?;
    let price = purchase_price_pct.unwrap_or(100.0);
    if !price.is_finite() || price < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "purchase_price_pct ({price}) must be a finite non-negative percent"
        )));
    }
    let current_balance = equity.current_balance.amount();
    let invested = current_balance * price / 100.0;
    let cashflows = (deal).tranche_cashflows(equity.id.as_str(), market, as_of)?;
    let disc = market.get_discount(deal.discount_curve_id.as_str())?;
    let curve = disc.as_ref();

    let mut flows: Vec<(Date, f64)> = vec![(as_of, -invested)];
    let mut total = 0.0_f64;
    let mut pv = 0.0_f64;
    let mut cash_on_cash = Vec::new();
    for (date, amount) in &cashflows.cashflows {
        if *date <= as_of {
            continue;
        }
        let cash = amount.amount();
        flows.push((*date, cash));
        total += cash;
        pv += cash * curve.df_between_dates(as_of, *date)?;
        cash_on_cash.push((*date, if invested > 0.0 { cash / invested } else { 0.0 }));
    }
    let irr = if invested > 0.0 {
        finstack_quant_core::cashflow::xirr(&flows, None)
            .ok()
            .filter(|irr| irr.is_finite())
    } else {
        None
    };
    Ok(EquityMetrics {
        tranche_id: equity.id.to_string(),
        currency: equity.current_balance.currency(),
        invested,
        irr,
        moic: if invested > 0.0 {
            total / invested
        } else {
            0.0
        },
        nav_pct: if current_balance > 0.0 {
            pv / current_balance * 100.0
        } else {
            0.0
        },
        cash_on_cash,
    })
}
