//! Resolution of a deal's base waterfall into its concrete waterfall.
//!
//! This is the seam through which declarative [`WaterfallRules`] are layered
//! onto the base waterfall produced by `StructuredCredit::create_waterfall`.
//! Each rule edits the period's waterfall in place; the simulation clones the
//! base waterfall at most once per period, on the first rule that applies, so
//! deals that configure no rules run the base waterfall unchanged.

use crate::instruments::fixed_income::structured_credit::types::{
    AfcSpec, AllocationMode, FundingSource, PaymentCalculation, PaymentType, Recipient, ShiftMode,
    ShiftingInterestSpec, ShiftingInterestStep, StepDownSpec, StepDownTrigger, Waterfall,
    WaterfallRules,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use std::collections::HashMap;

/// Rewrite the interest recipients of available-funds-capped tranches to
/// capped interest at `cap_rate`.
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `afc` - Available-funds-cap rule naming the capped tranches.
/// * `cap_rate` - Cap rate (decimal), the live collateral net WAC.
pub(crate) fn apply_afc_cap(waterfall: &mut Waterfall, afc: &AfcSpec, cap_rate: f64) {
    for tier in &mut waterfall.tiers {
        for recipient in &mut tier.recipients {
            // Rewrite TrancheInterest -> CappedTrancheInterest for capped
            // tranches. Compute the replacement first so the immutable borrow
            // from the match ends before the reassignment.
            let replacement = match &recipient.calculation {
                PaymentCalculation::TrancheInterest {
                    tranche_id,
                    rounding,
                } if afc.capped_tranches.iter().any(|t| t == tranche_id) => {
                    Some(PaymentCalculation::CappedTrancheInterest {
                        tranche_id: tranche_id.clone(),
                        cap_rate,
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

/// Per-period deal-health metrics evaluated against [`StepDownTrigger`]s.
///
/// All fields are computed on the *current* period's balances. See
/// [`StepDownTrigger`] for the exact conventions of each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StepDownMetrics {
    /// Delinquent balance as a fraction of the current pool balance.
    pub delinquency_rate: f64,
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
        StepDownTrigger::MaxDelinquency(max) => metrics.delinquency_rate <= *max,
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
/// Whether the deal's step-down is in effect this period: a `StepDownSpec`
/// is configured, `date` is on or after the step-down date and every
/// [`StepDownTrigger`] passes on `metrics`. While any trigger is breached the
/// deal reverts to sequential (re-evaluated each period).
///
/// # Arguments
///
/// * `rules` - The deal's waterfall rules, if any.
/// * `date` - Payment date of the period.
/// * `metrics` - The period's deal-health metrics.
pub(crate) fn step_down_in_effect(
    rules: Option<&WaterfallRules>,
    date: Date,
    metrics: &StepDownMetrics,
) -> bool {
    rules
        .and_then(|r| r.step_down.as_ref())
        .is_some_and(|sd| step_down_active(sd, date, metrics))
}

/// Switch every principal tier to pro-rata allocation weighted by current
/// balance, releasing subordination to the junior tranches. Call only while
/// [`step_down_in_effect`] holds.
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `tranche_balances` - Live note balances used as pro-rata weights.
pub(crate) fn apply_step_down<S: std::hash::BuildHasher>(
    waterfall: &mut Waterfall,
    tranche_balances: &HashMap<String, Money, S>,
) {
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
/// Rewrites the principal tiers to pro-rata with the senior tranche weighted by an *effective* share, and the
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
/// Applying `w_senior` to the combined principal is algebraically the same as
/// paying scheduled and unscheduled in two buckets when the junior remainder
/// is pro-rata by current balance. Prospectus forms that allocate juniors
/// other than pro-rata need a custom waterfall.
///
/// `schedule_senior_pct` depends on the spec's [`ShiftMode`]: under
/// `ShiftOfSubordinate` the schedule value `s` is the shifted share of the
/// subordinates' pro-rata, so
/// `schedule_senior_pct = senior_prorata_share + s · (1 − senior_prorata_share)`;
/// under `SeniorShare` it is `s` itself. While any of the spec's triggers
/// fails on `metrics`, `s` is `1.0` (full lockout).
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `si` - Shifting-interest rule (senior class, schedule, triggers, mode).
/// * `months_from_closing` - Deal age selecting the schedule step.
/// * `senior_prorata_share` - Senior's share of the debt by current balance.
/// * `unscheduled_fraction` - Unscheduled share `u` of the period's principal.
/// * `tranche_balances` - Live note balances splitting the junior remainder.
/// * `metrics` - Deal-health metrics tested against the lockout triggers.
pub(crate) fn apply_shifting_interest<S: std::hash::BuildHasher>(
    waterfall: &mut Waterfall,
    si: &ShiftingInterestSpec,
    months_from_closing: u32,
    senior_prorata_share: f64,
    unscheduled_fraction: f64,
    tranche_balances: &HashMap<String, Money, S>,
    metrics: &StepDownMetrics,
) {
    let locked_out = si
        .triggers
        .iter()
        .any(|trigger| !trigger_passes(trigger, metrics));
    let step = if locked_out {
        1.0
    } else {
        senior_share(&si.schedule, months_from_closing)
    };
    let senior_prorata_share = senior_prorata_share.clamp(0.0, 1.0);
    let schedule_senior_pct = match si.mode {
        ShiftMode::ShiftOfSubordinate => senior_prorata_share + step * (1.0 - senior_prorata_share),
        ShiftMode::SeniorShare => step,
    };
    let u = unscheduled_fraction.clamp(0.0, 1.0);
    let senior_pct = (senior_prorata_share * (1.0 - u) + schedule_senior_pct * u).clamp(0.0, 1.0);

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
            } else {
                // Residual and fee recipients take only what the notes leave:
                // a `None` weight would count as 1.0 in the pro-rata split and
                // hand equity a share of principal during the lockout.
                recipient.weight = Some(0.0);
            }
        }
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
}

/// Apply targeted-overcollateralization amortization to the principal tiers.
///
/// The notes' aggregate principal this period is capped at
/// `required = max(0, notes − max(pool − target_oc, 0))`, allocated by
/// recipient order through each note's `target_balance`; the tiers draw on
/// interest proceeds first so a required amount above the period's principal
/// collections turbos from excess interest. The equity residual recipient is
/// removed from the principal tiers and the residual tier sweeps interest
/// then principal, so the collections above the requirement are released to
/// the residual holder rather than carried forward.
///
/// # Arguments
///
/// * `waterfall` - Period waterfall (already carrying any step-down or
///   shifting-interest weights), edited in place.
/// * `spec` - Target rule.
/// * `current_pool` - Pool balance after this period's collections.
/// * `original_pool` - Original (cut-off) pool balance.
/// * `tranche_balances` - Live note balances.
pub(crate) fn apply_target_oc<S: std::hash::BuildHasher>(
    waterfall: &mut Waterfall,
    spec: &super::super::types::TargetOcSpec,
    current_pool: f64,
    original_pool: f64,
    tranche_balances: &HashMap<String, Money, S>,
) {
    let target_oc = spec.target(current_pool, original_pool);
    let notes: f64 = waterfall
        .tiers
        .iter()
        .filter(|tier| tier.payment_type == PaymentType::Principal)
        .flat_map(|tier| tier.recipients.iter())
        .filter_map(|recipient| tranche_id_of(recipient, TrancheRecipientScope::PrincipalOnly))
        .map(|id| tranche_balances.get(id).map_or(0.0, |m| m.amount()))
        .sum();
    let mut required = (notes - (current_pool - target_oc).max(0.0)).max(0.0);
    for tier in &mut waterfall.tiers {
        match tier.payment_type {
            PaymentType::Principal => {
                tier.funding = Some(FundingSource::InterestThenPrincipal);
                tier.recipients.retain(|recipient| {
                    !matches!(recipient.calculation, PaymentCalculation::ResidualCash)
                });
                for recipient in &mut tier.recipients {
                    if let PaymentCalculation::TranchePrincipal {
                        tranche_id,
                        target_balance,
                        ..
                    } = &mut recipient.calculation
                    {
                        let balance = tranche_balances
                            .get(tranche_id.as_str())
                            .copied()
                            .unwrap_or(Money::from((0_i64, waterfall.base_currency)));
                        let pay = balance.amount().max(0.0).min(required);
                        required -= pay;
                        *target_balance = Money::new(balance.amount() - pay, balance.currency())
                            .ok()
                            .or(Some(balance));
                    }
                }
            }
            PaymentType::Residual => {
                tier.funding = Some(FundingSource::InterestThenPrincipal);
            }
            _ => {}
        }
    }
}

/// Point every `NetWacCarryover` recipient at its tranche's carryover
/// balance brought into the period.
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `carryover` - Net-WAC carryover balance per tranche id.
pub(crate) fn apply_net_wac_carryover<S: std::hash::BuildHasher>(
    waterfall: &mut Waterfall,
    carryover: &HashMap<String, Money, S>,
) {
    for tier in &mut waterfall.tiers {
        for recipient in &mut tier.recipients {
            if let PaymentCalculation::NetWacCarryover { tranche_id, amount } =
                &mut recipient.calculation
            {
                if let Some(balance) = carryover.get(tranche_id.as_str()) {
                    *amount = *balance;
                }
            }
        }
    }
}

/// Point every `ReserveReplenishment` recipient at this period's reserve
/// target (the template inserts the recipient with a placeholder target).
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `target` - Reserve target balance for the period.
pub(crate) fn apply_reserve_target(waterfall: &mut Waterfall, target: Money) {
    for tier in &mut waterfall.tiers {
        for recipient in &mut tier.recipients {
            if let PaymentCalculation::ReserveReplenishment { target_balance } =
                &mut recipient.calculation
            {
                *target_balance = target;
            }
        }
    }
}

/// Lock out investor principal during a controlled-accumulation period.
///
/// Sets every `TranchePrincipal` recipient's target to the tranche's *current*
/// balance, so the principal tier requests nothing this period and the investor
/// balances stay flat. Residual cash (including excess interest that would
/// otherwise sweep into senior principal) flows on to the equity/residual tier.
/// Pool principal itself is withheld from the waterfall separately (held in the
/// accumulation funding account) and released as a bullet at the accumulation
/// end. Only called while accumulating.
///
/// # Arguments
///
/// * `waterfall` - Period waterfall edited in place.
/// * `tranche_balances` - Live note balances held flat this period.
pub(crate) fn apply_accumulation_lockout<S: std::hash::BuildHasher>(
    waterfall: &mut Waterfall,
    tranche_balances: &HashMap<String, Money, S>,
) {
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
            delinquency_rate: 0.0,
            cumulative_loss_fraction: 0.0,
            oc_ratio: 2.0,
            credit_enhancement: 0.5,
        }
    }

    fn balances(a: f64, b: f64) -> HashMap<String, Money> {
        let mut m = HashMap::new();
        m.insert(
            "A".to_string(),
            Money::new(a, Currency::USD).expect("valid money fixture"),
        );
        m.insert(
            "B".to_string(),
            Money::new(b, Currency::USD).expect("valid money fixture"),
        );
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
        let base = Waterfall::builder(Currency::USD)
            .add_tier(principal_tier())
            .build()
            .expect("valid waterfall");
        let rules = stepped_down_rules(date);
        let metrics = healthy_metrics();

        assert!(step_down_in_effect(Some(&rules), date, &metrics));
        let mut resolved = base;
        apply_step_down(&mut resolved, &balances(500.0, 50.0));

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
        let base = Waterfall::builder(Currency::USD)
            .add_tier(principal_tier())
            .build()
            .expect("valid waterfall");
        let rules = stepped_down_rules(date);

        assert!(step_down_in_effect(Some(&rules), date, &healthy_metrics()));
        let mut resolved = base;
        apply_step_down(&mut resolved, &balances(0.0, 0.0));

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

    /// A step-down before its date is not in effect.
    #[test]
    fn step_down_is_not_in_effect_before_its_date() {
        let future = Date::from_calendar_date(2030, Month::January, 1).expect("date");
        let now = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let rules = stepped_down_rules(future);
        assert!(!step_down_in_effect(Some(&rules), now, &healthy_metrics()));
        assert!(!step_down_in_effect(None, now, &healthy_metrics()));
    }
}
