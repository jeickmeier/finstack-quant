//! Endpoint valuation, P&L differencing, execution-policy mapping, and finalization.

use crate::AttributionRequest;
use crate::factors::MarketRestoreFlags;
use crate::model_params::{ModelParamsSnapshot, instrument_with_model_params};
use crate::{
    AttributionFactor, AttributionMethod, ExecutionPolicy, MoveUnit, PnlAttribution,
    SensitivityBucket, SensitivityStep,
};
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxPolicyMeta};
use finstack_quant_valuations::instruments::{Instrument, MarketDependencies};
use std::sync::Arc;

/// Resolve the same contract expiry on the two observed surface clocks.
/// Numeric-only grids use a shared ACT/365F opening tenor so that an
/// unchanged grid cannot contribute a market move through passage of time.
///
/// # Arguments
///
/// * `opening` - Surface observed at the opening attribution endpoint.
/// * `closing` - Surface observed at the closing attribution endpoint.
/// * `expiry` - Instrument's contractual option expiry date.
/// * `as_of_t0` - Opening valuation date for numeric-only surfaces.
pub(crate) fn volatility_expiry_times(
    opening: &VolSurface,
    closing: &VolSurface,
    expiry: Date,
    as_of_t0: Date,
) -> Result<(f64, f64)> {
    let resolve = |surface: &VolSurface| match surface.expiry_clock() {
        Some(clock) => clock.expiry_time(expiry),
        None => Ok((expiry - as_of_t0).whole_days() as f64 / 365.0),
    };
    Ok((resolve(opening)?, resolve(closing)?))
}

/// Families the instrument declares a pricing dependency on.
///
/// [`MarketDependencies`] has no correlation field, so correlation restores
/// stay snapshot-gated. A failed dependency lookup fails open (treat every
/// family as used) so a missing declaration cannot drop a live factor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct InstrumentFactorUse(MarketRestoreFlags);

impl InstrumentFactorUse {
    pub(crate) fn of(instrument: &dyn Instrument) -> Self {
        match instrument.market_dependencies() {
            Ok(deps) => Self::from_deps(&deps, instrument),
            Err(_) => Self(MarketRestoreFlags::all()),
        }
    }

    pub(super) fn from_deps(deps: &MarketDependencies, instrument: &dyn Instrument) -> Self {
        let families = [
            (
                !deps.curves.discount_curves.is_empty() || !deps.curves.forward_curves.is_empty(),
                MarketRestoreFlags::RATES,
            ),
            (
                !deps.curves.credit_curves.is_empty(),
                MarketRestoreFlags::HAZARD,
            ),
            (
                !deps.curves.inflation_curves.is_empty(),
                MarketRestoreFlags::INFLATION,
            ),
            (
                !deps.fx_pairs.is_empty() || instrument.fx_exposure().is_some(),
                MarketRestoreFlags::FX,
            ),
            (
                !deps.volatility_dependencies.is_empty(),
                MarketRestoreFlags::VOL,
            ),
            (
                !deps.market_scalar_ids.is_empty(),
                MarketRestoreFlags::SCALARS,
            ),
        ];
        // An empty dependency set is "undeclared", not "uses nothing". Test
        // stubs and some custom instruments omit the declaration; fail open
        // so a live FX/scalar/credit factor is not dropped into residual.
        if families.iter().all(|(used, _)| !used) {
            return Self(MarketRestoreFlags::all());
        }
        // Correlations are not declared as dependencies, so they always count.
        Self(
            families
                .into_iter()
                .filter(|(used, _)| *used)
                .fold(MarketRestoreFlags::CORRELATION, |all, (_, family)| {
                    all | family
                }),
        )
    }

    pub(crate) fn uses_attribution_factor(self, factor: &AttributionFactor) -> bool {
        MarketRestoreFlags::for_factor(factor).is_none_or(|family| self.0.contains(family))
    }
}

