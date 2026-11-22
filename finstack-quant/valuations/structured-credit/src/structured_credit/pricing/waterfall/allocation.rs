//! Tier allocation engines (sequential, pro-rata water-filling) and principal accounting.

use super::payments::calculate_payment_amount;
use crate::structured_credit::types::{
    EquityHistory, PaymentCalculation, PaymentRecord, PaymentType, Recipient, RecipientType,
    Tranche, TrancheSeniority, TrancheStructure, WaterfallTier,
};
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::Result;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Returns the number of decimal places for currency-aware penny-safe allocation.
#[inline]
pub(super) fn currency_decimal_places(currency: Currency) -> u32 {
    u32::from(currency.decimals())
}

/// Returns the scaling factor for converting amounts to smallest currency units.
#[inline]
fn currency_scale_factor(currency: Currency) -> f64 {
    let decimals = currency_decimal_places(currency);
    10_f64.powi(decimals as i32)
}

#[inline]
pub(super) fn to_currency_units(amount: f64, scale: f64) -> Result<i64> {
    if !amount.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(Error::Validation(
            "Invalid amount or scale for currency unit conversion".to_string(),
        ));
    }
    let scaled = amount * scale;
    if !scaled.is_finite() || scaled.abs() > i64::MAX as f64 {
        return Err(Error::Validation(
            "Tier amount exceeds penny-safe allocation capacity".to_string(),
        ));
    }
    let rounded = scaled.round();
    if rounded > i64::MAX as f64 || rounded < i64::MIN as f64 {
        return Err(Error::Validation(
            "Tier amount exceeds penny-safe allocation capacity".to_string(),
        ));
    }
    Ok(rounded as i64)
}

/// Immutable context for waterfall allocation operations.
///
/// Groups parameters that remain constant during allocation, reducing
/// parameter count in allocation functions.
pub(crate) struct AllocationContext<'a> {
    /// Base currency for allocations
    pub(crate) currency: Currency,
    /// Tranche structure for looking up tranche data
    pub(crate) tranches: &'a TrancheStructure,
    /// O(1) lookup from tranche ID to index
    pub(crate) tranche_index: &'a FxHashMap<&'a str, usize>,
    /// Current pool balance
    pub(crate) pool_balance: Money,
    /// Balance of the specially serviced collateral (special servicing fee base).
    pub(crate) special_serviced_balance: Money,
    /// Payment date
    pub(crate) payment_date: Date,
    pub(crate) accrual_end: Date,
    /// Valuation date for fixing lifecycle decisions.
    pub(crate) valuation_date: Date,
    /// Market context for rate lookups
    pub(crate) market: &'a MarketContext,
    /// Current tranche balances (overrides tranche.current_balance when present)
    pub(crate) tranche_balances: Option<&'a FxHashMap<String, Money>>,
    /// Deferred interest claims carried from prior periods.
    pub(crate) deferred_interest: Option<&'a FxHashMap<String, Money>>,
    /// Current reserve account balance (passed dynamically each period)
    pub(crate) reserve_balance: Money,
    /// Simulated shift applied to FLOATING tranche coupons (OAS rate
    /// path); zero outside OAS runs. Threading it here keeps the cash the
    /// waterfall *allocates* on the same rate path as the interest the engine
    /// *records* in Step 5.
    pub(crate) floating_rate_shift: f64,
    /// Equity cash to date for incentive-fee IRR tests.
    pub(crate) equity_history: Option<&'a EquityHistory>,
}

/// Mutable output for allocation tracking.
///
/// Groups mutable state that is updated during allocation.
pub(crate) struct AllocationOutput {
    /// Cash paid so far this period to `RecipientType::Equity`, the base of
    /// the incentive-fee hurdle.
    pub(crate) equity_paid: Money,
    /// Payment records for audit trail
    pub(crate) payment_records: Vec<PaymentRecord>,
    /// Principal paid to each tranche (by id) so far this period. Netted
    /// against the period-start balance so regular and diverted tiers cannot
    /// retire the same notional twice.
    pub(crate) principal_paid_in_period: FxHashMap<String, Money>,
}

impl AllocationOutput {
    /// Create new allocation state with pre-allocated capacity.
    pub(crate) fn new(currency: Currency, estimated_recipients: usize) -> Self {
        Self {
            equity_paid: Money::from((0_i64, currency)),
            payment_records: Vec::with_capacity(estimated_recipients),
            principal_paid_in_period: FxHashMap::default(),
        }
    }

