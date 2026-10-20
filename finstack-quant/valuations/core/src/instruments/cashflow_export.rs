//! Per-flow cashflow export with discount-factor / survival-probability / PV enrichment.
//!
//! Provides the Rust cashflow envelope behind the umbrella crate's
//! `instrument_cashflows` entry points and the Python and WASM bindings. Produces a structured envelope for any instrument
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
//! accrual_factor, accrual_start, accrual_end, accrual_day_count,
//! accrual_notional, index_rate, principal_delta, year_fraction, rate,
//! reset_date, discount_factor, survival_probability,
//! conditional_default_prob, inflation_index_ratio, prepayment_smm,
//! beginning_balance, ending_balance, native_pv, fx_rate, pv`.
//!
//! The accrual columns carry the inputs of each coupon, so a row can be
//! recomputed as `accrual_notional × rate × accrual_factor` when the balance
//! is constant over its accrual period; `native_pv × fx_rate` (times one plus
//! the envelope's `scenario_price_shock_decimal`) gives `pv`.
//!
//! Hazard-only columns are populated when `model = "hazard_rate"`.
//! Inflation / MBS columns are populated from the instrument's
//! [`CashflowExportTerms`](crate::instruments::cashflow_export::CashflowExportTerms):
//! index-linked bonds supply index ratios and
//! prepaying pools supply their paydown states.
//!
//! **CMO tranche pool state** is intentionally exported as `null`: the waterfall
//! engine does not yet expose a stable per-tranche balance hook for this path.
//! Consumers should not treat `null` as missing data for non-CMO instruments.

use finstack_quant_cashflows::aggregation::{credit_adjusted_cashflow_pvs, DateContext};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use serde::Serialize;

use crate::instruments::Instrument;
use crate::pricer::{ModelKey, PricerKey};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use std::collections::HashMap;

// Envelope schema

/// Top-level cashflow envelope returned by [`export_cashflows`].
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct InstrumentCashflowEnvelope {
    /// Instrument identifier.
    pub instrument_id: String,
    /// Reporting currency used for row PVs and `total_pv`.
    pub currency: Currency,
    /// Model key used (`"discounting"` or `"hazard_rate"`).
    pub model: String,
    /// Valuation date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Discount curve ID used.
    pub discount_curve_id: CurveId,
    /// Hazard curve ID (`credit_curve_id`) used (omitted for `discounting` model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_curve_id: Option<CurveId>,
    /// Recovery rate from the hazard curve (omitted for `discounting` model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_rate: Option<f64>,
    /// Scenario price shock applied to every row `pv` and to `total_pv`, as a
    /// decimal (`-0.10` = multiplied by 0.90). Absent when rows are unshocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_price_shock_decimal: Option<f64>,
    /// Per-row enriched cashflows.
    pub flows: Vec<CashflowRow>,
    /// Sum of `flows[i].pv`. Matches `base_value` for supported products.
    pub total_pv: f64,
    /// `true` when `total_pv` agrees with the instrument's canonical
    /// `base_value` (`Instrument::value`) within rounding tolerance. The
    /// exporter returns an error instead of emitting a non-reconciling
    /// envelope, so every successful response carries `true`.
    pub reconciles_with_base_value: bool,
}

