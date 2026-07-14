//! Per-flow cashflow export with discount-factor / survival-probability / PV enrichment.
//!
//! Designed as the single Rust entry point behind the `instrument_cashflows_json`
//! Python and WASM bindings. Produces a structured envelope for any instrument
//! that is priceable under either the `Discounting` or `HazardRate` model. For
//! those two models, `sum(flows.pv) ≈ base_value` within rounding.
//!
//! # Not supported
//!
//! Option / tree / Monte Carlo / PDE / static-replication pricers are rejected
//! with a clear error explaining which models *are* valid for the given
//! instrument type. This guarantees reconciliation: if the exporter answers,
//! the sum is the price.
//!
//! # Columns
//!
//! Always populated (null-as-needed): `date, amount, currency, kind,
//! accrual_factor, year_fraction, rate, reset_date, discount_factor,
//! survival_probability, conditional_default_prob, inflation_index_ratio,
//! prepayment_smm, beginning_balance, ending_balance, pv`.
//!
//! Hazard-only columns are populated when `model = "hazard_rate"`.
//! Inflation / MBS columns are populated by concrete-type downcasts when the
//! instrument is `InflationLinkedBond` / `AgencyMbsPassthrough`.
//!
//! **CMO tranche pool state** is intentionally exported as `null`: the waterfall
//! engine does not yet expose a stable per-tranche balance hook for this path.
//! Consumers should not treat `null` as missing data for non-CMO instruments.

use finstack_quant_cashflows::aggregation::credit_adjusted_cashflow_pv;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use serde::Serialize;

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::instruments::fixed_income::mbs_passthrough::{
    pricer::project_cashflows as project_mbs_cashflows, AgencyMbsPassthrough,
};
use crate::instruments::fx::fx_swap::FxSwap;
use crate::instruments::json_loader::InstrumentEnvelope;
use crate::instruments::rates::xccy_swap::XccySwap;
use crate::pricer::{shared_standard_registry, ModelKey, PricerKey};

// ---------------------------------------------------------------------------
// Envelope schema
// ---------------------------------------------------------------------------

/// Top-level JSON envelope returned by [`instrument_cashflows_json`].
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct InstrumentCashflowEnvelope {
    /// Instrument identifier.
    pub instrument_id: String,
    /// Reporting currency used for row PVs and `total_pv`.
    pub currency: Currency,
    /// Model key used (`"discounting"` or `"hazard_rate"`).
    pub model: String,
    /// Valuation date.
    pub as_of: Date,
    /// Discount curve ID used.
    pub discount_curve_id: CurveId,
    /// Hazard curve ID used (omitted for `discounting` model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hazard_curve_id: Option<CurveId>,
    /// Recovery rate from the hazard curve (omitted for `discounting` model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_rate: Option<f64>,
    /// Per-row enriched cashflows.
    pub flows: Vec<CashflowRow>,
    /// Sum of `flows[i].pv`. Matches `base_value` for supported products.
    pub total_pv: f64,
    /// `true` when `total_pv` agrees with the instrument's canonical
    /// `base_value` (`Instrument::value`) within rounding tolerance.
    ///
    /// This is verified per call, not assumed: it is `false` when the per-flow
    /// PV sum does not reconcile with `base_value` (for example, when the
    /// requested `model` differs from the instrument's default pricing model)
    /// or when `base_value` itself cannot be computed.
    pub reconciles_with_base_value: bool,
}

/// Single-row enriched cashflow view.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct CashflowRow {
    /// Payment date.
    pub date: Date,
    /// Signed cashflow amount in row currency.
    pub amount: f64,
    /// Row currency (matters for `XccySwap` / `FxSwap`).
    pub currency: Currency,
    /// `CFKind` discriminator (serde rename: `fixed`, `notional`, …).
    pub kind: CFKind,
    /// Accrual factor stored on the `CashFlow`.
    pub accrual_factor: f64,
    /// Year fraction from `as_of` to `date` under the discount curve's day count.
    pub year_fraction: f64,
    /// Projected / contractual rate when present (floats, real-coupon rates, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    /// Reset date when the flow is a floating-rate fixing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_date: Option<Date>,
    /// `df(as_of, date)`.
    pub discount_factor: f64,
    /// Discount curve used for this row.
    pub discount_curve_id: CurveId,
    /// Cumulative survival probability (hazard mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub survival_probability: Option<f64>,
    /// Interval default probability `SP(t_{i-1}) − SP(t_i)` (hazard mode only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditional_default_prob: Option<f64>,
    /// Inflation index ratio (populated for `InflationLinkedBond`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inflation_index_ratio: Option<f64>,
    /// Single Monthly Mortality for the period (populated for agency MBS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepayment_smm: Option<f64>,
    /// Beginning pool balance for the period (agency MBS only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beginning_balance: Option<f64>,
    /// Ending pool balance for the period (agency MBS only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ending_balance: Option<f64>,
    /// Per-flow present value in the envelope reporting currency. Sums to `total_pv`.
    pub pv: f64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build the enriched cashflow envelope for a tagged instrument and serialize to JSON.
