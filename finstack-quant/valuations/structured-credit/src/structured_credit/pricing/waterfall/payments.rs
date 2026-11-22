//! Recipient payment calculators, coupon shifting, interest claim caps, and senior fee accrual.

use super::allocation::{
    AllocationContext, AllocationOutput, currency_decimal_places, unpaid_tranche_balance,
};
use crate::structured_credit::types::{
    AfcSpec, PaymentCalculation, PaymentType, RoundingConvention, Tranche, TrancheStructure,
    Waterfall,
};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Contractual coupon for the period with the simulated rate-path shift applied.
///
/// A FLOATING tranche's coupon moves with the simulated short-rate path
/// (`floating_rate_shift`), floored at zero; a fixed coupon is contractual and
/// unaffected. This is the same shift-then-floor rule as the engine's
/// interest-due kernel, so allocation and recording cannot diverge on the path.
fn shifted_tranche_rate(
    tranche: &Tranche,
    period_start: Date,
    period_end: Date,
    accrual_year_fraction: f64,
    valuation_date: Date,
    market: &MarketContext,
    floating_rate_shift: f64,
) -> Result<f64> {
    let raw = tranche.coupon.try_rate_for_period(
        period_start,
        period_end,
        accrual_year_fraction,
        valuation_date,
        market,
    )?;
    Ok(match tranche.coupon {
        RateSpec::Floating(_) => (raw + floating_rate_shift).max(0.0),
        RateSpec::Fixed { .. } => raw,
    })
}

/// Per-tranche interest-claim definition extracted from a waterfall spec.
///
/// The waterfall is the single source of truth for what interest a tranche is
/// *owed* each period, not merely how cash is allocated:
///
/// - `Some(None)` — an uncapped [`PaymentCalculation::TrancheInterest`]
///   recipient: the tranche is owed its full coupon accrual.
/// - `Some(Some(cap))` — a [`PaymentCalculation::CappedTrancheInterest`]
///   recipient: the claim itself is capped at `cap` (available-funds style),
///   so the capped-off portion is never owed and never defers.
/// - absent key — the waterfall defines **no** interest claim for the tranche
///   (e.g. a principal-only class): nothing accrues, nothing defers.
///
/// When `afc` names a tranche whose recipient is still the uncapped variant,
/// `live_afc_cap` is applied — exactly the rewrite `resolve::apply_afc_cap` performs
/// on the period waterfall, so claims extracted from the base waterfall agree
/// with the allocation the resolved waterfall executes.
///
/// `StructuredCredit::validate_custom_waterfall` enforces at most one
/// interest-type recipient per tranche, so the extraction is unambiguous.
pub(crate) fn interest_claim_caps<'w>(
    waterfall: &'w Waterfall,
    afc: Option<&AfcSpec>,
    live_afc_cap: f64,
) -> FxHashMap<&'w str, Option<f64>> {
    let mut caps: FxHashMap<&'w str, Option<f64>> = FxHashMap::default();
    for tier in &waterfall.tiers {
        for recipient in &tier.recipients {
            match &recipient.calculation {
                PaymentCalculation::TrancheInterest { tranche_id, .. } => {
                    let afc_capped = afc
                        .is_some_and(|spec| spec.capped_tranches.iter().any(|t| t == tranche_id));
                    caps.insert(tranche_id.as_str(), afc_capped.then_some(live_afc_cap));
                }
                PaymentCalculation::CappedTrancheInterest {
                    tranche_id,
                    cap_rate,
                    ..
                } => {
                    caps.insert(tranche_id.as_str(), Some(*cap_rate));
                }
                _ => {}
            }
        }
    }
    caps
}

/// Inputs the senior-fee kernel needs beyond the waterfall and tranche index.
pub(crate) struct SeniorFeeInputs<'a> {
    /// Cash the fee tier is measured against (pool interest for a period).
    pub available: Money,
    /// Current tranche balances, when known.
    pub tranche_balances: Option<&'a FxHashMap<String, Money>>,
    /// Outstanding deferred interest, when known.
    pub deferred_interest: Option<&'a FxHashMap<String, Money>>,
    /// Collateral balance the percentage-of-collateral fees accrue on.
    pub pool_balance: Money,
    /// Balance of the specially serviced collateral (special servicing fee base).
    pub special_serviced_balance: Money,
    /// Accrual period start.
    pub period_start: Date,
    /// Payment date.
    pub payment_date: Date,
    /// Contractual end of coupon and fee accrual, before payment adjustment.
    pub accrual_end: Date,
    /// Valuation date.
    pub valuation_date: Date,
    /// Market context for any index-linked fee.
    pub market: &'a MarketContext,
    /// Reserve balance, for reserve-linked calculations.
    pub reserve_balance: Money,
    /// Simulated floating-coupon shift; zero outside OAS runs.
    pub floating_rate_shift: f64,
}

