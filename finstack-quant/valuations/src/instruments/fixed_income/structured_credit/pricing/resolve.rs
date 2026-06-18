//! Resolution of a deal's base waterfall into its concrete waterfall.
//!
//! This is the seam through which declarative [`WaterfallRules`] are layered
//! onto the base waterfall produced by `StructuredCredit::create_waterfall`.
//! When no rules are present the resolved waterfall is identical to the base
//! (the identity), so deals that configure no rules are bit-for-bit unaffected.
//!
//! Today the only rule is the available-funds cap, whose cap rate (the
//! collateral weighted-average coupon) is effectively constant over the deal's
//! life, so resolution runs once. Future per-period rules (step-down triggers,
//! revolving phases) will call this per period with the evolving deal state.

use crate::instruments::fixed_income::structured_credit::types::{
    AllocationMode, PaymentCalculation, PaymentType, Recipient, ShiftingInterestStep, StepDownSpec,
    StepDownTrigger, Waterfall, WaterfallRules,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use std::borrow::Cow;
use std::collections::HashMap;

/// Resolve `base` into the concrete waterfall for a deal, applying any rules.
///
/// # Arguments
///
/// * `base` - The deal's base waterfall (from `create_waterfall`).
/// * `rules` - Optional declarative rules to layer on.
/// * `collateral_wac` - The collateral weighted-average coupon (decimal), used
///   as the available-funds cap rate.
///
/// # Returns
///
/// A waterfall identical to `base` when `rules` is `None` or carries no
/// applicable rule; otherwise a rewritten copy (e.g. capped-interest recipients
/// for available-funds-capped tranches).
pub fn resolve_waterfall(
    base: &Waterfall,
    rules: Option<&WaterfallRules>,
    collateral_wac: f64,
) -> Waterfall {
    let mut waterfall = base.clone();

    let Some(rules) = rules else {
        return waterfall;
    };

    if let Some(afc) = rules.afc.as_ref() {
        for tier in &mut waterfall.tiers {
            for recipient in &mut tier.recipients {
                // Rewrite TrancheInterest -> CappedTrancheInterest for capped
                // tranches. Compute the replacement first so the immutable
                // borrow from the match ends before the reassignment.
                let replacement = match &recipient.calculation {
                    PaymentCalculation::TrancheInterest {
                        tranche_id,
                        rounding,
                    } if afc.capped_tranches.iter().any(|t| t == tranche_id) => {
                        Some(PaymentCalculation::CappedTrancheInterest {
                            tranche_id: tranche_id.clone(),
                            cap_rate: collateral_wac,
                            rounding: *rounding,
                        })
                    }
                    _ => None,
                };
                if let Some(new_calc) = replacement {
                    recipient.calculation = new_calc;
                }
            }
        }
    }

    waterfall
}

/// Per-period deal-health metrics evaluated against [`StepDownTrigger`]s.
///
/// All fields are computed on the *current* period's balances. See
/// [`StepDownTrigger`] for the exact conventions of each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepDownMetrics {
    /// Cumulative loss as a fraction of the original pool balance.
    pub cumulative_loss_fraction: f64,
    /// Overcollateralization ratio: current pool ÷ rated (non-equity) notes.
    pub oc_ratio: f64,
    /// Senior credit enhancement: `(pool − senior note) ÷ pool`.
    pub credit_enhancement: f64,
}

/// Whether a single trigger passes given this period's metrics.
fn trigger_passes(trigger: &StepDownTrigger, metrics: &StepDownMetrics) -> bool {
    match trigger {
        StepDownTrigger::MaxCumulativeLoss(max) => metrics.cumulative_loss_fraction <= *max,
        StepDownTrigger::MinOcRatio(min) => metrics.oc_ratio >= *min,
        StepDownTrigger::MinCreditEnhancement(min) => metrics.credit_enhancement >= *min,
    }
}

/// Whether the step-down condition holds this period: seasoned past the
/// step-down date and every configured trigger passing (vacuously true for an
/// empty trigger list).
fn step_down_active(spec: &StepDownSpec, date: Date, metrics: &StepDownMetrics) -> bool {
    date >= spec.step_down_date
        && spec
            .triggers
            .iter()
            .all(|trigger| trigger_passes(trigger, metrics))
}