/// Single-row enriched cashflow view.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CashflowRow {
    /// Payment date.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Signed cashflow amount in row currency.
    pub amount: f64,
    /// Row currency (matters for instruments whose legs settle in several currencies).
    pub currency: Currency,
    /// `CFKind` discriminator (serde rename: `fixed`, `notional`, …).
    pub kind: CFKind,
    /// Accrual factor stored on the `CashFlow`.
    pub accrual_factor: f64,
    /// Contractual accrual-period start behind `accrual_factor`. Absent for
    /// flows that carry no accrual metadata (principal, fees, exchanges).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub accrual_start: Option<Date>,
    /// Contractual accrual-period end behind `accrual_factor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub accrual_end: Option<Date>,
    /// Day-count convention that turns the accrual period into `accrual_factor`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accrual_day_count: Option<finstack_quant_core::dates::DayCount>,
    /// Outstanding principal at `accrual_start`, in row currency: the balance
    /// the coupon accrues on when it is constant over the period. Absent when
    /// the flow has no accrual period or the schedule has no issue date to
    /// replay balances from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accrual_notional: Option<f64>,
    /// Index rate (annualized decimal) before spread, gearing, caps and
    /// floors, for floating coupons. `rate` is the all-in rate after them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_rate: Option<f64>,
    /// Change in outstanding principal carried by this flow, in row currency
    /// (positive increases the balance). Absent when the flow kind and amount
    /// alone determine the balance movement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_delta: Option<f64>,
    /// Year fraction from `as_of` to `date` under the discount curve's day count.
    pub year_fraction: f64,
    /// Projected / contractual rate when present (floats, real-coupon rates, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    /// Reset date when the flow is a floating-rate fixing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
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
    /// Inflation index ratio (populated for index-linked flows).
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
    /// Present value in row currency, before FX conversion and before any
    /// scenario price shock.
    pub native_pv: f64,
    /// FX rate (reporting currency per unit of row currency) applied to
    /// `native_pv`. Absent when the row is already in the reporting currency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_rate: Option<f64>,
    /// Per-flow present value in the envelope reporting currency, after FX
    /// conversion and any scenario price shock. Sums to `total_pv`.
    pub pv: f64,
}