/// Senior fees accruing this period, i.e. the fee tiers that rank ahead of
/// every note.
///
/// Priority determines seniority even when deserialized tiers arrive out of order.
/// Only fees ranking ahead of the first non-fee tier enter the senior claim.
///
/// This is the single source of truth for "what the fee tier will take",
/// shared by three call sites that must agree:
///   * the IC numerator, which nets it from interest collections;
///   * the excess-spread capture/draw and the reserve draw (N1), which must
///     treat it as a senior claim ranking ahead of note interest;
///   * the waterfall itself, which actually pays it.
///
/// Computing it with `calculate_payment_amount` — the same kernel the fee tier
/// uses to pay — is what makes the measured and paid amounts identical by
/// construction rather than by coincidence.
pub(crate) fn senior_fee_accrual(
    waterfall: &Waterfall,
    tranches: &TrancheStructure,
    tranche_index: &FxHashMap<&str, usize>,
    inputs: SeniorFeeInputs<'_>,
) -> Result<Money> {
    // Senior fees are measured before any tier pays: no in-period principal
    // and no equity cash yet.
    let nothing_paid = AllocationOutput::new(waterfall.currency, 0);
    let ctx = AllocationContext {
        currency: waterfall.currency,
        tranches,
        tranche_index,
        pool_balance: inputs.pool_balance,
        special_serviced_balance: inputs.special_serviced_balance,
        payment_date: inputs.payment_date,
        accrual_end: inputs.accrual_end,
        valuation_date: inputs.valuation_date,
        market: inputs.market,
        tranche_balances: inputs.tranche_balances,
        deferred_interest: inputs.deferred_interest,
        reserve_balance: inputs.reserve_balance,
        floating_rate_shift: inputs.floating_rate_shift,
        equity_history: None,
    };
    let mut total = Money::from((0_i64, waterfall.currency));
    let first_note_priority = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type != PaymentType::Fee)
        .map(|tier| tier.priority)
        .min()
        .unwrap_or(usize::MAX);
    let mut fees: Vec<_> = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type == PaymentType::Fee && tier.priority < first_note_priority)
        .collect();
    fees.sort_by_key(|tier| tier.priority);
    for tier in fees {
        for recipient in &tier.recipients {
            let amount = calculate_payment_amount(
                &ctx,
                &recipient.calculation,
                inputs.available,
                inputs.period_start,
                false,
                &nothing_paid,
            )?;
            total = total.checked_add(amount)?;
        }
    }
    Ok(total)
}

