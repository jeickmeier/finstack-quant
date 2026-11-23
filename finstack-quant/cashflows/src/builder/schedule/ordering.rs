//! Canonical cashflow sorting, kind ranking, and accrual tie-breaking.

use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowAccrual};
use finstack_quant_core::dates::DayCount;

pub(super) fn kind_rank(kind: CFKind) -> u8 {
    match kind {
        CFKind::Fixed | CFKind::Stub | CFKind::Floating | CFKind::InflationCoupon => 0,
        CFKind::Fee
        | CFKind::CommitmentFee
        | CFKind::UsageFee
        | CFKind::FacilityFee
        | CFKind::LcFee
        | CFKind::FrontingFee => 1,
        CFKind::Amortization => 2,
        CFKind::PrePayment => 3,
        CFKind::DefaultedNotional => 4,
        CFKind::Recovery | CFKind::AccruedOnDefault => 5,
        CFKind::Pik => 6,
        CFKind::Notional | CFKind::RevolvingDraw | CFKind::RevolvingRepayment => 7,
        CFKind::InitialMarginPost
        | CFKind::InitialMarginReturn
        | CFKind::VariationMarginReceive
        | CFKind::VariationMarginPay
        | CFKind::MarginInterest
        | CFKind::CollateralSubstitutionIn
        | CFKind::CollateralSubstitutionOut => 8,
        _ => 9,
    }
}

/// Compare flows using schedule ordering semantics.
///
/// Multi-key ordering, applied in priority order:
///
/// 1. **Date** — earliest first.
/// 2. **Kind rank** — see [`kind_rank`]. Coupons sort before fees, fees before
///    amortization, amortization before prepayment, etc.
/// 3. **Currency** — lexicographic on the ISO code.
/// 4. **Amount** — exact Decimal value, without conversion to floating point.
/// 5. **Reset and principal dates** — `Option<Date>` ordering, with `None` first.
/// 6. **Accrual factor** — `f64::total_cmp`.
/// 7. **Rate** — `Option<f64>` ordering, with `None` first, then `total_cmp`.
/// 8. **Accrual metadata** — period, day count, then projected index rate.
/// 9. **Principal delta** — currency, then exact Decimal value.
/// 10. **Representation ties** — actual kind and Decimal encodings distinguish
///     rows whose preceding comparison keys are equal.
///
/// This is the canonical order downstream consumers
/// (`outstanding_by_date`, `pv_by_period`, accrual, dataframe export, etc.)
/// rely on. Because the comparator distinguishes every field that can vary
/// between two flows, it is a *total order*: the result is independent of input
/// order (deterministic across runs and across `Vec` reorderings).
///
/// A stable sort is used deliberately: `merge_cashflow_schedules` concatenates
/// already-sorted schedules, and the stable sort's run detection handles those
/// pre-sorted runs in near-linear time (an unstable sort re-partitions them).
pub(super) fn compare_flows(a: &CashFlow, b: &CashFlow) -> std::cmp::Ordering {
    a.get_date()
        .cmp(&b.get_date())
        .then_with(|| kind_rank(a.get_kind()).cmp(&kind_rank(b.get_kind())))
        .then_with(|| a.get_amount().currency().cmp(&b.get_amount().currency()))
        .then_with(|| {
            a.get_amount()
                .amount_decimal()
                .cmp(&b.get_amount().amount_decimal())
        })
        .then_with(|| a.get_reset_date().cmp(&b.get_reset_date()))
        .then_with(|| a.get_balance_date().cmp(&b.get_balance_date()))
        .then_with(|| a.get_accrual_factor().total_cmp(&b.get_accrual_factor()))
        .then_with(|| match (a.get_rate(), b.get_rate()) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => x.total_cmp(&y),
        })
        .then_with(|| {
            compare_accrual(a.get_accrual().as_ref(), b.get_accrual().as_ref()).then_with(
                || match (a.get_principal_delta(), b.get_principal_delta()) {
                    (Some(x), Some(y)) => x
                        .currency()
                        .cmp(&y.currency())
                        .then_with(|| x.amount_decimal().cmp(&y.amount_decimal())),
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                },
            )
        })
        // Retain the existing economic precedence before resolving representation ties.
        .then_with(|| (a.get_kind() as usize).cmp(&(b.get_kind() as usize)))
        .then_with(|| {
            a.get_amount()
                .amount_decimal()
                .serialize()
                .cmp(&b.get_amount().amount_decimal().serialize())
        })
        .then_with(|| {
            a.get_principal_delta()
                .map(|value| value.amount_decimal().serialize())
                .cmp(
                    &b.get_principal_delta()
                        .map(|value| value.amount_decimal().serialize()),
                )
        })
}

pub(super) fn compare_accrual(
    left: Option<&CashFlowAccrual>,
    right: Option<&CashFlowAccrual>,
) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(left), Some(right)) => left
            .get_start()
            .cmp(&right.get_start())
            .then_with(|| left.get_end().cmp(&right.get_end()))
            .then_with(|| left.get_nominal_dates().cmp(right.get_nominal_dates()))
            .then_with(|| {
                left.get_context()
                    .get_calendar_id()
                    .cmp(&right.get_context().get_calendar_id())
            })
            .then_with(|| {
                left.get_context()
                    .get_coupon_period()
                    .cmp(&right.get_context().get_coupon_period())
            })
            .then_with(|| {
                left.get_context()
                    .get_end_is_termination_date()
                    .cmp(&right.get_context().get_end_is_termination_date())
            })
            .then_with(|| {
                left.get_context()
                    .get_frequency()
                    .map(|tenor| tenor.to_string())
                    .cmp(
                        &right
                            .get_context()
                            .get_frequency()
                            .map(|tenor| tenor.to_string()),
                    )
            })
            .then_with(|| {
                left.get_context()
                    .get_bus_basis()
                    .cmp(&right.get_context().get_bus_basis())
            })
            .then_with(|| {
                day_count_rank(left.get_day_count()).cmp(&day_count_rank(right.get_day_count()))
            })
            .then_with(|| {
                match (
                    left.get_projected_index_rate(),
                    right.get_projected_index_rate(),
                ) {
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (Some(left), Some(right)) => left.total_cmp(&right),
                }
            }),
    }
}

/// Stable day-count rank for accrual tie-breaks.
///
/// Explicit, so the canonical schedule order does not depend on the
/// declaration order of `DayCount` variants.
pub(super) fn day_count_rank(day_count: DayCount) -> u8 {
    match day_count {
        DayCount::Act360 => 0,
        DayCount::Act365F => 1,
        DayCount::Act365L => 2,
        DayCount::Thirty360 => 3,
        DayCount::ThirtyE360 => 4,
        DayCount::ThirtyE360Isda => 5,
        DayCount::Nl365 => 6,
        DayCount::ActAct => 7,
        DayCount::ActActIsma => 8,
        DayCount::Bus252 => 9,
        DayCount::ActActAfb => 10,
        DayCount::Thirty360It => 11,
        DayCount::OneOne => 12,
        _ => 13,
    }
}