///
/// # Errors
///
/// Returns `Error::Validation` if the model string is not one of
/// `{"discounting", "hazard_rate"}`, if the `(instrument_type, model)` pair is
/// not in the standard pricer registry, if required curves are missing from
/// the market, or if the schedule mixes currencies.
pub fn instrument_cashflows_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<String> {
    let envelope = build_envelope(instrument_json, market, as_of, model)?;
    serde_json::to_string(&envelope)
        .map_err(|e| Error::Validation(format!("failed to serialize cashflow envelope: {e}")))
}

fn build_envelope(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<InstrumentCashflowEnvelope> {
    // --- Parse inputs ---
    let model_key: ModelKey = model.parse().map_err(|e: String| {
        Error::Validation(format!(
            "unknown model '{model}': {e}. Supported: 'discounting', 'hazard_rate'"
        ))
    })?;
    if !matches!(model_key, ModelKey::Discounting | ModelKey::HazardRate) {
        return Err(Error::Validation(format!(
            "model '{model}' not supported for instrument_cashflows; supported: 'discounting', 'hazard_rate'"
        )));
    }

    let as_of_date = finstack_quant_core::dates::parse_iso_date(as_of)
        .map_err(|e| Error::Validation(format!("invalid as_of '{as_of}': {e}")))?;

    let instrument = InstrumentEnvelope::from_str(instrument_json)?;
    let instrument_type = instrument.key();
    let instrument_id = instrument.id().to_string();

    // --- Pricer registry gate: ensure the (type, model) pair is supported ---
    let registry = shared_standard_registry();
    let pricer_key = PricerKey::new(instrument_type, model_key);
    if registry.get_pricer(pricer_key).is_none() {
        return Err(Error::Validation(format!(
            "instrument type {instrument_type:?} is not priced under model '{model}' in instrument_cashflows; \
             this exporter supports only 'discounting' / 'hazard_rate' products where sum(pv) == base_value"
        )));
    }

    // --- Resolve curves ---
    let deps = instrument.market_dependencies()?;
    let curves = deps.curve_dependencies();
    let default_discount_curve_id = curves.discount_curves.first().cloned().ok_or_else(|| {
        Error::Validation(
            "instrument has no declared discount curve; cannot compute cashflow DFs".into(),
        )
    })?;
    let mut currency_discount_curves = std::collections::HashMap::new();
    let (discount_curve_id, reporting_currency) = if let Some(swap) =
        instrument.as_any().downcast_ref::<FxSwap>()
    {
        currency_discount_curves.insert(swap.base_currency, swap.foreign_discount_curve_id.clone());
        currency_discount_curves
            .insert(swap.quote_currency, swap.domestic_discount_curve_id.clone());
        (
            swap.domestic_discount_curve_id.clone(),
            Some(swap.quote_currency),
        )
    } else if let Some(swap) = instrument.as_any().downcast_ref::<XccySwap>() {
        currency_discount_curves.insert(swap.leg1.currency, swap.leg1.discount_curve_id.clone());
        currency_discount_curves.insert(swap.leg2.currency, swap.leg2.discount_curve_id.clone());
        let primary = currency_discount_curves
            .get(&swap.reporting_currency)
            .cloned()
            .unwrap_or(default_discount_curve_id);
        (primary, Some(swap.reporting_currency))
    } else {
        (default_discount_curve_id, None)
    };
    let primary_discount = market.get_discount(discount_curve_id.as_str())?;

    let (hazard_curve_id, hazard_arc) = if matches!(model_key, ModelKey::HazardRate) {
        let id = curves.credit_curves.first().cloned().ok_or_else(|| {
            Error::Validation(
                "instrument declares no hazard curve; hazard_rate model requires one".into(),
            )
        })?;
        let arc = market.get_hazard(id.as_str())?;
        (Some(id), Some(arc))
    } else {
        (None, None)
    };
    let recovery_rate = hazard_arc.as_ref().map(|h| h.recovery_rate());

    // --- Build one schedule, retaining MBS diagnostics from the same projection. ---
    let (schedule, mbs_state) =
        if let Some(mbs) = instrument.as_any().downcast_ref::<AgencyMbsPassthrough>() {
            let projection = project_mbs_cashflows(mbs, as_of_date, Some(mbs.wam + 12))?;
            let states: std::collections::HashMap<Date, MbsState> = projection
                .diagnostics
                .into_iter()
                .map(|row| {
                    (
                        row.payment_date,
                        MbsState {
                            smm: row.smm,
                            beginning_balance: row.beginning_balance,
                            ending_balance: row.ending_balance,
                        },
                    )
                })
                .collect();
            (projection.schedule, Some(states))
        } else {
            (instrument.cashflow_schedule(market, as_of_date)?, None)
        };
    let inflation_bond: Option<&InflationLinkedBond> =
        instrument.as_any().downcast_ref::<InflationLinkedBond>();

    // --- Iterate flows ---
    let dc_ctx = DayCountContext::default();

    // Survival probability at `as_of` under the hazard curve's own time origin.
    // The exporter must report *conditional* survival Q(as_of, T) = S(T)/S(as_of)
    // so the PV is correct even when the hazard curve's base date differs from
    // `as_of` (a seasoned instrument or a reused prior-day curve). This mirrors
    // `HazardBondEngine::price_raw`, which renormalizes by S(as_of).
    let survival_at_as_of = match hazard_arc.as_ref() {
        Some(h) => {
            let s0 = h.sp_on_date(as_of_date)?;
            if !s0.is_finite() || s0 <= 0.0 {
                return Err(Error::Validation(format!(
                    "instrument_cashflows: hazard curve '{}' implies survival probability {s0} \
                     at as_of {as_of_date}; an already-defaulted name cannot be exported as a \
                     surviving cashflow stream — value recovery proceeds instead. \
                     Check the hazard curve's base date and calibration.",
                    hazard_curve_id
                        .as_ref()
                        .map(|id| id.as_str())
                        .unwrap_or("<unknown>")
                )));
            }
            Some(s0)
        }
        None => None,
    };

    let mut rows = Vec::with_capacity(schedule.flows.len());
    let mut envelope_currency = reporting_currency;
    let mut prev_sp = 1.0_f64;

    for flow in &schedule.flows {
        let ccy = flow.amount.currency();
        if envelope_currency.is_none() {
            envelope_currency = Some(ccy);
        }

        let row_discount_curve_id = currency_discount_curves
            .get(&ccy)
            .unwrap_or(&discount_curve_id);
        let row_discount = if row_discount_curve_id == &discount_curve_id {
            std::sync::Arc::clone(&primary_discount)
        } else {
            market.get_discount(row_discount_curve_id.as_str())?
        };
        let curve_dc = row_discount.day_count();
        let year_fraction = curve_dc.signed_year_fraction(as_of_date, flow.date, dc_ctx)?;

        // Flows on or before `as_of` are already settled (holder view): they
        // are not part of present value, matching `core::cashflow::npv` and
        // therefore `Instrument::value` / `base_value` (2026-06-09 core quant
        // review: market-standard position value excludes flows with
        // `date <= as_of`). Discounting them would also extrapolate the curve
        // backwards and can return DF > 1. The rows stay in the export for the
        // audit trail (face amount, DF 1, no default adjustment) but carry
        // `pv = 0` so `total_pv` reconciles with `base_value` — including for
        // T+0 instruments like a deposit valued on its effective date, whose
        // initial notional exchange settles on `as_of`.
        let settled = flow.date <= as_of_date;
        let (discount_factor, survival_probability, conditional_default_prob) = if settled {
            let sp = if hazard_arc.is_some() {
                Some(1.0)
            } else {
                None
            };
            let cond_pd = sp.map(|_| 0.0);
            (1.0, sp, cond_pd)
        } else {
            // Date-based discounting: `DiscountCurve::df` expects time from
            // the curve's own `base_date`, not from `as_of`. For a seasoned
            // instrument (`as_of != base_date`), feeding an `as_of`-relative
            // year fraction into `df` lands on the wrong time origin and
            // breaks reconciliation with `Instrument::value`, which uses
            // `df_between_dates`. Use the same date-based helper here.
            let df = row_discount.df_between_dates(as_of_date, flow.date)?;
            let (sp, cond_pd) = match (hazard_arc.as_ref(), survival_at_as_of) {
                (Some(h), Some(s0)) => {
                    // Conditional survival Q(as_of, T) = S(T) / S(as_of).
                    let s_t = h.sp_on_date(flow.date)?;
                    let sp = (s_t / s0).clamp(0.0, 1.0);
                    let cond_pd = (prev_sp - sp).max(0.0);
                    prev_sp = sp;
                    (Some(sp), Some(cond_pd))
                }
                _ => (None, None),
            };
            (df, sp, cond_pd)
        };

        let native_pv = credit_adjusted_cashflow_pv(
            flow,
            discount_factor,
            survival_probability.unwrap_or(1.0),
            recovery_rate,
            as_of_date,
        )?;
        let pv = market
            .convert_money(
                Money::new(native_pv, ccy),
                envelope_currency.unwrap_or(ccy),
                as_of_date,
            )?
            .amount();

        let mbs_row = mbs_state.as_ref().and_then(|m| m.get(&flow.date));

        rows.push(CashflowRow {
            date: flow.date,
            amount: flow.amount.amount(),
            currency: ccy,
            kind: flow.kind,
            accrual_factor: flow.accrual_factor,
            year_fraction,
            rate: flow.rate,
            reset_date: flow.reset_date,
            discount_factor,
            discount_curve_id: row_discount_curve_id.clone(),
            survival_probability,
            conditional_default_prob,
            inflation_index_ratio: inflation_bond
                .and_then(|b| b.index_ratio_from_market(flow.date, market).ok()),
            prepayment_smm: mbs_row.map(|s| s.smm),
            beginning_balance: mbs_row.map(|s| s.beginning_balance),
            ending_balance: mbs_row.map(|s| s.ending_balance),
            pv,
        });
    }

    // The reporting currency comes from the schedule's flows. An empty
    // schedule (or one whose currency couldn't be inferred for any other
    // reason) is a real problem for an XccySwap / FxSwap export, where a
    // silent USD default would mis-tag `total_pv` against the wrong unit.
    // Fail loudly instead.
    let currency = envelope_currency.ok_or_else(|| {
        Error::Validation(format!(
            "instrument_cashflows: cannot determine reporting currency for instrument '{instrument_id}' \
             — schedule has no flows with a currency stamp. \
             This typically indicates a corrupt or empty cashflow schedule."
        ))
    })?;

    let total_pv = sum_pvs(rows.iter().map(|row| row.pv));

    // Honest reconciliation flag: compare `total_pv` against the instrument's
    // canonical `base_value` (`Instrument::value`) rather than asserting `true`
    // unconditionally. The flag is only `true` when the per-flow PV sum
    // actually agrees with `base_value` within rounding. This catches genuine
    // mismatches — e.g. a model whose `base_value` pricer diverges from the
    // discounting/hazard cashflow sum, or a future change that re-introduces a
    // discounting bug — instead of silently claiming a reconciliation that
    // does not hold.
    let reconciles_with_base_value = match instrument.value(market, as_of_date) {
        Ok(base_value) if base_value.currency() == currency => {
            let base = base_value.amount();
            // Relative tolerance with an absolute floor: per-flow PVs and
            // `base_value` are summed by different (compensated) accumulators,
            // so exact equality is not expected, but any real time-origin or
            // model mismatch is orders of magnitude larger than this bound.
            let tol = (base.abs() * 1e-6).max(1e-6);
            (total_pv - base).abs() <= tol
        }
        // If the instrument cannot be priced via `Instrument::value` we cannot
        // assert reconciliation; report `false` rather than an unverified `true`.
        Ok(_) | Err(_) => false,
    };

    Ok(InstrumentCashflowEnvelope {
        instrument_id,
        currency,
        model: model_key.to_string(),
        as_of: as_of_date,
        discount_curve_id,
        hazard_curve_id,
        recovery_rate,
        flows: rows,
        total_pv,
        reconciles_with_base_value,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct MbsState {
    smm: f64,
    beginning_balance: f64,
    ending_balance: f64,
}

fn sum_pvs<I>(pvs: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let mut acc = NeumaierAccumulator::new();
    for pv in pvs {
        acc.add(pv);
    }
    acc.total()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::Bond;
    use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
    use crate::instruments::Instrument;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    fn serialize_bond(bond: &Bond) -> String {
        let envelope = InstrumentEnvelope {
            schema: InstrumentEnvelope::CURRENT_SCHEMA.to_string(),
            instrument: InstrumentJson::Bond(bond.clone()),
        };
        serde_json::to_string(&envelope).expect("serialize bond envelope")
    }

    #[test]
    fn mixed_currency_fx_swap_rows_use_native_curves_and_reporting_currency_pv() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let swap = FxSwap::example();
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("fx quote");
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.95)])
                    .build()
                    .expect("usd curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("eur curve"),
            )
            .insert_fx(FxMatrix::new(provider));
        let instrument = InstrumentEnvelope {
            schema: InstrumentEnvelope::CURRENT_SCHEMA.to_string(),
            instrument: InstrumentJson::FxSwap(swap),
        };
        let json = serde_json::to_string(&instrument).expect("serialize fx swap");

        let payload = instrument_cashflows_json(&json, &market, "2024-01-01", "discounting")
            .expect("mixed-currency cashflow export");
        let envelope: InstrumentCashflowEnvelope =
            serde_json::from_str(&payload).expect("parse envelope");

        assert_eq!(envelope.currency, Currency::USD);
        assert!(envelope.reconciles_with_base_value);
        assert!(envelope.flows.iter().any(|row| {
            row.currency == Currency::EUR && row.discount_curve_id.as_str() == "EUR-OIS"
        }));
        assert!(envelope.flows.iter().any(|row| {
            row.currency == Currency::USD && row.discount_curve_id.as_str() == "USD-OIS"
        }));
    }

    #[test]
    fn discounting_reconciles_with_schedule_pv_for_fixed_bond() {
        use crate::instruments::common_impl::helpers::schedule_pv_using_curve_dc_raw;

        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let bond = Bond::fixed(
            "BOND-DISC-RECONCILE",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("bond");

        // Use as_of strictly after issue so the initial notional outflow is
        // unambiguously in the past and excluded by both the schedule-based
        // engine and `base_value`. Note this makes the instrument *seasoned*
        // (as_of != curve base_date), which is exactly the case where a wrong
        // time origin in the exporter's discount factors would surface.
        let as_of_date = Date::from_calendar_date(2025, Month::July, 1).expect("date");

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc);

        let json = serialize_bond(&bond);
        let payload = instrument_cashflows_json(&json, &market, "2025-07-01", "discounting")
            .expect("cashflows envelope");
        let envelope: InstrumentCashflowEnvelope =
            serde_json::from_str(&payload).expect("parse envelope");

        // Reference PV uses `schedule_pv_using_curve_dc_raw`, which discounts via
        // the date-based `df_between_dates(as_of, date)` helper — the same time
        // origin used by the discounting `BondEngine` (and therefore by
        // `base_value`). The previous reference, `schedule_pv_using_curve_dc`,
        // routes through `core::npv`, which discounts with `df(t)` for `t`
        // measured from `as_of`; for this seasoned bond that lands on the wrong
        // time origin and disagrees with `base_value` by ~$3.5k on $1M notional.
        let discount_curve_id = bond
            .market_dependencies()
            .expect("deps")
            .curve_dependencies()
            .discount_curves
            .first()
            .cloned()
            .expect("bond should declare a discount curve");
        let reference =
            schedule_pv_using_curve_dc_raw(&bond, &market, as_of_date, &discount_curve_id)
                .expect("schedule pv");

        let diff = (envelope.total_pv - reference).abs();
        assert!(
            diff < 1e-2,
            "total_pv {} should reconcile with schedule PV {} (diff={})",
            envelope.total_pv,
            reference,
            diff,
        );
        assert_eq!(envelope.model, "discounting");
        assert_eq!(envelope.currency, Currency::USD);
        assert!(envelope.reconciles_with_base_value);
        assert!(!envelope.flows.is_empty());
        for row in &envelope.flows {
            assert!(row.survival_probability.is_none());
            assert!(row.discount_factor > 0.0);
        }
    }

    #[test]
    fn seasoned_instrument_discount_factors_use_correct_time_origin() {
        // Failure mode: `discount.df(year_fraction)` measures `year_fraction`
        // from `as_of`, but `DiscountCurve::df` expects time from the curve's
        // own `base_date`. When `as_of != base_date` (a seasoned instrument)
        // every exported discount factor and PV lands on the wrong time origin,
        // and `total_pv` no longer reconciles with `base_value`
        // (`Instrument::value`), which uses date-based `df_between_dates`.
        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let bond = Bond::fixed(
            "BOND-SEASONED-TIME-ORIGIN",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("bond");

        // as_of strictly after the curve base_date => seasoned instrument.
        let as_of_date = Date::from_calendar_date(2025, Month::July, 1).expect("date");

        // Curve base_date == issue, deliberately != as_of.
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .build()
            .expect("discount curve");
        let market = MarketContext::new().insert(disc.clone());

        // base_value: the canonical price from Instrument::value, which the
        // discounting BondEngine computes via df_between_dates(as_of, date).
        let base_value = bond
            .value(&market, as_of_date)
            .expect("bond base value")
            .amount();

        let json = serialize_bond(&bond);
        let payload = instrument_cashflows_json(&json, &market, "2025-07-01", "discounting")
            .expect("cashflows envelope");
        let envelope: InstrumentCashflowEnvelope =
            serde_json::from_str(&payload).expect("parse envelope");

        // total_pv must reconcile with base_value within rounding.
        let diff = (envelope.total_pv - base_value).abs();
        assert!(
            diff < 1e-2,
            "seasoned-instrument total_pv {} must reconcile with base_value {} (diff={}); \
             a wrong time origin in the exported discount factors breaks reconciliation",
            envelope.total_pv,
            base_value,
            diff,
        );

        // Each exported discount factor must equal df_between_dates(as_of, date).
        for row in &envelope.flows {
            if row.year_fraction < 0.0 {
                continue;
            }
            let expected_df = disc
                .df_between_dates(as_of_date, row.date)
                .expect("df_between_dates");
            assert!(
                (row.discount_factor - expected_df).abs() < 1e-12,
                "discount_factor for flow {} is {}, expected df_between_dates(as_of, date)={}",
                row.date,
                row.discount_factor,
                expected_df,
            );
        }

        // The reconciliation flag must be honest: it is only `true` here
        // because the export actually reconciles.
        assert!(envelope.reconciles_with_base_value);
    }

    #[test]
    fn rejects_unsupported_model_for_equity_option_style_instrument() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2026, Month::January, 15).expect("date");
        let bond = Bond::fixed(
            "BOND-BAD-MODEL",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("bond");
        let json = serialize_bond(&bond);
        let market = MarketContext::new();

        let err = instrument_cashflows_json(&json, &market, "2025-01-15", "monte_carlo_gbm")
            .expect_err("monte_carlo_gbm should reject bond");
        let msg = err.to_string();
        assert!(
            msg.contains("monte_carlo_gbm")
                || msg.contains("not priced")
                || msg.contains("supported"),
            "error should explain unsupported model: {msg}"
        );
    }

    #[test]
    fn total_pv_uses_compensated_summation_for_mixed_sign_flows() {
        let total = sum_pvs([1.0e16, 1.0, -1.0e16]);

        assert_eq!(total, 1.0);
    }

    #[test]
    fn row_pv_rejects_survival_probability_outside_unit_interval() {
        let date = Date::from_calendar_date(2026, Month::January, 15).expect("date");
        let flow = finstack_quant_cashflows::primitives::CashFlow::new(
            date,
            None,
            Money::new(100.0, Currency::USD),
            CFKind::Notional,
            0.0,
            None,
        );
        let err = credit_adjusted_cashflow_pv(&flow, 0.95, -0.01, Some(0.4), date)
            .expect_err("negative survival probability should be rejected");

        let msg = err.to_string();
        assert!(
            msg.contains("survival probability") && msg.contains("[0, 1]"),
            "error should explain invalid survival probability: {msg}"
        );
    }

    // NOTE: A hazard_rate reconciliation test would require an instrument that
    // (a) declares a credit curve in `curve_dependencies()` AND (b) has a
    // registered `HazardRate` pricer. The existing CDS and bond-with-hazard
    // pathways satisfy both via the instrument JSON layer, which is exercised
    // by the workspace-wide test suite (see `tests/instruments/cds/`). The
    // per-flow PV formula here is byte-identical to the one validated by
    // `period_pv.rs::test_periodized_pv_credit_adjusted_matches_detailed_engine`.
}