/// Map `items` with the infallible `f`; see [`try_map_policy`].
pub(crate) fn map_policy<T, R, F>(policy: ExecutionPolicy, items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync + Send,
{
    let Ok(mapped) = try_map_policy(policy, items, |item| {
        Ok::<R, std::convert::Infallible>(f(item))
    });
    mapped
}

/// Fallible map of `items` with `f`, using Rayon only on native `Parallel`.
///
/// Both policies report the error of the lowest-index failing item, so a
/// parallel run names the same failure as a serial (or wasm32) run.
pub(crate) fn try_map_policy<T, R, E, F>(
    policy: ExecutionPolicy,
    items: &[T],
    f: F,
) -> std::result::Result<Vec<R>, E>
where
    T: Sync,
    R: Send,
    E: Send,
    F: Fn(&T) -> std::result::Result<R, E> + Sync + Send,
{
    match policy {
        ExecutionPolicy::Parallel => finstack_quant_core::parallel::try_map_ordered(items, f),
        ExecutionPolicy::Serial => items.iter().map(f).collect(),
    }
}

/// Keep a curve-movement measurement, treating a curve absent from either
/// market as unmeasured.
///
/// A missing curve cannot move, so it is skipped; any other measurement
/// failure (a non-finite curve sample, an empty tenor grid) is returned so the
/// attribution fails rather than silently averaging over fewer curves.
pub(crate) fn measured_unless_missing(
    measurement: finstack_quant_core::Result<f64>,
) -> finstack_quant_core::Result<Option<f64>> {
    use finstack_quant_core::InputError;
    match measurement {
        Ok(shift) => Ok(Some(shift)),
        Err(finstack_quant_core::Error::Input(
            InputError::MissingCurve { .. } | InputError::NotFound { .. },
        )) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Compute P&L between two valuations in target currency.
///
/// Converts both valuations to target currency before computing difference.
///
/// # Arguments
///
/// * `val_t0` - Opening mark-to-market amount, converted with the T₁ FX matrix
///   under this simple non-isolated convention.
/// * `val_t1` - Closing mark-to-market amount to compare with `val_t0`.
/// * `target_currency` - Currency in which the returned P&L is reported.
/// * `market_t1` - Closing market context supplying the FX matrix used to
///   convert both marks.
/// * `as_of_t1` - Closing valuation date supplied to the T₁ FX conversion.
///
/// # Returns
///
/// P&L in target currency (val_t1 - val_t0).
///
/// # Errors
///
/// Returns error if currency conversion fails.
pub(crate) fn compute_pnl(
    val_t0: Money,
    val_t1: Money,
    target_currency: Currency,
    market_t1: &MarketContext,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_t1
        .convert_money(
            val_t0,
            target_currency,
            as_of_t1,
            FxConversionPolicy::CashflowDate,
        )?
        .amount;
    let val_t1_converted = market_t1
        .convert_money(
            val_t1,
            target_currency,
            as_of_t1,
            FxConversionPolicy::CashflowDate,
        )?
        .amount;

    val_t1_converted.checked_sub(val_t0_converted)
}

/// Compute P&L with explicit FX conversion for each date.
///
/// This allows proper isolation of FX translation effects by using
/// date-appropriate FX rates for conversion.
///
/// # Arguments
///
/// * `val_t0` - Opening mark-to-market amount, converted with T₀ FX.
/// * `val_t1` - Closing mark-to-market amount, converted with T₁ FX.
/// * `target_currency` - Currency in which the returned P&L is reported.
/// * `market_fx_t0` - Opening market context supplying the FX matrix for the
///   opening-value translation.
/// * `market_fx_t1` - Closing market context supplying the FX matrix for the
///   closing-value translation.
/// * `as_of_t0` - Opening valuation date supplied to the T₀ FX conversion.
/// * `as_of_t1` - Closing valuation date supplied to the T₁ FX conversion.
///
/// # Returns
///
/// P&L in target currency with FX translation properly isolated.
///
/// # Errors
///
/// Returns error if currency conversion fails.
pub(crate) fn compute_pnl_with_fx(
    val_t0: Money,
    val_t1: Money,
    target_currency: Currency,
    market_fx_t0: &MarketContext,
    market_fx_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_fx_t0
        .convert_money(
            val_t0,
            target_currency,
            as_of_t0,
            FxConversionPolicy::CashflowDate,
        )?
        .amount;
    let val_t1_converted = market_fx_t1
        .convert_money(
            val_t1,
            target_currency,
            as_of_t1,
            FxConversionPolicy::CashflowDate,
        )?
        .amount;

    val_t1_converted.checked_sub(val_t0_converted)
}

/// The instrument carrying the opening model parameters.
///
/// `None`, or a snapshot without parameters, reuses `instrument` itself;
/// otherwise a clone is built with the snapshot applied.
///
/// # Errors
///
/// Returns an error when the snapshot belongs to a different instrument type.
pub(crate) fn instrument_at_t0(
    instrument: &Arc<dyn Instrument>,
    model_params_t0: Option<&ModelParamsSnapshot>,
) -> Result<Arc<dyn Instrument>> {
    match model_params_t0 {
        None | Some(ModelParamsSnapshot::None) => Ok(Arc::clone(instrument)),
        Some(params) => instrument_with_model_params(instrument, params),
    }
}

/// Price both endpoints of a repricing request.
///
/// Returns the T₀-parameter instrument with the unscaled `(T₀, T₁)` values.
/// Prepared endpoints on the request replace the two valuations.
pub(crate) fn endpoint_values(
    request: &AttributionRequest<'_>,
) -> Result<(Arc<dyn Instrument>, Money, Money)> {
    let instrument_t0 = instrument_at_t0(request.instrument, request.model_params_t0)?;
    let (val_t0, val_t1) = match request.prepared_endpoints {
        Some(endpoints) => endpoints,
        None => (
            instrument_t0.value(request.market_t0, request.as_of_t0)?,
            request
                .instrument
                .value(request.market_t1, request.as_of_t1)?,
        ),
    };
    Ok((instrument_t0, val_t0, val_t1))
}

/// Seed a repricing attribution: total P&L in the T₁ value's currency, the
/// request's rounding context, and the execution policy the run used.
pub(crate) fn seed_attribution(
    request: &AttributionRequest<'_>,
    val_t0: Money,
    val_t1: Money,
    method: AttributionMethod,
    execution_policy: ExecutionPolicy,
) -> Result<PnlAttribution> {
    let total_pnl = compute_pnl_with_fx(
        val_t0,
        val_t1,
        val_t1.currency(),
        request.market_t0,
        request.market_t1,
        request.as_of_t0,
        request.as_of_t1,
    )?;
    let mut attribution = PnlAttribution::new(
        total_pnl,
        request.instrument.id(),
        request.as_of_t0,
        request.as_of_t1,
        method,
    );
    attribution.meta.rounding = finstack_quant_core::config::RoundingContext::from(request.config);
    stamp_endpoints(&mut attribution, val_t0, val_t1);
    // Policy-visibility invariant: results carry the parallel flag.
    attribution.meta.execution_policy = Some(execution_policy);
    Ok(attribution)
}

/// Record the endpoint present values the attribution was computed from.
///
/// Both are stored only when they are quoted in the `total_pnl` currency, so
/// `mark_to_market_pnl == pv_t1 − pv_t0` holds without an FX conversion.
pub(crate) fn stamp_endpoints(attribution: &mut PnlAttribution, val_t0: Money, val_t1: Money) {
    let currency = attribution.total_pnl.currency();
    if val_t0.currency() == currency && val_t1.currency() == currency {
        attribution.pv_t0 = Some(val_t0);
        attribution.pv_t1 = Some(val_t1);
    }
}

pub(crate) fn stamp_fx_policy(
    attribution: &mut PnlAttribution,
    target_currency: Currency,
    notes: impl Into<String>,
) {
    attribution.meta.fx_policy = Some(FxPolicyMeta {
        strategy: FxConversionPolicy::CashflowDate,
        target_currency: Some(target_currency),
        notes: notes.into(),
    });
}

pub(crate) fn finalize_attribution(
    attribution: &mut PnlAttribution,
    instrument_id: &str,
    method: &str,
    num_repricings: usize,
    tolerance_abs: f64,
    tolerance_pct: f64,
) {
    if let Err(e) = attribution.compute_residual() {
        tracing::warn!(
            error = %e,
            instrument_id = %instrument_id,
            method,
            "Residual computation failed; attribution may be incomplete"
        );
    }

    attribution.meta.num_repricings = num_repricings;
    attribution.meta.tolerance_abs = tolerance_abs;
    attribution.meta.tolerance_pct = tolerance_pct;
}

/// Construct a factor P&L [`Money`] from a computed `f64` amount.
///
/// If `amount` is non-finite (NaN or ±Inf), this function:
/// - Appends a diagnostic note to `notes`,
/// - Sets `*result_invalid = true` so [`crate::PnlAttribution::result_invalid`]
///   is propagated to callers, and
/// - Returns a **zero sentinel** in `currency` so the attribution can continue
///   and produce a complete (though flagged-invalid) result rather than
///   failing on [`Money::new`], which rejects non-finite input.
///
/// For finite amounts it delegates directly to [`Money::new`].
#[inline]
pub(crate) fn factor_money_or_invalid(
    amount: f64,
    currency: Currency,
    label: &str,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) -> Money {
    match Money::new(amount, currency) {
        Ok(money) => money,
        Err(error) => {
            notes.push(format!(
                "Invalid factor P&L ({amount:?}) for {label}: {error}; attribution flagged invalid"
            ));
            *result_invalid = true;
            Money::from((0_i64, currency))
        }
    }
}

/// How a sensitivity-based factor's first-order P&L was obtained; the inputs
/// a reviewer needs to reproduce it.
#[derive(Debug, Clone)]
pub(crate) enum FactorWorking {
    /// One sensitivity (currency per `unit`) times one observed move.
    Scalar {
        unit: MoveUnit,
        sensitivity: f64,
        market_move: f64,
    },
    /// Per-tenor `(tenor_years, sensitivity, market_move)` triples in `unit`.
    KeyRate {
        unit: MoveUnit,
        buckets: Vec<(f64, f64, f64)>,
    },
    /// Second-order term only: a convexity/gamma metric applied to one
    /// observed move. The P&L is the row's `gamma_pnl`.
    SecondOrder { unit: MoveUnit, market_move: f64 },
    /// Isolated by a full reprice; `pv` is the repriced value.
    Repriced { pv: Money },
}

/// Build the public [`SensitivityStep`] row for one factor.
///
/// `explained_pnl` and `gamma_pnl` are the amounts already accumulated into
/// the factor bucket. If any sensitivity or move is non-finite (the factor
/// P&L is then already flagged invalid by its caller), the working is omitted
/// and a note records why.
pub(crate) fn sensitivity_step(
    factor: &str,
    working: FactorWorking,
    explained_pnl: Money,
    gamma_pnl: Option<Money>,
    notes: &mut Vec<String>,
) -> SensitivityStep {
    let currency = explained_pnl.currency();
    let mut step = SensitivityStep {
        factor: factor.to_string(),
        move_unit: None,
        sensitivity: None,
        market_move: None,
        buckets: Vec::new(),
        repriced_pv: None,
        explained_pnl,
        gamma_pnl,
    };
    match working {
        FactorWorking::Scalar {
            unit,
            sensitivity,
            market_move,
        } => match Money::new(sensitivity, currency) {
            Ok(sensitivity) if market_move.is_finite() => {
                step.move_unit = Some(unit);
                step.sensitivity = Some(sensitivity);
                step.market_move = Some(market_move);
            }
            _ => notes.push(format!(
                "{factor}: sensitivity working omitted (non-finite sensitivity or market move)"
            )),
        },
        FactorWorking::KeyRate { unit, buckets } => {
            let rows: Option<Vec<SensitivityBucket>> = buckets
                .into_iter()
                .map(|(tenor_years, sensitivity, market_move)| {
                    let sensitivity = Money::new(sensitivity, currency).ok()?;
                    (tenor_years.is_finite() && market_move.is_finite()).then_some(
                        SensitivityBucket {
                            tenor_years,
                            sensitivity,
                            market_move,
                        },
                    )
                })
                .collect();
            match rows {
                Some(rows) => {
                    step.move_unit = Some(unit);
                    step.buckets = rows;
                }
                None => notes.push(format!(
                    "{factor}: sensitivity working omitted (non-finite bucket sensitivity or \
                     market move)"
                )),
            }
        }
        FactorWorking::SecondOrder { unit, market_move } => {
            if market_move.is_finite() {
                step.move_unit = Some(unit);
                step.market_move = Some(market_move);
            } else {
                notes.push(format!(
                    "{factor}: sensitivity working omitted (non-finite market move)"
                ));
            }
        }
        FactorWorking::Repriced { pv } => step.repriced_pv = Some(pv),
    }
    step
}

/// `Money` for a row component whose non-finite case is already flagged
/// through the factor total it belongs to; falls back to zero without a note.
pub(crate) fn money_or_zero(amount: f64, currency: Currency) -> Money {
    Money::new(amount, currency).unwrap_or_else(|_| Money::from((0_i64, currency)))
}

/// Validate that the attribution period is well-formed: `as_of_t1 >= as_of_t0`.
///
/// A reversed period silently flips the sign of theta / carry (`time_period_days`
/// goes negative) and produces a nonsensical decomposition, so it is rejected
/// at every attribution entry point. A zero-length period (`t1 == t0`) is
/// permitted — same-day attribution is a degenerate but valid request, with
/// theta zero over zero elapsed time.
pub(crate) fn validate_attribution_period(as_of_t0: Date, as_of_t1: Date) -> Result<()> {
    if as_of_t1 < as_of_t0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "attribution period is reversed: as_of_t1 ({as_of_t1}) precedes as_of_t0 ({as_of_t0})"
        )));
    }
    Ok(())
}
