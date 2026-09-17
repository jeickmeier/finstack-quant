//! Delinquency bucket transitions for one pool asset.
//!
//! The pool-flow loop owns the cash accounting (interest on the performing
//! balance, charge-offs into the recovery queue, servicer advances); this
//! module only rolls the buckets.

use crate::instruments::fixed_income::structured_credit::types::DelinquencyModel;

/// Balances moved by one payment period of bucket transitions.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct PeriodOutcome {
    /// Balance that went from current to the first bucket.
    pub(super) entered: f64,
    /// Balance that rolled out of the last bucket (charge-off).
    pub(super) charged_off: f64,
    /// Balance that returned to current without modification.
    pub(super) cured: f64,
    /// Balance that returned to current through a modification.
    pub(super) modified: f64,
}

/// Roll one asset's buckets through `months` monthly transitions.
///
/// Each month the entry `performing × monthly_entry_rate` moves into the
/// first bucket, then every bucket's balance is split simultaneously on its
/// beginning-of-month value: a modification share (if any) returns to
/// current, the rest rolls forward, cures, or stays. The roll out of the
/// last bucket is the charge-off.
///
/// # Arguments
///
/// * `buckets` - Bucket balances for the asset, updated in place.
/// * `performing` - Performing (current) balance, updated in place.
/// * `monthly_entry_rate` - Share of the performing balance that becomes
///   30 days delinquent each month, as a decimal.
/// * `months` - Monthly transitions in this payment period.
/// * `model` - Roll, cure and modification terms.
pub(super) fn roll_period(
    buckets: &mut [f64],
    performing: &mut f64,
    monthly_entry_rate: f64,
    months: u32,
    model: &DelinquencyModel,
) -> PeriodOutcome {
    let mut outcome = PeriodOutcome::default();
    let entry_rate = monthly_entry_rate.clamp(0.0, 1.0);
    let modification_share = model
        .modification
        .map_or(0.0, |spec| spec.share_of_delinquent.clamp(0.0, 1.0));
    for _ in 0..months.max(1) {
        let entry = (*performing).max(0.0) * entry_rate;
        *performing -= entry;
        outcome.entered += entry;

        let mut inflow = entry;
        let transitions = model.roll_rates.iter().zip(&model.cure_rates);
        for (slot, (roll_rate, cure_rate)) in buckets.iter_mut().zip(transitions) {
            let opening = slot.max(0.0);
            let modified = opening * modification_share;
            let remaining = opening - modified;
            let roll = remaining * roll_rate.clamp(0.0, 1.0);
            let cure = remaining * cure_rate.clamp(0.0, 1.0);
            let stay = (remaining - roll - cure).max(0.0);
            *slot = stay + inflow;
            inflow = roll;
            outcome.cured += cure;
            outcome.modified += modified;
            *performing += cure + modified;
        }
        outcome.charged_off += inflow;
    }
    outcome
}

/// Total delinquent balance across an asset's buckets.
pub(super) fn delinquent_balance(buckets: &[f64]) -> f64 {
    buckets.iter().map(|balance| balance.max(0.0)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_rolls_charges_off_after_the_last_bucket() {
        let model = DelinquencyModel::new(vec![1.0, 1.0, 1.0], vec![0.0, 0.0, 0.0]);
        let mut buckets = vec![0.0; 3];
        let mut performing = 1_000_000.0;
        let mut charged = Vec::new();
        for _ in 0..5 {
            let outcome = roll_period(&mut buckets, &mut performing, 0.01, 1, &model);
            charged.push(outcome.charged_off);
        }
        // Entries of month 1 (10,000) charge off in month 4.
        assert_eq!(charged[0], 0.0);
        assert_eq!(charged[2], 0.0);
        assert!((charged[3] - 10_000.0).abs() < 1e-9);
        assert!((charged[4] - 9_900.0).abs() < 1e-9);
        assert!((performing - 1_000_000.0 * 0.99_f64.powi(5)).abs() < 1e-6);
    }

    #[test]
    fn cures_return_to_current_and_stays_remain_delinquent() {
        let model = DelinquencyModel::new(vec![0.5], vec![0.25]);
        let mut buckets = vec![100.0];
        let mut performing = 900.0;
        let outcome = roll_period(&mut buckets, &mut performing, 0.0, 1, &model);
        assert_eq!(outcome.charged_off, 50.0);
        assert_eq!(outcome.cured, 25.0);
        assert_eq!(buckets[0], 25.0);
        assert_eq!(performing, 925.0);
        assert_eq!(delinquent_balance(&buckets), 25.0);
    }
}
