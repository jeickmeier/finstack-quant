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
    AllocationMode, PaymentCalculation, PaymentType, Recipient, ShiftingInterestStep, Waterfall,
    WaterfallRules,
};
use finstack_quant_core::dates::Date;
use std::borrow::Cow;

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

/// Apply per-period step-down to the waterfall's principal allocation.
///
/// Returns `base` unchanged (borrowed) unless a `StepDownSpec` is configured
/// and the step-down condition holds this period — on or after the step-down
/// date with cumulative losses below the trigger — in which case it returns a
/// copy with every principal tier switched to pro-rata allocation, releasing
/// subordination to the junior tranches. While the loss trigger is breached the
/// deal reverts to sequential (the switch is re-evaluated every period).
pub fn apply_step_down<'w>(
    base: &'w Waterfall,
    rules: Option<&WaterfallRules>,
    date: Date,
    cumulative_loss_fraction: f64,
) -> Cow<'w, Waterfall> {
    let Some(sd) = rules.and_then(|r| r.step_down.as_ref()) else {
        return Cow::Borrowed(base);
    };

    let stepped_down =
        date >= sd.step_down_date && cumulative_loss_fraction < sd.max_cumulative_loss_pct;
    if !stepped_down {
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