/// Calculate payment amount for a recipient.
///
/// `ctx` supplies the period's balances, dates and market; `paid_so_far`
/// supplies the principal already paid per tranche and the equity cash paid
/// earlier in this period. `diverted` marks a coverage-cure diversion pass.
pub(super) fn calculate_payment_amount(
    ctx: &AllocationContext<'_>,
    calculation: &PaymentCalculation,
    available: Money,
    period_start: Date,
    diverted: bool,
    paid_so_far: &AllocationOutput,
) -> Result<Money> {
    let &AllocationContext {
        currency,
        tranches,
        tranche_index,
        pool_balance,
        special_serviced_balance,
        payment_date,
        accrual_end,
        valuation_date,
        market,
        tranche_balances,
        deferred_interest,
        reserve_balance,
        floating_rate_shift,
        equity_history,
    } = ctx;
    let principal_paid_in_period = &paid_so_far.principal_paid_in_period;
    let equity_paid_in_period = paid_so_far.equity_paid;
    let (raw_amount, rounding) = match calculation {
        PaymentCalculation::FixedAmount { amount, rounding } => (amount.amount(), *rounding),

        PaymentCalculation::PercentageOfCollateral {
            rate,
            annualized,
            day_count,
            rounding,
        } => {
            let accrual_fraction = if *annualized {
                day_count.unwrap_or(DayCount::Act360).year_fraction(
                    period_start,
                    accrual_end,
                    DayCountContext::default(),
                )?
            } else {
                1.0
            };
            (pool_balance.amount() * rate * accrual_fraction, *rounding)
        }

        PaymentCalculation::PercentageOfSpecialServiced {
            rate,
            annualized,
            day_count,
            rounding,
        } => {
            let accrual_fraction = if *annualized {
                day_count.unwrap_or(DayCount::Act360).year_fraction(
                    period_start,
                    accrual_end,
                    DayCountContext::default(),
                )?
            } else {
                1.0
            };
            (
                special_serviced_balance.amount() * rate * accrual_fraction,
                *rounding,
            )
        }

        PaymentCalculation::TrancheInterest {
            tranche_id,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // Use current tranche balance when available. Floor at zero so a
            // rounding-induced negative balance can never produce a negative
            // interest claim (which would distort IC coverage tests).
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance)
                .amount()
                .max(0.0);
            let accrual_fraction = tranche.day_count.year_fraction(
                period_start,
                accrual_end,
                DayCountContext::default(),
            )?;
            let rate = shifted_tranche_rate(
                tranche,
                period_start,
                accrual_end,
                accrual_fraction,
                valuation_date,
                market,
                floating_rate_shift,
            )?;
            // Invariant: `deferred_interest` is `None` outside multi-period
            // carry-forward runs and omits tranches with no prior unpaid
            // interest shortfall, so absence strictly means 0.0 carried interest.
            let carried = deferred_interest
                .and_then(|d| d.get(tranche_id.as_str()))
                .map(|m| m.amount())
                .unwrap_or(0.0);
            (balance * rate * accrual_fraction + carried, *rounding)
        }

        PaymentCalculation::CappedTrancheInterest {
            tranche_id,
            cap_rate,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // Floored at zero for the same reason as `TrancheInterest`.
            let balance = tranche_balances
                .and_then(|b| b.get(tranche_id.as_str()))
                .copied()
                .unwrap_or(tranche.current_balance)
                .amount()
                .max(0.0);
            // Available-funds cap: the effective coupon cannot exceed `cap_rate`.
            // The cap applies AFTER the rate-path shift, matching the engine's
            // interest-due kernel (the cap tracks the shifted net WAC).
            let accrual_fraction = tranche.day_count.year_fraction(
                period_start,
                accrual_end,
                DayCountContext::default(),
            )?;
            let rate = shifted_tranche_rate(
                tranche,
                period_start,
                accrual_end,
                accrual_fraction,
                valuation_date,
                market,
                floating_rate_shift,
            )?
            .min(*cap_rate);
            // Invariant: same as `TrancheInterest` above — absent
            // `deferred_interest` means zero carried interest shortfall.
            let carried = deferred_interest
                .and_then(|d| d.get(tranche_id.as_str()))
                .map(|m| m.amount())
                .unwrap_or(0.0);
            (balance * rate * accrual_fraction + carried, *rounding)
        }

        PaymentCalculation::TranchePrincipal {
            tranche_id,
            target_balance,
            rounding,
        } => {
            let idx = *tranche_index.get(tranche_id.as_str()).ok_or_else(|| {
                Error::from(finstack_quant_core::InputError::NotFound {
                    id: format!("tranche:{}", tranche_id),
                })
            })?;
            let tranche = &tranches.tranches[idx];
            // Net out principal already paid to this tranche by earlier tiers
            // in the SAME period (e.g. its regular principal tier before an
            // OC/IC diversion). The snapshot balance is period-start, so
            // without this the tranche requests its full balance twice and
            // gets over-paid into a negative balance.
            let unpaid =
                unpaid_tranche_balance(tranche, tranche_balances, principal_paid_in_period);
            // A coverage-cure diversion pays the senior tranche *below* its
            // scheduled target (toward zero) to de-leverage the structure;
            // the scheduled target only applies to the tier's regular pass.
            let target = if diverted {
                Money::from((0_i64, currency))
            } else {
                target_balance.unwrap_or(Money::from((0_i64, currency)))
            };
            let needed = (unpaid - target.amount()).max(0.0);
            (needed, *rounding)
        }

        PaymentCalculation::ResidualCash => (available.amount(), None),

        PaymentCalculation::NetWacCarryover { amount, .. } => (amount.amount().max(0.0), None),

        PaymentCalculation::IncentiveFee { hurdle_irr, share } => {
            if !share.is_finite() || !(0.0..=1.0).contains(share) {
                return Err(Error::Validation(format!(
                    "waterfall recipient calculation IncentiveFee.share must be a decimal \
                     fraction in [0, 1] (0.2 = 20%), got {share}"
                )));
            }
            // The hurdle is tested on equity's cash to date plus what it has
            // already received earlier in this run plus this cash; the share
            // applies only to the part of this cash above the hurdle, so the
            // crossing period is split rather than all-or-nothing.
            let fee = match equity_history {
                Some(history) => {
                    let total = equity_paid_in_period.checked_add(available)?;
                    let shortfall = history
                        .hurdle_shortfall(payment_date, total, *hurdle_irr)
                        .amount();
                    let excess = (total.amount() - shortfall.max(equity_paid_in_period.amount()))
                        .clamp(0.0, available.amount());
                    excess * share
                }
                None => 0.0,
            };
            (fee, None)
        }

        PaymentCalculation::ReserveReplenishment { target_balance } => {
            // Shortfall = max(0, target - current). Current balance is passed
            // dynamically from SimulationState, not stored in the waterfall definition.
            let shortfall = target_balance
                .checked_sub(reserve_balance)
                .unwrap_or(Money::from((0_i64, currency)));
            (shortfall.amount().max(0.0).min(available.amount()), None)
        }
    };

    if let Some(convention) = rounding {
        // m1 fix: use currency-specific decimal places
        let decimals = currency_decimal_places(currency) as i32;
        let scale = 10f64.powi(decimals);
        let val = raw_amount;
        let rounded_val = match convention {
            RoundingConvention::Nearest => {
                finstack_quant_core::math::round_half_away(val, decimals)?
            }
            RoundingConvention::Floor => (val * scale).floor() / scale,
            RoundingConvention::Ceiling => (val * scale).ceil() / scale,
        };
        Ok(Money::from_model_value(rounded_val, currency)?)
    } else {
        Ok(Money::from_model_value(raw_amount, currency)?)
    }
}