    /// Record one recipient's payment.
    fn record(&mut self, record: PaymentRecord) -> Result<()> {
        if record.recipient == RecipientType::Equity {
            self.equity_paid = self.equity_paid.checked_add(record.paid_amount)?;
        }
        self.payment_records.push(record);
        Ok(())
    }
}

/// Allocate cash sequentially to recipients.
pub(super) fn allocate_sequential(
    ctx: &AllocationContext,
    tier: &WaterfallTier,
    recipients: &[Recipient],
    mut available: Money,
    period_start: Date,
    diverted: bool,
    output: &mut AllocationOutput,
) -> Result<Money> {
    let currency = ctx.currency;
    let mut tier_total = Money::from((0_i64, currency));

    for recipient in recipients {
        if available.amount() <= 0.0 {
            break;
        }

        let requested = calculate_payment_amount(
            ctx,
            &recipient.calculation,
            available,
            period_start,
            diverted,
            output,
        )?;
        let requested = cap_residual_at_debt_balance(
            ctx,
            recipient,
            requested,
            &output.principal_paid_in_period,
        )?;

        let paid = if requested.amount() <= available.amount() {
            requested
        } else {
            available
        };

        record_in_period_principal(ctx, &mut output.principal_paid_in_period, recipient, paid)?;

        let shortfall = requested
            .checked_sub(paid)
            .unwrap_or(Money::from((0_i64, currency)));

        output.record(PaymentRecord {
            tier_id: tier.id.clone(),
            recipient_id: recipient.id.clone(),
            priority: tier.priority,
            recipient: recipient.recipient_type.clone(),
            requested_amount: requested,
            paid_amount: paid,
            shortfall,
            diverted,
            is_principal: is_principal_payment(ctx, recipient, tier.payment_type),
        })?;

        tier_total = tier_total.checked_add(paid)?;
        available = available.checked_sub(paid)?;
    }

    Ok(tier_total)
}

/// Allocate cash pro-rata to recipients using penny-safe allocation.
pub(super) fn allocate_pro_rata(
    ctx: &AllocationContext,
    tier: &WaterfallTier,
    recipients: &[Recipient],
    available: Money,
    period_start: Date,
    diverted: bool,
    output: &mut AllocationOutput,
) -> Result<Money> {
    let currency = ctx.currency;
    if recipients.is_empty() {
        return Ok(Money::from((0_i64, currency)));
    }

    let mut total_requested = Money::from((0_i64, currency));
    let mut recipient_requests = Vec::with_capacity(recipients.len());

    for recipient in recipients {
        let requested = calculate_payment_amount(
            ctx,
            &recipient.calculation,
            available,
            period_start,
            diverted,
            output,
        )?;
        let requested = cap_residual_at_debt_balance(
            ctx,
            recipient,
            requested,
            &output.principal_paid_in_period,
        )?;
        total_requested = total_requested.checked_add(requested)?;
        recipient_requests.push((recipient, requested));
    }

    let tier_available = if total_requested.amount() <= available.amount() {
        total_requested
    } else {
        available
    };

    // Penny-safe weighted allocation. Each recipient is capped at its own request;
    // the capped excess from a recipient whose weight-share exceeds its request is
    // *water-filled* onto the recipients still below their caps, so the full
    // `tier_available` is distributed instead of leaking to the next tier (which,
    // for a single combined principal tier, is the residual/equity tier). Without
    // this redistribution a small-balance senior carrying a large shifting-interest
    // weight would drop its excess past outstanding junior debt — a subordination
    // inversion.
    let scale = currency_scale_factor(currency);
    let tier_available_units = to_currency_units(tier_available.amount(), scale)?;

    let weights: Vec<f64> = recipient_requests
        .iter()
        .map(|(recipient, _)| recipient.weight.unwrap_or(1.0))
        .collect();
    let caps: Vec<i64> = recipient_requests
        .iter()
        .map(|(_, requested)| to_currency_units(requested.amount(), scale))
        .collect::<Result<Vec<_>>>()?;

    let final_units = water_fill_allocation(tier_available_units, &weights, &caps);

    let mut tier_total = Money::from((0_i64, currency));

    for (idx, (recipient, requested)) in recipient_requests.iter().enumerate() {
        let allocated = Money::from_model_value(final_units[idx] as f64 / scale, currency)?;

        // `water_fill_allocation` never allocates above a recipient's cap
        // (`requested`); the `min` is retained as a defensive floor.
        let paid = if allocated.amount() <= requested.amount() {
            allocated
        } else {
            *requested
        };

        record_in_period_principal(ctx, &mut output.principal_paid_in_period, recipient, paid)?;

        let shortfall = requested
            .checked_sub(paid)
            .unwrap_or(Money::from((0_i64, currency)));

        output.record(PaymentRecord {
            tier_id: tier.id.clone(),
            recipient_id: recipient.id.clone(),
            priority: tier.priority,
            recipient: recipient.recipient_type.clone(),
            requested_amount: *requested,
            paid_amount: paid,
            shortfall,
            diverted,
            is_principal: is_principal_payment(ctx, recipient, tier.payment_type),
        })?;

        tier_total = tier_total.checked_add(paid)?;
    }

    Ok(tier_total)
}

