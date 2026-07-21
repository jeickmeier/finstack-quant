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
pub(crate) struct StepDownMetrics {
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
pub(crate) fn apply_step_down<'w, S: std::hash::BuildHasher>(
    base: &'w Waterfall,
    rules: Option<&WaterfallRules>,
    date: Date,
    metrics: &StepDownMetrics,
    tranche_balances: &HashMap<String, Money, S>,
) -> Cow<'w, Waterfall> {
    let Some(sd) = rules.and_then(|r| r.step_down.as_ref()) else {
        return Cow::Borrowed(base);
    };

    if !step_down_active(sd, date, metrics) {
        return Cow::Borrowed(base);
    }

    let mut waterfall = base.clone();
    for tier in &mut waterfall.tiers {
        if tier.payment_type != PaymentType::Principal {
            continue;
        }
        tier.allocation_mode = AllocationMode::ProRata;

        // Step-down principal is pro-rata by current balance, matching shifting
        // interest. Zero-balance recipients receive no allocation.
        for recipient in &mut tier.recipients {
            let balance = tranche_id_of(recipient, TrancheRecipientScope::PrincipalOrInterest)
                .and_then(|id| tranche_balances.get(id))
                .map_or(0.0, |m| m.amount().max(0.0));
            recipient.weight = Some(balance);
        }

        // Degenerate case: every referenced tranche is retired. Leave the tier
        // at equal weights rather than emitting an all-zero weight vector,
        // which `validate_tiers` rejects as an invalid pro-rata tier.
        if tier
            .recipients
            .iter()
            .all(|r| r.weight.unwrap_or(0.0) <= 0.0)
        {
            for recipient in &mut tier.recipients {
                recipient.weight = None;
            }
        }
    }
    Cow::Owned(waterfall)
}

#[derive(Clone, Copy)]
enum TrancheRecipientScope {
    PrincipalOnly,
    PrincipalOrInterest,
}

/// Tranche id referenced by a recipient within the requested payment scope.
fn tranche_id_of(recipient: &Recipient, scope: TrancheRecipientScope) -> Option<&str> {
    match &recipient.calculation {
        PaymentCalculation::TranchePrincipal { tranche_id, .. } => Some(tranche_id.as_str()),
        PaymentCalculation::TrancheInterest { tranche_id, .. }
            if matches!(scope, TrancheRecipientScope::PrincipalOrInterest) =>
        {
            Some(tranche_id.as_str())
        }
        _ => None,
    }
}