/// Apply per-period step-down to the waterfall's principal allocation.
///
/// Returns `base` unchanged (borrowed) unless a `StepDownSpec` is configured
/// and the step-down condition holds this period — on or after the step-down
/// date with every [`StepDownTrigger`] passing for the given `metrics` — in
/// which case it returns a copy with every principal tier switched to pro-rata
/// allocation, releasing subordination to the junior tranches. While any
/// trigger is breached the deal reverts to sequential (re-evaluated each period).
pub fn apply_step_down<'w>(
    base: &'w Waterfall,
    rules: Option<&WaterfallRules>,
    date: Date,
    metrics: &StepDownMetrics,
) -> Cow<'w, Waterfall> {
    let Some(sd) = rules.and_then(|r| r.step_down.as_ref()) else {
        return Cow::Borrowed(base);
    };

    if !step_down_active(sd, date, metrics) {
        return Cow::Borrowed(base);
    }

    let mut waterfall = base.clone();
    for tier in &mut waterfall.tiers {
        if tier.payment_type == PaymentType::Principal {
            tier.allocation_mode = AllocationMode::ProRata;
        }
    }
    Cow::Owned(waterfall)
}

/// Apply per-period shifting-interest weights to the principal tiers.
///
/// Returns `base` unchanged (borrowed) unless a shifting-interest spec is
/// configured, in which case it returns a copy whose principal tiers are
/// pro-rata with the senior tranche weighted by its scheduled share for the
/// deal's current age (the remainder split equally across the other debt
/// tranches).
pub fn apply_shifting_interest<'w>(
    base: &'w Waterfall,
    rules: Option<&WaterfallRules>,
    months_from_closing: u32,
) -> Cow<'w, Waterfall> {
    let Some(si) = rules.and_then(|r| r.shifting_interest.as_ref()) else {
        return Cow::Borrowed(base);
    };

    let senior_pct = senior_share(&si.schedule, months_from_closing);

    let mut waterfall = base.clone();
    for tier in &mut waterfall.tiers {
        if tier.payment_type != PaymentType::Principal {
            continue;
        }
        tier.allocation_mode = AllocationMode::ProRata;
        let others = tier
            .recipients
            .iter()
            .filter(|r| principal_tranche_id(r).is_some_and(|id| id != si.senior_id.as_str()))
            .count();
        let other_weight = if others > 0 {
            (1.0 - senior_pct) / others as f64
        } else {
            0.0
        };
        for recipient in &mut tier.recipients {
            let id = principal_tranche_id(recipient).map(str::to_string);
            if let Some(id) = id {
                recipient.weight = Some(if id == si.senior_id {
                    senior_pct
                } else {
                    other_weight
                });
            }
        }
    }
    Cow::Owned(waterfall)
}

/// Lock out investor principal during a controlled-accumulation period.
///
/// Sets every `TranchePrincipal` recipient's target to the tranche's *current*
/// balance, so the principal tier requests nothing this period and the investor
/// balances stay flat. Residual cash (including excess interest that would
/// otherwise sweep into senior principal) flows on to the equity/residual tier.
/// Pool principal itself is withheld from the waterfall separately (held in the
/// accumulation funding account) and released as a bullet at the accumulation
/// end. Always returns an owned waterfall (only called while accumulating).
pub fn apply_accumulation_lockout<'w, S: std::hash::BuildHasher>(
    base: &'w Waterfall,
    tranche_balances: &HashMap<String, Money, S>,
) -> Cow<'w, Waterfall> {
    let mut waterfall = base.clone();
    for tier in &mut waterfall.tiers {
        if tier.payment_type != PaymentType::Principal {
            continue;
        }
        for recipient in &mut tier.recipients {
            if let PaymentCalculation::TranchePrincipal {
                tranche_id,
                target_balance,
                ..
            } = &mut recipient.calculation
            {
                if let Some(balance) = tranche_balances.get(tranche_id.as_str()) {
                    *target_balance = Some(*balance);
                }
            }
        }
    }
    Cow::Owned(waterfall)
}

/// Senior share in effect at `months`: the `senior_pct` of the latest schedule
/// step whose `months_from_closing` is `<= months` (or the first step's share
/// when the deal is younger than every step).
fn senior_share(schedule: &[ShiftingInterestStep], months: u32) -> f64 {
    let mut pct = schedule.first().map_or(1.0, |s| s.senior_pct);
    for step in schedule {
        if step.months_from_closing <= months {
            pct = step.senior_pct;
        } else {
            break;
        }
    }
    pct
}

/// Tranche id of a principal recipient, if it pays tranche principal.
fn principal_tranche_id(recipient: &Recipient) -> Option<&str> {
    match &recipient.calculation {
        PaymentCalculation::TranchePrincipal { tranche_id, .. } => Some(tranche_id.as_str()),
        _ => None,
    }
}