/// Penny-exact weighted water-filling of `total_units` across recipients, each
/// capped at `caps[i]` (its request in currency units), in proportion to
/// `weights[i]`.
///
/// A recipient whose proportional share exceeds its cap is filled to the cap and
/// the freed units are redistributed across the recipients still below their
/// caps, iterating until either all units are placed or every recipient is at
/// its cap. The integer remainder left by flooring is handed out one unit at a
/// time to the still-open recipients with the largest fractional shares (each
/// such recipient has at least one unit of headroom, so a cap is never
/// exceeded).
///
/// The returned allocation sums to `min(total_units, Σ caps)` (up to currency
/// rounding); any shortfall means every recipient is already at its request, so
/// the unplaced cash correctly remains for the next tier. This is the fix for
/// the subordination inversion where weight-capped excess used to leak straight
/// to the residual tier.
pub(super) fn water_fill_allocation(total_units: i64, weights: &[f64], caps: &[i64]) -> Vec<i64> {
    let n = weights.len();
    let mut alloc = vec![0i64; n];
    if n == 0 || total_units <= 0 {
        return alloc;
    }

    let mut open: Vec<usize> = (0..n).filter(|&i| caps[i] > 0).collect();
    let mut remaining = total_units;

    loop {
        if remaining <= 0 || open.is_empty() {
            break;
        }

        let active_weight: f64 = open.iter().map(|&i| weights[i].max(0.0)).sum();

        // Degenerate weights (all zero/negative on the open set): spread the
        // remaining units as evenly as possible across the open recipients,
        // respecting caps. Bulk-allocate the even base first (capping iterates),
        // then hand out the sub-`open.len()` remainder one unit each.
        if active_weight <= 0.0 {
            let m = open.len() as i64;
            let base = remaining / m;
            let mut progressed = false;
            if base > 0 {
                for &i in &open {
                    let add = base.min(caps[i] - alloc[i]);
                    if add > 0 {
                        alloc[i] += add;
                        remaining -= add;
                        progressed = true;
                    }
                }
            }
            for &i in &open {
                if remaining <= 0 {
                    break;
                }
                if alloc[i] < caps[i] {
                    alloc[i] += 1;
                    remaining -= 1;
                    progressed = true;
                }
            }
            open.retain(|&i| alloc[i] < caps[i]);
            if !progressed {
                break;
            }
            continue;
        }

        let remaining_f = remaining as f64;
        // Provisional floor allocation proportional to weight, tracking the
        // fractional part for the largest-remainder tie-break below.
        let mut provisional: Vec<(usize, i64, f64)> = open
            .iter()
            .map(|&i| {
                let want = remaining_f * (weights[i].max(0.0) / active_weight);
                let floor = want.floor() as i64;
                (i, floor, want - floor as f64)
            })
            .collect();

        let mut any_capped = false;
        let mut placed = 0i64;
        for &(i, floor, _) in &provisional {
            let headroom = caps[i] - alloc[i];
            let add = floor.min(headroom);
            alloc[i] += add;
            placed += add;
            if floor >= headroom {
                any_capped = true;
            }
        }
        remaining -= placed;
        open.retain(|&i| alloc[i] < caps[i]);

        if !any_capped {
            // No recipient hit its cap this round, so the leftover (< open.len()
            // units, lost only to flooring) goes one unit at a time to the
            // largest fractional shares. Each still-open recipient has >= 1 unit
            // of headroom, so caps continue to hold.
            provisional.retain(|&(i, _, _)| alloc[i] < caps[i]);
            provisional.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for &(i, _, _) in &provisional {
                if remaining <= 0 {
                    break;
                }
                alloc[i] += 1;
                remaining -= 1;
            }
            break;
        }
    }

    alloc
}

