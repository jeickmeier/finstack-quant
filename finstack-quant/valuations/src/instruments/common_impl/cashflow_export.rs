//! Per-flow cashflow export with discount-factor / survival-probability / PV enrichment.
//!
//! Provides the Rust cashflow envelope behind the `instrument_cashflows`
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
//! Inflation / MBS columns are populated by concrete-type downcasts when the
//! instrument is `InflationLinkedBond` / `AgencyMbsPassthrough`.
//!
//! **CMO tranche pool state** is intentionally exported as `null`: the waterfall
//! engine does not yet expose a stable per-tranche balance hook for this path.
//! Consumers should not treat `null` as missing data for non-CMO instruments.

use finstack_quant_cashflows::aggregation::{credit_adjusted_cashflow_pvs, DateContext};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use serde::Serialize;

use crate::instruments::fixed_income::bond::Bond;
use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::instruments::fixed_income::mbs_passthrough::{
    pricer::project_cashflows as project_mbs_cashflows, AgencyMbsPassthrough,
};
use crate::instruments::fx::fx_swap::FxSwap;
use crate::instruments::rates::xccy_swap::XccySwap;
use crate::instruments::Instrument;
use crate::pricer::{shared_standard_registry, ModelKey, ParsedInstrument, PricerKey};

// Envelope schema

/// Top-level JSON envelope returned by [`instrument_cashflows_json`].
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
    /// Row currency (matters for `XccySwap` / `FxSwap`).
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

// Public entry point

