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
use crate::instruments::fixed_income::structured_credit::{
    CallAssumption, CallScope, StructuredCredit, TrancheAccrualPeriod, TrancheCashflows,
    TrancheCoupon, TrancheSeniority,
};
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
#[serde(deny_unknown_fields)]
pub struct TrancheMetrics {
    /// Identifier of the tranche.
    pub tranche_id: String,
    /// ISO-4217 code of the currency `pv` and `cs01` are denominated in.
    pub currency: String,
    /// Present value of the tranche (currency units).
    pub pv: f64,
    /// Model clean settlement price as a percentage of the tranche's CURRENT
    /// balance (the factor-adjusted secondary-market quote basis).
    pub price_pct: f64,
    /// Pool factor of the note: `current_balance / original_balance`, so a
    /// price on original face is `price_pct * factor`.
    pub factor: f64,
    /// Weighted-average life (years).
    pub wal: f64,
    /// Z-spread to `target_price_pct` (basis points): the constant spread over the
    /// discount curve equating the tranche's PV to that price. Zero when solved
    /// against the tranche's own model price (no spread to its curve-discounted value).
    pub z_spread_bp: f64,
    /// Credit-spread DV01 — currency change for a +1 bp z-spread shock. Negative
    /// for a long tranche (wider spreads reduce PV).
    pub cs01: f64,
    /// Spread duration (years): `-CS01 / (dirty settlement target · 1bp)`.
    pub spread_duration: f64,
    /// Modified (rate) duration of the projected cashflows (years).
    pub modified_duration: f64,
    /// Modified convexity of the projected cashflows (years²).
    pub convexity: f64,
    /// Price the z-spread/CS01 were solved against (% of current balance) —
    /// the supplied market price, or the model price when none was given.
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
/// * `market_price_pct` - Clean settlement quote (% of the tranche's CURRENT
///   balance, the factor-adjusted secondary-market basis) the z-spread and
///   CS01 are solved against. When `None`, the tranche's own model price is used,
///   unless the deal supplies a clean/dirty quote override. The model target
///   gives a zero z-spread while CS01, duration and
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
    let cashflows = to_maturity
        .as_ref()
        .unwrap_or(deal)
        .get_tranche_cashflows(tranche_id, market, as_of)?;
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
    let pv_money = Money::new(pv, deal.pool.get_base_currency())?;
    let quote =
        super::quote::SettlementQuote::for_tranche(deal, as_of, current_balance, &cashflows)?;
    let model_dirty = quote.model_dirty(&cashflows.cashflows, curve)?;
    let price_pct = quote.clean_price(model_dirty);

    let wal = calculate_tranche_wal(&cashflows, as_of)?;
    let modified_duration = calculate_tranche_duration(
        &cashflows.cashflows,
        curve,
        quote.settlement,
        Money::new(model_dirty, pv_money.currency())?,
    )?;
    let convexity = calculate_tranche_convexity(&cashflows.cashflows, curve, quote.settlement)?;

    // Z-spread (and the CS01 measured at it) are solved against the supplied
    // market price, or the tranche's own model price when none is given.
    let target_value = match market_price_pct {
        Some(clean) => quote.clean_target(clean)?,
        None => quote.external_target(deal)?.unwrap_or(model_dirty),
    };
    let target_price_pct = quote.clean_price(target_value);
    let target_pv = Money::new(target_value, pv_money.currency())?;
    let z_spread_bp =
        calculate_tranche_z_spread(&cashflows.cashflows, curve, target_pv, quote.settlement)?;
    let cs01 = calculate_tranche_cs01(
        &cashflows.cashflows,
        curve,
        z_spread_bp * 1e-4,
        quote.settlement,
    )?;
    quote.dirty_target(target_value)?;
    let spread_duration = -cs01 / (target_value * ONE_BASIS_POINT);

    let call_flows = match deal
        .call_assumption
        .as_ref()
        .map(|call| (&call.scope, call))
    {
        Some((CallScope::Deal, _)) => Some(deal.get_tranche_cashflows(tranche_id, market, as_of)?),
        Some((CallScope::Tranche(id), call)) if id == tranche_id => {
            truncate_at_call(&cashflows, call)?
        }
        None | Some((CallScope::Tranche(_), _)) => None,
    };
    let (wal_to_call, z_spread_to_call_bp, dm_to_call_bp) = match call_flows {
        None => (None, None, None),
        Some(flows) => {
            let wal = calculate_tranche_wal(&flows, as_of)?;
            let z_spread =
                calculate_tranche_z_spread(&flows.cashflows, curve, target_pv, quote.settlement)?;
            // The discount margin is the z-spread of a floater's projected flows.
            let dm = matches!(tranche.coupon, TrancheCoupon::Floating(_)).then_some(z_spread);
            (Some(wal), Some(z_spread), dm)
        }
    };

    Ok(TrancheMetrics {
        tranche_id: tranche_id.to_string(),
        currency: pv_money.currency().to_string(),
        pv,
        price_pct,
        factor,
        wal,
        z_spread_bp,
        cs01,
        spread_duration,
        modified_duration,
        convexity,
        target_price_pct,
        wal_to_call,
        z_spread_to_call_bp,
        dm_to_call_bp,
    })
}

#[cfg(test)]
mod currency_stamp_tests {
    use super::*;

    /// Monetary outputs carry their ISO-4217 currency.
    #[test]
    fn tranche_metrics_carry_their_currency() {
        let metrics = TrancheMetrics {
            tranche_id: "A".to_string(),
            currency: "EUR".to_string(),
            pv: 1_000.0,
            price_pct: 100.0,
            factor: 1.0,
            wal: 3.0,
            z_spread_bp: 0.0,
            cs01: -1.0,
            spread_duration: 3.0,
            modified_duration: 3.0,
            convexity: 12.0,
            target_price_pct: 100.0,
            wal_to_call: None,
            z_spread_to_call_bp: None,
            dm_to_call_bp: None,
        };
        let json = serde_json::to_string(&metrics).expect("serialize");
        assert!(
            json.contains("\"currency\":\"EUR\""),
            "the currency must reach the wire alongside pv/cs01; got {json}"
        );

        let parsed: TrancheMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.currency, "EUR");
    }

    #[test]
    fn metrics_require_currency() {
        let incomplete = r#"{
            "tranche_id": "A",
            "pv": 1000.0,
            "price_pct": 100.0,
            "factor": 1.0,
            "wal": 3.0,
            "z_spread_bp": 0.0,
            "cs01": -1.0,
            "spread_duration": 3.0,
            "modified_duration": 3.0,
            "convexity": 12.0,
            "target_price_pct": 100.0
        }"#;
        let error = serde_json::from_str::<TrancheMetrics>(incomplete)
            .expect_err("currency is required by the canonical metrics contract");
        assert!(error.to_string().contains("currency"));
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
    pub currency: String,
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
    let cashflows = deal.get_tranche_cashflows(equity.id.as_str(), market, as_of)?;
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
        currency: equity.current_balance.currency().to_string(),
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