/// Apply per-period shifting-interest weights to the principal tiers.
///
/// Returns `base` unchanged (borrowed) unless a shifting-interest spec is
/// configured, in which case it returns a copy whose principal tiers are
/// pro-rata with the senior tranche weighted by an *effective* share, and the
/// remainder split across the other debt tranches **pro-rata by current
/// balance** (`tranche_balances`).
///
/// # Scheduled vs prepayment split
///
/// The shifting-interest schedule lock-out applies only to *unscheduled*
/// principal (prepayments and recovery/liquidation proceeds); *scheduled*
/// amortization is always paid pro-rata. The effective senior weight blends the
/// two by the period's unscheduled fraction `u`:
///
/// ```text
/// w_senior = senior_prorata_share · (1 − u) + schedule_senior_pct · u
/// ```
///
/// where `senior_prorata_share` is the senior's pro-rata share by current
/// balance. With `u = 1` (no scheduled principal, e.g. a bullet pool) this
/// reduces to the pure schedule share; with `u = 0` it is fully pro-rata.
pub(crate) fn apply_shifting_interest<'w, S: std::hash::BuildHasher>(
    base: &'w Waterfall,
    rules: Option<&WaterfallRules>,
    months_from_closing: u32,
    senior_prorata_share: f64,
    unscheduled_fraction: f64,
    tranche_balances: &HashMap<String, Money, S>,
) -> Cow<'w, Waterfall> {
    let Some(si) = rules.and_then(|r| r.shifting_interest.as_ref()) else {
        return Cow::Borrowed(base);
    };

    let schedule_senior_pct = senior_share(&si.schedule, months_from_closing);
    let u = unscheduled_fraction.clamp(0.0, 1.0);
    let senior_pct = (senior_prorata_share * (1.0 - u) + schedule_senior_pct * u).clamp(0.0, 1.0);

    let mut waterfall = base.clone();
    for tier in &mut waterfall.tiers {
        if tier.payment_type != PaymentType::Principal {
            continue;
        }
        tier.allocation_mode = AllocationMode::ProRata;
        // Total current balance of the non-senior principal recipients, used to
        // split the remaining `(1 − senior_pct)` pro-rata. Falls back to an
        // equal split when balances are unavailable/zero.
        let other_ids: Vec<String> = tier
            .recipients
            .iter()
            .filter_map(|r| tranche_id_of(r, TrancheRecipientScope::PrincipalOnly))
            .filter(|id| *id != si.senior_id.as_str())
            .map(str::to_string)
            .collect();
        let other_total: f64 = other_ids
            .iter()
            .map(|id| tranche_balances.get(id).map_or(0.0, |m| m.amount()))
            .sum();
        for recipient in &mut tier.recipients {
            let id =
                tranche_id_of(recipient, TrancheRecipientScope::PrincipalOnly).map(str::to_string);
            if let Some(id) = id {
                let weight = if id == si.senior_id {
                    senior_pct
                } else if other_total > 0.0 {
                    (1.0 - senior_pct) * tranche_balances.get(&id).map_or(0.0, |m| m.amount())
                        / other_total
                } else if !other_ids.is_empty() {
                    (1.0 - senior_pct) / other_ids.len() as f64
                } else {
                    0.0
                };
                recipient.weight = Some(weight);
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
pub(crate) fn apply_accumulation_lockout<'w, S: std::hash::BuildHasher>(
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

/// Senior share in effect at `months`: the `senior_pct` of the schedule step
/// with the greatest `months_from_closing` not exceeding `months` (or the
/// earliest step's share when the deal is younger than every step, defaulting to
/// full lock-out for an empty schedule).
///
/// Scanning for the max-qualifying step — rather than breaking on the first
/// later step — keeps the result correct even if the schedule is not sorted
/// ascending. (`WaterfallRules::validate` enforces a strictly-ascending
/// schedule, so this is defense-in-depth for any direct caller.)
fn senior_share(schedule: &[ShiftingInterestStep], months: u32) -> f64 {
    schedule
        .iter()
        .filter(|s| s.months_from_closing <= months)
        .max_by_key(|s| s.months_from_closing)
        .or_else(|| schedule.iter().min_by_key(|s| s.months_from_closing))
        .map_or(1.0, |s| s.senior_pct)
}

#[cfg(test)]
mod step_down_weight_tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{StepDownSpec, WaterfallTier};
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn principal_tier() -> WaterfallTier {
        WaterfallTier::new("principal", 1, PaymentType::Principal)
            .allocation_mode(AllocationMode::Sequential)
            .add_recipient(Recipient::tranche_principal("A_principal", "A", None))
            .add_recipient(Recipient::tranche_principal("B_principal", "B", None))
    }

    /// Metrics that pass every trigger, so the date alone drives activation.
    fn healthy_metrics() -> StepDownMetrics {
        StepDownMetrics {
            cumulative_loss_fraction: 0.0,
            oc_ratio: 2.0,
            credit_enhancement: 0.5,
        }
    }

    fn balances(a: f64, b: f64) -> HashMap<String, Money> {
        let mut m = HashMap::new();
        m.insert("A".to_string(), Money::new(a, Currency::USD));
        m.insert("B".to_string(), Money::new(b, Currency::USD));
        m
    }

    fn stepped_down_rules(date: Date) -> WaterfallRules {
        WaterfallRules {
            step_down: Some(StepDownSpec {
                step_down_date: date,
                triggers: Vec::new(),
            }),
            ..Default::default()
        }
    }

    /// Active step-down principal is weighted by current tranche balance.
    #[test]
    fn step_down_weights_principal_by_current_balance() {
        let date = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let base = Waterfall::new(Currency::USD).add_tier(principal_tier());
        let rules = stepped_down_rules(date);
        let metrics = healthy_metrics();

        let resolved = apply_step_down(&base, Some(&rules), date, &metrics, &balances(500.0, 50.0));

        let tier = resolved
            .tiers
            .iter()
            .find(|t| t.payment_type == PaymentType::Principal)
            .expect("principal tier");
        assert_eq!(tier.allocation_mode, AllocationMode::ProRata);

        let weight = |id: &str| {
            tier.recipients
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.weight)
                .unwrap_or(f64::NAN)
        };
        assert!(
            (weight("A_principal") - 500.0).abs() < 1e-9,
            "senior weight must be its current balance, got {}",
            weight("A_principal")
        );
        assert!(
            (weight("B_principal") - 50.0).abs() < 1e-9,
            "junior weight must be its current balance, got {}",
            weight("B_principal")
        );
        assert!(
            weight("A_principal") > weight("B_principal") * 9.0,
            "a 10:1 balance ratio must produce a ~10:1 weight ratio, not the \
             1:1 equal-share fallback"
        );
    }

    /// Fully retired tiers leave weights unset instead of emitting all zeros.
    #[test]
    fn step_down_with_all_tranches_retired_leaves_weights_unset() {
        let date = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let base = Waterfall::new(Currency::USD).add_tier(principal_tier());
        let rules = stepped_down_rules(date);

        let resolved = apply_step_down(
            &base,
            Some(&rules),
            date,
            &healthy_metrics(),
            &balances(0.0, 0.0),
        );

        let tier = resolved
            .tiers
            .iter()
            .find(|t| t.payment_type == PaymentType::Principal)
            .expect("principal tier");
        assert!(
            tier.recipients.iter().all(|r| r.weight.is_none()),
            "a fully-retired structure must not emit an all-zero weight vector"
        );
    }

    /// An inactive step-down must be exact identity — no clone, no weights.
    #[test]
    fn inactive_step_down_is_identity() {
        let future = Date::from_calendar_date(2030, Month::January, 1).expect("date");
        let now = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let base = Waterfall::new(Currency::USD).add_tier(principal_tier());
        let rules = stepped_down_rules(future);

        let resolved = apply_step_down(
            &base,
            Some(&rules),
            now,
            &healthy_metrics(),
            &balances(500.0, 50.0),
        );
        assert!(
            matches!(resolved, Cow::Borrowed(_)),
            "an inactive step-down must borrow the base waterfall unchanged"
        );
    }
}
