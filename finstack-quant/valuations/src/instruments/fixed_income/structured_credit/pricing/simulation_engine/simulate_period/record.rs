//! Recording the waterfall's payments against each tranche: interest,
//! principal, deferred and PIK interest, net-WAC carryover and balances.

use super::*;

/// Interest shortfalls at or below this amount (in currency units) are float
/// residue from reconstructing the paid coupon by subtraction, not deferrals.
const INTEREST_SHORTFALL_FLOOR: f64 = 1e-6;

/// Record the period's waterfall payments per tranche and roll the note
/// balances, deferred interest and net-WAC carryover forward.
///
/// # Arguments
///
/// * `state` - Simulation state whose results and balances are updated.
/// * `inputs` - The period's deal, market, dates and interest-claim caps.
/// * `waterfall_result` - The executed waterfall's distributions.
/// * `afc_carryover` - Whether capped interest accrues as net-WAC carryover.
///
/// # Returns
///
/// The note interest due this period, summed over the debt tranches.
pub(super) fn record_tranche_flows(
    state: &mut SimulationState,
    inputs: &PeriodInputs<'_>,
    waterfall_result: &WaterfallDistribution,
    afc_carryover: bool,
) -> Result<f64> {
    let (context, claim_caps, period_start) =
        (inputs.context, inputs.claim_caps, inputs.period_start);
    let (pay_date, as_of) = (inputs.period.payment, inputs.period.valuation);
    let mut debt_interest_due = 0.0_f64;
    for (idx, tranche) in state.tranches.tranches.iter().enumerate() {
        let recipient_key = &state.tranche_recipient_keys[idx];
        let tranche_id_str = tranche.id.as_str();

        let current_balance = state
            .tranche_balances
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        let existing_deferred = state
            .deferred_interest
            .get(tranche_id_str)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        // Current-period interest due on post-writedown balance, as the
        // waterfall spec defines the claim (`claim_caps`, F3): uncapped
        // recipients owe the full coupon, capped recipients owe the capped
        // coupon (AFC live cap or a custom static cap — the capped-off
        // portion is never owed, so it never defers), and a debt tranche with
        // no interest recipient owes nothing. The same map sized the
        // excess-spread/reserve draw above, so the two cannot diverge.
        //
        // Equity receives residual interest; its metadata coupon creates no
        // separate debt claim or deferred-interest balance.
        let current_interest_due = if tranche.seniority == TrancheSeniority::Equity {
            Money::from((0_i64, state.base_currency))
        } else {
            match claim_caps.get(tranche_id_str) {
                None => Money::from((0_i64, state.base_currency)),
                Some(cap) => Money::new(
                    tranche_period_interest_due(
                        tranche,
                        current_balance.amount(),
                        TrancheAccrualDates {
                            start: period_start,
                            payment: pay_date,
                            valuation: as_of,
                        },
                        context,
                        cap.unwrap_or(0.0),
                        cap.is_some(),
                        state.floating_rate_shift,
                    )?,
                    state.base_currency,
                )?,
            }
        };
        debt_interest_due += current_interest_due.amount();
        // Carryover repaid this period sits outside the capped claim; it is
        // recorded as interest and taken off the carryover balance below.
        let carryover_paid = Money::new(
            waterfall_result
                .payment_records
                .iter()
                .filter(|record| {
                    matches!(
                        &record.recipient,
                        RecipientType::Tranche(id) if id == tranche_id_str
                    ) && record.recipient_id == format!("net_wac_carryover_{tranche_id_str}")
                })
                .map(|record| record.paid_amount.amount())
                .sum::<f64>(),
            state.base_currency,
        )?;
        let total_interest_claim = if tranche.pik_enabled {
            current_interest_due
        } else {
            existing_deferred.checked_add(current_interest_due)?
        }
        .checked_add(carryover_paid)?;

        let payment_received = waterfall_result
            .distributions
            .get(recipient_key)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));

        // Take the waterfall's OWN interest/principal classification
        // rather than re-deriving it from the aggregate.
        //
        // `distributions` keys a tranche's interest and principal under the
        // same `RecipientType::Tranche(id)`, so this used to reconstruct the
        // split by assuming interest is satisfied FIRST:
        //     interest_paid = min(payment_received, total_interest_claim)
        //     principal     = remainder
        //
        // That silently reclassified principal as interest whenever a tranche
        // carried a shortfall — which is exactly the state an OC cure exists to
        // address. A cure diverted to senior PRINCIPAL was booked as interest,
        // the balance was never retired, and the next period's OC denominator
        // was unchanged: the cure could not de-lever the ratio it was sized to
        // fix. The defect bound precisely in the stress scenarios the cure
        // mechanics were built for.
        //
        // The waterfall already knows which payments were `TranchePrincipal`;
        // `principal_distributions` reports it. Interest is then the remainder,
        // capped by the claim so a residual/equity distribution against a zero
        // interest claim cannot be misbooked as a coupon.
        let principal_from_waterfall = waterfall_result
            .principal_distributions
            .get(recipient_key)
            .copied()
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        let principal_classified = Money::new(
            principal_from_waterfall
                .amount()
                .min(payment_received.amount())
                .max(0.0),
            state.base_currency,
        )?;
        let interest_portion = payment_received
            .checked_sub(principal_classified)
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        let interest_paid = if tranche.seniority == TrancheSeniority::Equity {
            interest_portion
        } else if interest_portion.amount() >= total_interest_claim.amount() {
            total_interest_claim
        } else {
            interest_portion
        };
        let deferred_repaid = Money::new(
            interest_paid
                .amount()
                .min(existing_deferred.amount())
                .max(0.0),
            state.base_currency,
        )?;
        let current_interest_paid = interest_paid
            .checked_sub(deferred_repaid)
            .unwrap_or(Money::from((0_i64, state.base_currency)));
        // `current_interest_paid` is reconstructed by subtraction, so a coupon
        // paid in full can leave a sub-microcent residue; that is float noise,
        // not a deferral, and must not be booked as one.
        let raw_shortfall = current_interest_due.amount() - current_interest_paid.amount();
        let current_interest_shortfall = Money::new(
            if raw_shortfall > INTEREST_SHORTFALL_FLOOR {
                raw_shortfall
            } else {
                0.0
            },
            state.base_currency,
        )?;

        // Only explicitly classified principal retires loss-absorbing capital.
        let principal_payment = principal_classified;

        // Net-WAC carryover: settle what this period repaid, then accrue the
        // interest the cap withheld this period for repayment from later
        // excess (no interest accrues on the carryover itself).
        if afc_carryover && tranche.seniority != TrancheSeniority::Equity {
            if let Some(Some(_)) = claim_caps.get(tranche_id_str) {
                let uncapped_due = tranche_period_interest_due(
                    tranche,
                    current_balance.amount(),
                    TrancheAccrualDates {
                        start: period_start,
                        payment: pay_date,
                        valuation: as_of,
                    },
                    context,
                    0.0,
                    false,
                    state.floating_rate_shift,
                )?;
                let withheld = (uncapped_due - current_interest_due.amount()).max(0.0);
                let entry = state
                    .carryover_balance
                    .entry(tranche_id_str.to_string())
                    .or_insert(Money::from((0_i64, state.base_currency)));
                *entry = Money::new(
                    (entry.amount() - carryover_paid.amount() + withheld).max(0.0),
                    state.base_currency,
                )?;
            }
        }

        if let Some(res) = state.results.get_mut(tranche_id_str) {
            if payment_received.amount() > 0.0 {
                res.cashflows.push((pay_date, payment_received));
            }
            if interest_paid.amount() > 0.0 {
                res.interest_flows.push((pay_date, interest_paid));
                res.total_interest = res.total_interest.checked_add(interest_paid)?;
            }
            if principal_payment.amount() > 0.0 {
                res.principal_flows.push((pay_date, principal_payment));
                res.total_principal = res.total_principal.checked_add(principal_payment)?;
            }
            // PIK and DEFERRED interest are different things and are
            // now recorded separately. PIK capitalizes the shortfall into the
            // tranche balance (it accrues thereafter and enlarges the OC
            // denominator); a non-PIK deferral is a separate senior claim that
            // leaves notional untouched. Booking both under `pik_flows` made
            // `total_pik` unusable as a measure of capitalized balance.
            if current_interest_shortfall.amount() > 0.0 {
                if tranche.pik_enabled {
                    res.pik_flows.push((pay_date, current_interest_shortfall));
                    res.total_pik = res.total_pik.checked_add(current_interest_shortfall)?;
                } else {
                    res.deferred_flows
                        .push((pay_date, current_interest_shortfall));
                    res.total_deferred =
                        res.total_deferred.checked_add(current_interest_shortfall)?;
                }
            }
        }

        let remaining_deferred = if tranche.pik_enabled {
            Money::from((0_i64, state.base_currency))
        } else {
            existing_deferred
                .checked_sub(deferred_repaid)
                .unwrap_or(Money::from((0_i64, state.base_currency)))
                .checked_add(current_interest_shortfall)?
        };
        state
            .deferred_interest
            .insert(tranche_id_str.to_string(), remaining_deferred);

        // Update tranche balance:
        // - Always reduce by principal payment
        // - Only accrete shortfall if PIK is explicitly enabled for this tranche
        //
        // Standard CLO/ABS indenture: shortfalls are tracked as deferred interest
        // and paid from future interest collections, NOT capitalized into balance.
        // Non-PIK deferred balances do not compound (no interest-on-interest);
        // only an explicit `pik_enabled` tranche accretes the shortfall so it
        // earns the note rate thereafter.
        if let Some(current) = state.tranche_balances.get_mut(tranche_id_str) {
            let after_principal = current.checked_sub(principal_payment).unwrap_or(*current);
            // The waterfall nets in-period principal against the period-start
            // balance snapshot, so TranchePrincipal payments cannot exceed the
            // remaining balance. Residual/equity distributions, however, are
            // booked here as "principal" against a zero balance — floor at
            // zero so a negative balance never propagates into later periods'
            // interest accrual and coverage tests.
            let after_principal = if after_principal.amount() < 0.0 {
                Money::from((0_i64, state.base_currency))
            } else {
                after_principal
            };
            if tranche.pik_enabled && current_interest_shortfall.amount() > 0.0 {
                *current = after_principal.checked_add(current_interest_shortfall)?;
            } else {
                *current = after_principal;
            }
        }
    }
    Ok(debt_interest_due)
}