/// Build the enriched cashflow envelope for a validated instrument.
///
/// This is the engine behind the umbrella crate's `instrument_cashflows` and
/// `instrument_cashflows_json` entry points. The instrument is priced once
/// through its family registry under `model` and its flows are reconciled to
/// that value.
///
/// # Arguments
///
/// * `instrument` - Validated instrument whose cashflows are projected and
///   reconciled to canonical pricing.
/// * `market` - Market context supplying discount, credit, index, and FX data
///   required to build and value the cashflow schedule.
/// * `as_of` - ISO-8601 valuation date used for schedule eligibility and
///   cashflow present values.
/// * `model` - Registered pricing-model name: `"discounting"` or
///   `"hazard_rate"` when that instrument/model pair is supported.
///
/// # Errors
///
/// Returns `Error::Validation` if the model is unsupported, the instrument and
/// model are not registered together, required market data is missing, or the
/// resulting cashflows cannot be reconciled.
pub fn export_cashflows(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<InstrumentCashflowEnvelope> {
    let model_key: ModelKey = model.parse().map_err(|e: String| {
        Error::Validation(format!(
            "unknown model '{model}': {e}. Supported: 'discounting', 'hazard_rate'"
        ))
    })?;
    ensure_decomposable(instrument, model_key)?;

    let requested_as_of = finstack_quant_core::dates::parse_iso_date(as_of)
        .map_err(|e| Error::Validation(format!("invalid as_of '{as_of}': {e}")))?;

    // --- Pricer registry gate: ensure the (type, model) pair is supported ---
    let registry = instrument.family().pricers();
    let instrument_type = instrument.key();
    let pricer_key = PricerKey::new(instrument_type, model_key);
    if registry.get_pricer(pricer_key).is_none() {
        return Err(Error::Validation(format!(
            "instrument type {instrument_type:?} is not priced under model '{model}' in instrument_cashflows; \
             this exporter supports only 'discounting' / 'hazard_rate' products where sum(pv) == base_value"
        )));
    }

    // Enter the selected canonical pricing lifecycle once. Besides proving the
    // requested model can actually price the instrument, this supplies the
    // effective valuation date and the scenario-adjusted value used for honest
    // reconciliation below.
    let canonical_result = registry.price_with_metrics(
        instrument,
        model_key,
        market,
        requested_as_of,
        &[],
        crate::instruments::PricingOptions::default().mark_instrument_validated(),
    )?;
    envelope_for_priced(instrument, market, model_key, &canonical_result)
}

/// Reject models and instruments whose value is not a sum of static cashflow PVs.
fn ensure_decomposable(instrument: &dyn Instrument, model_key: ModelKey) -> Result<()> {
    if !matches!(model_key, ModelKey::Discounting | ModelKey::HazardRate) {
        return Err(Error::Validation(format!(
            "model '{model_key}' not supported for instrument_cashflows; supported: 'discounting', 'hazard_rate'"
        )));
    }

    instrument.validate_cashflow_export(model_key)
}

/// Build the per-flow trace attached to a valuation result's `explanation`.
///
/// Uses the same rows as [`export_cashflows`], so the trace reconciles to
/// `priced.value`. When the model has no static per-flow decomposition the
/// trace holds one `computation_step` entry carrying the reason.
#[doc(hidden)]
pub fn pricing_trace(
    instrument: &dyn Instrument,
    market: &MarketContext,
    model_key: ModelKey,
    priced: &crate::results::ValuationResult,
    explain: finstack_quant_core::explain::ExplainOpts,
) -> finstack_quant_core::explain::ExplanationTrace {
    use finstack_quant_core::explain::{ExplanationTrace, TraceEntry};

    let mut trace = ExplanationTrace::new("pricing");
    let envelope = ensure_decomposable(instrument, model_key)
        .and_then(|()| envelope_for_priced(instrument, market, model_key, priced));
    match envelope {
        Ok(envelope) => {
            for row in &envelope.flows {
                trace.push(
                    TraceEntry::CashflowPV {
                        date: row.date,
                        cashflow_amount: row.amount,
                        cashflow_currency: row.currency.to_string(),
                        discount_factor: row.discount_factor,
                        pv_amount: row.pv,
                        pv_currency: envelope.currency.to_string(),
                        curve_id: row.discount_curve_id.to_string(),
                        survival_probability: row.survival_probability,
                    },
                    explain.max_entries,
                );
            }
        }
        Err(error) => trace.push(
            TraceEntry::ComputationStep {
                name: "cashflow_trace_unavailable".to_string(),
                description: error.to_string(),
                metadata: None,
            },
            explain.max_entries,
        ),
    }
    trace
}

/// Decompose an already priced discounting / hazard-rate result into per-flow rows.
///
/// `canonical_result` must come from the registered `model_key` pricer for
/// `instrument`; its `as_of` is the effective valuation date and its `value`
/// is the reconciliation target.
/// How the cashflow exporter decomposes one instrument into dated, discounted
/// flows.
///
/// [`Instrument::cashflow_export_terms`] returns these; the default exports the
/// instrument's own schedule on its first declared discount curve, and
/// instruments with several leg currencies, a projected (prepaying) schedule
/// or inflation-indexed flows fill the optional parts.
#[derive(Debug, Clone)]
pub struct CashflowExportTerms {
    /// Flows to export, in schedule order.
    pub schedule: CashFlowSchedule,
    /// Discount curve per flow currency when legs settle in several currencies;
    /// a currency not listed discounts on `discount_curve_id`.
    pub currency_discount_curves: HashMap<Currency, CurveId>,
    /// Curve discounting flows whose currency is not listed; `None` uses the
    /// instrument's first declared discount curve.
    pub discount_curve_id: Option<CurveId>,
    /// Currency the envelope reports in; `None` uses the first flow's currency.
    pub reporting_currency: Option<Currency>,
    /// Pool paydown state per payment date for prepaying pools.
    pub pool_states: Option<HashMap<Date, CashflowPoolState>>,
    /// Inflation index ratio per flow date for index-linked flows; a date
    /// without a ratio exports an empty cell.
    pub index_ratios: Option<HashMap<Date, f64>>,
}

impl CashflowExportTerms {
    /// Terms exporting `schedule` on the instrument's declared discount curve.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Flows to export, usually the instrument's own cashflow
    ///   schedule at the effective valuation date.
    #[must_use]
    pub fn from_schedule(schedule: CashFlowSchedule) -> Self {
        Self {
            schedule,
            currency_discount_curves: HashMap::new(),
            discount_curve_id: None,
            reporting_currency: None,
            pool_states: None,
            index_ratios: None,
        }
    }
}

/// Pool paydown state behind one exported flow of a prepaying pool.
#[derive(Debug, Clone, Copy)]
pub struct CashflowPoolState {
    /// Single monthly mortality applied in the period, as a decimal fraction.
    pub smm: f64,
    /// Pool balance at the start of the period.
    pub beginning_balance: f64,
    /// Pool balance at the end of the period.
    pub ending_balance: f64,
}

fn envelope_for_priced(
    instrument: &dyn Instrument,
    market: &MarketContext,
    model_key: ModelKey,
    canonical_result: &crate::results::ValuationResult,
) -> Result<InstrumentCashflowEnvelope> {
    let instrument_id = instrument.id().to_string();
    let as_of_date = canonical_result.as_of;

    let deps = instrument.market_dependencies()?;
    let curves = &deps.curves;
    let default_discount_curve_id = curves.discount_curves.first().cloned().ok_or_else(|| {
        Error::Validation(
            "instrument has no declared discount curve; cannot compute cashflow DFs".into(),
        )
    })?;
    let terms = instrument.cashflow_export_terms(market, as_of_date)?;
    let currency_discount_curves = terms.currency_discount_curves;
    let discount_curve_id = terms.discount_curve_id.unwrap_or(default_discount_curve_id);
    let reporting_currency = terms.reporting_currency;
    let primary_discount = market.get_discount(discount_curve_id.as_str())?;

    let (credit_curve_id, hazard_arc) = if matches!(model_key, ModelKey::HazardRate) {
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
    let schedule = terms.schedule;
    let mbs_state = terms.pool_states;
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
                    credit_curve_id
                        .as_ref()
                        .map(|id| id.as_str())
                        .unwrap_or("<unknown>")
                )));
            }
            Some(s0)
        }
        None => None,
    };

    let mut rows = Vec::with_capacity(schedule.get_flows().len());
    let mut envelope_currency = reporting_currency;
    let mut prev_sp = 1.0_f64;
    let row_discounts: Vec<_> = schedule
        .get_flows()
        .iter()
        .map(|flow| {
            let id = currency_discount_curves
                .get(&flow.amount.currency())
                .unwrap_or(&discount_curve_id);
            if id == &discount_curve_id {
                Ok(std::sync::Arc::clone(&primary_discount))
            } else {
                market.get_discount(id.as_str())
            }
        })
        .collect::<Result<_>>()?;
    let discount_factors: Vec<_> = schedule
        .get_flows()
        .iter()
        .zip(&row_discounts)
        .map(|(flow, discount)| {
            if flow.date <= as_of_date {
                Ok(1.0)
            } else {
                discount.df_between_dates(as_of_date, flow.date)
            }
        })
        .collect::<Result<_>>()?;
    let native_pvs = credit_adjusted_cashflow_pvs(
        schedule.get_flows(),
        &discount_factors,
        hazard_arc.as_deref(),
        recovery_rate,
        DateContext::new(as_of_date, primary_discount.day_count(), dc_ctx),
    )?;

    let accrual_notionals = accrual_start_balances(&schedule);

    for (row_index, flow) in schedule.get_flows().iter().enumerate() {
        let ccy = flow.amount.currency();
        if envelope_currency.is_none() {
            envelope_currency = Some(ccy);
        }

        let row_discount_curve_id = currency_discount_curves
            .get(&ccy)
            .unwrap_or(&discount_curve_id);
        let row_discount = &row_discounts[row_index];
        let curve_day_count = row_discount.day_count();
        let year_fraction = curve_day_count.signed_year_fraction(as_of_date, flow.date, dc_ctx)?;

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
            let df = discount_factors[row_index];
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

        let native_pv = native_pvs[row_index];
        let row_reporting_currency = envelope_currency.unwrap_or(ccy);
        let base_pv = market
            .convert_money(
                Money::new(native_pv, ccy)?,
                row_reporting_currency,
                as_of_date,
            )?
            .amount
            .amount();
        let fx_rate = if row_reporting_currency == ccy {
            None
        } else {
            Some(
                market
                    .fx_required()?
                    .rate(finstack_quant_core::money::fx::FxQuery::new(
                        ccy,
                        row_reporting_currency,
                        as_of_date,
                    ))?
                    .rate,
            )
        };
        let pv =
            crate::instruments::common_impl::helpers::apply_scenario_raw_value(instrument, base_pv);

        let mbs_row = mbs_state.as_ref().and_then(|m| m.get(&flow.date));

        // A missing inflation index ratio is indistinguishable from a data
        // outage when it lands in the export as a blank cell, so log loudly
        // when the lookup fails on an instrument that carries an inflation
        // link. (Instruments with no inflation bond keep the column empty by
        // design.)
        let inflation_index_ratio = terms
            .index_ratios
            .as_ref()
            .and_then(|ratios| ratios.get(&flow.date).copied());

        rows.push(CashflowRow {
            date: flow.date,
            amount: flow.amount.amount(),
            currency: ccy,
            kind: flow.kind,
            accrual_factor: flow.accrual_factor,
            accrual_start: flow.accrual.as_ref().map(|accrual| accrual.start),
            accrual_end: flow.accrual.as_ref().map(|accrual| accrual.end),
            accrual_day_count: flow.accrual.as_ref().map(|accrual| accrual.day_count),
            accrual_notional: accrual_notionals
                .as_ref()
                .and_then(|balances| balances[row_index]),
            index_rate: flow
                .accrual
                .as_ref()
                .and_then(|accrual| accrual.projected_index_rate),
            principal_delta: flow.principal_delta.map(|delta| delta.amount()),
            year_fraction,
            rate: flow.rate,
            reset_date: flow.reset_date,
            discount_factor,
            discount_curve_id: row_discount_curve_id.clone(),
            survival_probability,
            conditional_default_prob,
            inflation_index_ratio,
            prepayment_smm: mbs_row.map(|s| s.smm),
            beginning_balance: mbs_row.map(|s| s.beginning_balance),
            ending_balance: mbs_row.map(|s| s.ending_balance),
            native_pv,
            fx_rate,
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

    // Compare against the same selected registry result that supplied the
    // effective date. Per-flow and model PVs use different compensated sums,
    // so allow a small numerical tolerance while still catching real drift.
    if canonical_result.value.currency() != currency {
        return Err(Error::Validation(format!(
            "instrument_cashflows: exported currency {currency} does not match canonical price currency {} for instrument '{instrument_id}'",
            canonical_result.value.currency()
        )));
    }
    let base = canonical_result.value.amount();
    let tol = (base.abs() * 1e-6).max(1e-6);
    if (total_pv - base).abs() > tol {
        return Err(Error::Validation(format!(
            "instrument_cashflows: per-flow PV {total_pv} does not reconcile with canonical {model_key} price {base} for instrument '{instrument_id}' (tolerance {tol}); this instrument/model requires a model-specific cashflow decomposition"
        )));
    }

    Ok(InstrumentCashflowEnvelope {
        instrument_id,
        currency,
        model: model_key.to_string(),
        as_of: as_of_date,
        discount_curve_id,
        credit_curve_id,
        recovery_rate,
        scenario_price_shock_decimal: instrument
            .get_scenario_pricing_overrides()
            .and_then(|overrides| overrides.scenario_price_shock_decimal),
        flows: rows,
        total_pv,
        reconciles_with_base_value: true,
    })
}

/// Outstanding principal at each flow's accrual start, row-aligned.
///
/// Returns `None` when the schedule cannot replay balances (no issue-date
/// metadata); rows without an accrual period map to `None`.
fn accrual_start_balances(
    schedule: &finstack_quant_cashflows::builder::CashFlowSchedule,
) -> Option<Vec<Option<f64>>> {
    let flows = schedule.get_flows();
    let mut starts: Vec<Date> = flows
        .iter()
        .filter_map(|flow| flow.accrual.as_ref().map(|accrual| accrual.start))
        .collect();
    starts.sort_unstable();
    starts.dedup();
    let balances = schedule.outstanding_at_dates(&starts).ok()?;
    Some(
        flows
            .iter()
            .map(|flow| {
                let start = flow.accrual.as_ref()?.start;
                let index = starts.binary_search(&start).ok()?;
                Some(balances[index].amount())
            })
            .collect(),
    )
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
    use super::sum_pvs;

    #[test]
    fn total_pv_uses_compensated_summation_for_mixed_sign_flows() {
        let total = sum_pvs([1.0e16, 1.0, -1.0e16]);

        assert_eq!(total, 1.0);
    }
}