/// Build the enriched cashflow envelope for a tagged instrument and serialize to JSON.
///
/// # Errors
///
/// Returns `Error::Validation` if the model string is not one of
/// `{"discounting", "hazard_rate"}`, if the `(instrument_type, model)` pair is
/// not in the standard pricer registry, if required curves are missing from
/// the market, or if the schedule mixes currencies.
///
/// # Arguments
///
/// * `instrument_json` - UTF-8 canonical v1 instrument envelope.
/// * `market` - Market context supplying discount, credit, index, and FX data
///   required to build and value the cashflow schedule.
/// * `as_of` - ISO-8601 valuation date used for schedule eligibility and
///   cashflow present values.
/// * `model` - Registered pricing-model name: `"discounting"` or
///   `"hazard_rate"` when that instrument/model pair is supported.
pub fn instrument_cashflows_json(
    instrument_json: &str,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<String> {
    let instrument = crate::pricer::parse_boxed_instrument_from_json(instrument_json, None)?;
    let envelope = instrument_cashflows(&instrument, market, as_of, model)?;
    serde_json::to_string(&envelope)
        .map_err(|e| Error::Validation(format!("failed to serialize cashflow envelope: {e}")))
}

/// Build the enriched cashflow envelope for an already parsed and validated
/// instrument.
///
/// This is the canonical core behind [`instrument_cashflows_json`]. Host
/// bindings use it after parsing the instrument so validation precedence does
/// not require a second deserialization.
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
pub fn instrument_cashflows(
    instrument: &ParsedInstrument,
    market: &MarketContext,
    as_of: &str,
    model: &str,
) -> Result<InstrumentCashflowEnvelope> {
    build_envelope(instrument.as_instrument(), market, as_of, model)
}

fn build_envelope(
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
    let registry = shared_standard_registry();
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

    if let Some(bond) = instrument.as_any().downcast_ref::<Bond>() {
        let has_embedded_options = bond
            .call_put
            .as_ref()
            .is_some_and(crate::instruments::fixed_income::bond::CallPutSchedule::has_options)
            || bond.return_floor.is_some();
        if has_embedded_options {
            return Err(Error::Validation(format!(
                "instrument_cashflows: static cashflow rows cannot decompose the \
                 exercise-contingent value of bond '{}' under model '{}'; request the model \
                 price directly",
                bond.id(),
                model_key.as_str()
            )));
        }
    }
    Ok(())
}

/// Build the per-flow trace attached to a valuation result's `explanation`.
///
/// Uses the same rows as [`instrument_cashflows`], so the trace reconciles to
/// `priced.value`. When the model has no static per-flow decomposition the
/// trace holds one `computation_step` entry carrying the reason.
pub(crate) fn pricing_trace(
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
        currency_discount_curves.insert(
            swap.leg1.notional.currency(),
            swap.leg1.leg.discount_curve_id.clone(),
        );
        currency_discount_curves.insert(
            swap.leg2.notional.currency(),
            swap.leg2.leg.discount_curve_id.clone(),
        );
        let primary = currency_discount_curves
            .get(&swap.reporting_currency)
            .cloned()
            .unwrap_or(default_discount_curve_id);
        (primary, Some(swap.reporting_currency))
    } else {
        (default_discount_curve_id, None)
    };
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
    let (schedule, mbs_state) =
        if let Some(mbs) = instrument.as_any().downcast_ref::<AgencyMbsPassthrough>() {
            let projection = project_mbs_cashflows(mbs, as_of_date, Some(mbs.wam_months + 12))?;
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
        hazard_arc
            .as_deref()
            .map(|hazard| hazard as &dyn finstack_quant_core::market_data::traits::Survival),
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
        let inflation_index_ratio =
            match inflation_bond.map(|b| b.index_ratio_from_market(flow.date, market)) {
                Some(Ok(ratio)) => Some(ratio),
                Some(Err(err)) => {
                    tracing::warn!(
                        instrument_id = %instrument_id,
                        date = %flow.date,
                        %err,
                        "cashflow export: inflation index ratio lookup failed; \
                         exporting an empty index_ratio cell"
                    );
                    None
                }
                None => None,
            };

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
    use crate::instruments::fixed_income::loan_terms::RateSpec;
    use crate::instruments::fixed_income::revolving_credit::{
        DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
    };
    use crate::instruments::fixed_income::structured_credit::StructuredCredit;
    use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
    use crate::instruments::{Instrument, ScenarioPricingOverrides};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    fn serialize_bond(bond: &Bond) -> String {
        let envelope = InstrumentEnvelope {
            schema: crate::instruments::json_loader::InstrumentSchema::CURRENT,
            instrument: InstrumentJson::Bond(bond.clone()),
        };
        serde_json::to_string(&envelope).expect("serialize bond envelope")
    }

    #[test]
    fn cashflow_export_rejects_exercise_contingent_bond_rows() {
        let bond = Bond::example_callable().expect("callable example");
        for model in ["discounting", "hazard_rate"] {
            let err = instrument_cashflows_json(
                &serialize_bond(&bond),
                &MarketContext::new(),
                "2025-01-01",
                model,
            )
            .expect_err("static rows cannot represent exercise-contingent value");

            assert!(
                err.to_string().contains("exercise-contingent value"),
                "unexpected error for {model}: {err}"
            );
        }
    }

    #[test]
    fn revolving_credit_credit_risk_export_fails_instead_of_returning_false_reconciliation() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let maturity = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let facility = RevolvingCredit::builder()
            .id("RC-CASHFLOW-CREDIT".into())
            .commitment(Money::from((1_000_000_i64, Currency::USD)))
            .drawn(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(as_of)
            .maturity(maturity)
            .rate(RateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .credit_curve_id("USD-HZ".into())
            .recovery_rate(0.4)
            .build()
            .expect("facility");
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("discount curve"),
            )
            .insert(
                HazardCurve::builder("USD-HZ")
                    .base_date(as_of)
                    .recovery_rate(0.4)
                    .knots([(1.0, 0.20), (5.0, 0.20)])
                    .build()
                    .expect("hazard curve"),
            );
        let json = serde_json::to_string(&InstrumentEnvelope::new(
            InstrumentJson::RevolvingCredit(facility),
        ))
        .expect("serialize");

        let err = instrument_cashflows_json(&json, &market, "2025-01-01", "discounting")
            .expect_err("generic rows cannot represent the recovery leg");
        assert!(
            err.to_string()
                .contains("model-specific cashflow decomposition"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn malformed_instrument_precedes_model_and_date_errors() {
        let mut deal = StructuredCredit::example().expect("example");
        deal.cleanup_call_decimal = Some(-0.5);
        let json = serde_json::to_string(&InstrumentEnvelope::new(
            InstrumentJson::StructuredCredit(Box::new(deal)),
        ))
        .expect("serialize structured credit");

        let err =
            instrument_cashflows_json(&json, &MarketContext::new(), "not-a-date", "not-a-model")
                .expect_err("instrument validation must win");

        assert!(
            err.to_string().contains("cleanup_call_decimal"),
            "unexpected error ordering: {err}"
        );
    }

    #[test]
    fn mixed_currency_fx_swap_rows_use_native_curves_and_reporting_currency_pv() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("date");
        let swap = FxSwap::example().expect("example");
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
            schema: crate::instruments::json_loader::InstrumentSchema::CURRENT,
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
        for row in &envelope.flows {
            if row.currency == Currency::EUR {
                assert_eq!(row.fx_rate, Some(1.10));
                assert!((row.native_pv * 1.10 - row.pv).abs() < 0.01);
            } else {
                assert!(row.fx_rate.is_none());
            }
        }
    }

    #[test]
    fn discounting_reconciles_with_schedule_pv_for_fixed_bond() {
        use crate::instruments::common_impl::helpers::schedule_pv_raw;

        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let bond = Bond::fixed(
            "BOND-DISC-RECONCILE",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
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

        // The raw and Money paths share the core relative-discounting kernel;
        // the raw path is used here to avoid Money rounding in reconciliation.
        let discount_curve_id = bond
            .market_dependencies()
            .expect("deps")
            .curves
            .discount_curves
            .first()
            .cloned()
            .expect("bond should declare a discount curve");
        let reference =
            schedule_pv_raw(&bond, &market, as_of_date, &discount_curve_id).expect("schedule pv");

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
            assert!(row.fx_rate.is_none());
            if row.date > as_of_date {
                assert!((row.pv - row.native_pv).abs() < 1e-9);
            }
        }
        assert!(envelope.scenario_price_shock_decimal.is_none());

        // Every coupon row carries the inputs it was computed from.
        let coupons: Vec<_> = envelope
            .flows
            .iter()
            .filter(|row| row.kind == CFKind::Fixed)
            .collect();
        assert!(!coupons.is_empty());
        for row in coupons {
            let start = row.accrual_start.expect("accrual start");
            let end = row.accrual_end.expect("accrual end");
            assert!(start < end && end <= row.date);
            assert!(row.accrual_day_count.is_some());
            let notional = row.accrual_notional.expect("accrual notional");
            let rate = row.rate.expect("coupon rate");
            assert!((notional - 1_000_000.0).abs() < 1e-9);
            assert!((notional * rate * row.accrual_factor - row.amount).abs() < 1e-6);
        }
    }

    #[test]
    fn explained_price_carries_reconciling_trace_and_provenance() {
        use crate::instruments::PricingOptions;
        use crate::metrics::MetricId;
        use finstack_quant_core::explain::{ExplainOpts, TraceEntry};

        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let mut bond = Bond::fixed(
            "BOND-EXPLAIN",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        bond.scenario_pricing_overrides =
            ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.10);
        bond.metric_pricing_overrides.bump_config.rate_bump_bp = Some(5.0);
        let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("date");
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(issue)
                .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
                .build()
                .expect("discount curve"),
        );

        let plain = bond
            .price_with_metrics(&market, as_of, &[], PricingOptions::default())
            .expect("plain price");
        assert!(plain.explanation.is_none());
        let provenance = plain.provenance.expect("registry stamps provenance");
        assert_eq!(provenance.model, ModelKey::Discounting);
        assert_eq!(provenance.requested_as_of, as_of);
        assert_eq!(
            provenance.market_dependencies,
            bond.market_dependencies().expect("deps")
        );
        assert_eq!(provenance.scenario_price_shock_decimal, Some(-0.10));
        assert!(provenance.sensitivity_bumps.is_none());

        let explained = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Dv01],
                PricingOptions::default().with_explain(ExplainOpts::enabled()),
            )
            .expect("explained price");
        assert_eq!(explained.value, plain.value);
        let bumps = explained
            .provenance
            .as_ref()
            .and_then(|p| p.sensitivity_bumps.as_ref())
            .expect("metric request stamps bumps");
        assert_eq!(bumps.rate_bump_bp, 5.0);
        assert_eq!(bumps.vol_bump_decimal, 0.01);

        let trace = explained.explanation.expect("trace");
        assert_eq!(trace.trace_type, "pricing");
        let mut total = 0.0;
        for entry in &trace.entries {
            let TraceEntry::CashflowPV {
                pv_amount,
                curve_id,
                survival_probability,
                ..
            } = entry
            else {
                panic!("unexpected entry {entry:?}");
            };
            assert_eq!(curve_id, "USD-OIS");
            assert!(survival_probability.is_none());
            total += pv_amount;
        }
        assert!(!trace.entries.is_empty());
        assert!((total - explained.value.amount()).abs() < 1e-2);
    }

    #[test]
    fn trace_names_the_reason_when_no_decomposition_exists() {
        use finstack_quant_core::explain::{ExplainOpts, TraceEntry};

        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let bond = Bond::fixed(
            "BOND-TREE",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let priced = crate::results::ValuationResult::stamped(
            "BOND-TREE",
            issue,
            Money::from((1_000_000_i64, Currency::USD)),
        );

        let trace = pricing_trace(
            &bond,
            &MarketContext::new(),
            ModelKey::Tree,
            &priced,
            ExplainOpts::enabled(),
        );

        assert!(matches!(
            trace.entries.as_slice(),
            [TraceEntry::ComputationStep { name, description, .. }]
                if name == "cashflow_trace_unavailable" && description.contains("tree")
        ));
    }

    #[test]
    fn scenario_price_shock_scales_cashflow_rows_and_total_once() {
        let issue = Date::from_calendar_date(2025, Month::January, 15).expect("date");
        let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("date");
        let mut bond = Bond::fixed(
            "BOND-CASHFLOW-SCENARIO",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond");
        let as_of = Date::from_calendar_date(2025, Month::July, 1).expect("date");
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(issue)
                .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
                .build()
                .expect("discount curve"),
        );

        let baseline: InstrumentCashflowEnvelope = serde_json::from_str(
            &instrument_cashflows_json(
                &serialize_bond(&bond),
                &market,
                "2025-07-01",
                "discounting",
            )
            .expect("baseline cashflows"),
        )
        .expect("baseline envelope");

        bond.scenario_pricing_overrides =
            ScenarioPricingOverrides::default().with_scenario_price_shock_decimal(-0.10);
        let shocked: InstrumentCashflowEnvelope = serde_json::from_str(
            &instrument_cashflows_json(
                &serialize_bond(&bond),
                &market,
                "2025-07-01",
                "discounting",
            )
            .expect("shocked cashflows"),
        )
        .expect("shocked envelope");

        assert_eq!(shocked.as_of, as_of);
        assert_eq!(shocked.scenario_price_shock_decimal, Some(-0.10));
        assert!((shocked.total_pv - baseline.total_pv * 0.90).abs() < 1e-8);
        assert!(shocked.reconciles_with_base_value);
        for (baseline_row, shocked_row) in baseline.flows.iter().zip(&shocked.flows) {
            assert!((shocked_row.pv - baseline_row.pv * 0.90).abs() < 1e-8);
            assert_eq!(shocked_row.native_pv, baseline_row.native_pv);
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
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
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
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            issue,
            maturity,
            finstack_quant_core::dates::StubKind::ShortFront,
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

    // NOTE: A hazard_rate reconciliation test would require an instrument that
    // (a) declares a canonical credit-curve dependency AND (b) has a
    // registered `HazardRate` pricer. The existing CDS and bond-with-hazard
    // pathways satisfy both via the instrument JSON layer, which is exercised
    // by the workspace-wide test suite (see `tests/instruments/cds/`). The
    // per-flow PV formula here is byte-identical to the one validated by
    // `period_pv.rs::test_periodized_pv_credit_adjusted_matches_detailed_engine`.
}