/// Evaluate coverage tests.
///
/// `principal_collections` is the cash component of the OC numerator
/// (standard CLO par-OC counts only principal proceeds, never interest).
/// Whether a payment calculation pays PRINCIPAL rather than interest.
///
/// The waterfall's own classification, decided in exactly one place.
/// `RecipientType::Tranche(id)` is the same map key for a tranche's interest
/// and its principal, so `distributions` aggregates them; without this the
/// engine had to guess the split by assuming interest is satisfied first.
///
/// Residual cash keeps its source for the residual holder, where only a
/// principal tier repays capital. A debt class has no residual claim, so what
/// reaches it is always principal: see [`residual_debt_tranche`].
fn is_principal_payment(
    ctx: &AllocationContext<'_>,
    recipient: &Recipient,
    payment_type: PaymentType,
) -> bool {
    match recipient.calculation {
        PaymentCalculation::TranchePrincipal { .. } => true,
        PaymentCalculation::ResidualCash => {
            payment_type == PaymentType::Principal
                || residual_debt_tranche(ctx, recipient).is_some()
        }
        _ => false,
    }
}

/// The debt class a [`PaymentCalculation::ResidualCash`] recipient pays, if any.
///
/// A note is owed its coupon and the return of its balance and nothing more, so
/// remaining cash directed at one is the turbo feature of ABS and CLO interest
/// waterfalls: it retires principal until the class is paid in full. An extra
/// coupon has no contractual basis and would not retire the balance the
/// coverage tests read. Equity is the residual holder and is excluded.
fn residual_debt_tranche<'a>(
    ctx: &AllocationContext<'a>,
    recipient: &Recipient,
) -> Option<&'a Tranche> {
    let (PaymentCalculation::ResidualCash, RecipientType::Tranche(id)) =
        (&recipient.calculation, &recipient.recipient_type)
    else {
        return None;
    };
    let tranche = &ctx.tranches.tranches[*ctx.tranche_index.get(id.as_str())?];
    (tranche.seniority != TrancheSeniority::Equity).then_some(tranche)
}

/// Balance a tranche still has outstanding within the current period: its
/// period-start balance less the principal earlier tiers paid it this period.
pub(super) fn unpaid_tranche_balance(
    tranche: &Tranche,
    tranche_balances: Option<&FxHashMap<String, Money>>,
    principal_paid_in_period: &FxHashMap<String, Money>,
) -> f64 {
    let current = tranche_balances
        .and_then(|b| b.get(tranche.id.as_str()))
        .copied()
        .unwrap_or(tranche.current_balance);
    // Invariant: `principal_paid_in_period` starts empty each period and is
    // populated by `record_in_period_principal` only when positive principal
    // is paid to a tranche, so an absent entry means 0.0 paid so far this
    // period (e.g., interest-only periods or tranches that receive no principal).
    let paid_this_period = principal_paid_in_period
        .get(tranche.id.as_str())
        .map(|m| m.amount())
        .unwrap_or(0.0);
    current.amount() - paid_this_period
}

/// Limit residual cash requested for a debt class to the balance it has left.
/// The cash above that stays available for the next recipient or tier.
fn cap_residual_at_debt_balance(
    ctx: &AllocationContext<'_>,
    recipient: &Recipient,
    requested: Money,
    principal_paid_in_period: &FxHashMap<String, Money>,
) -> Result<Money> {
    let Some(tranche) = residual_debt_tranche(ctx, recipient) else {
        return Ok(requested);
    };
    let unpaid = unpaid_tranche_balance(tranche, ctx.tranche_balances, principal_paid_in_period);
    Money::from_model_value(requested.amount().min(unpaid.max(0.0)), ctx.currency)
}

/// Record principal paid to a tranche within the current waterfall period so
/// later tiers (e.g. an OC/IC diversion into the senior principal tier) see
/// the post-payment balance instead of the stale period-start snapshot.
fn record_in_period_principal(
    ctx: &AllocationContext<'_>,
    principal_paid_in_period: &mut FxHashMap<String, Money>,
    recipient: &Recipient,
    paid: Money,
) -> Result<()> {
    if paid.amount() <= 0.0 {
        return Ok(());
    }
    let tranche_id = match &recipient.calculation {
        PaymentCalculation::TranchePrincipal { tranche_id, .. } => Some(tranche_id.as_str()),
        _ => residual_debt_tranche(ctx, recipient).map(|tranche| tranche.id.as_str()),
    };
    if let Some(tranche_id) = tranche_id {
        let entry = principal_paid_in_period
            .entry(tranche_id.to_string())
            .or_insert(Money::from((0_i64, ctx.currency)));
        *entry = entry.checked_add(paid)?;
    }
    Ok(())
}
