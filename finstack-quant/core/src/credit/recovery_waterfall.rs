//! Absolute-priority recovery waterfall for restructuring claims.
//!
//! All amounts use one caller-defined unit. The kernel performs no currency
//! conversion. `estate_value` is the total distributable estate **including**
//! pledged collateral. Net collateral is therefore allocated first and
//! deducted from the estate before the remaining value is distributed by
//! priority. This convention prevents collateral from being counted twice.
//! Conservation comparisons use a scale-aware tolerance of `64 × f64::EPSILON`;
//! accepted rounding residue is removed deterministically from the last
//! positive allocation in priority/input order.

use std::collections::HashSet;

use crate::math::NeumaierAccumulator;
use crate::{Error, Result};

const CONSERVATION_REL_TOLERANCE: f64 = 64.0 * f64::EPSILON;

/// A claim participating in a recovery waterfall.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RecoveryClaim {
    /// Stable claim identifier.
    pub id: String,
    /// Caller-defined seniority label used for reporting.
    pub seniority: String,
    /// Absolute-priority rank; lower values recover before higher values.
    pub priority: u32,
    /// Principal amount.
    pub principal: f64,
    /// Accrued amount.
    pub accrued: f64,
    /// Allowed penalties or fees.
    pub penalties: f64,
    /// Gross value of collateral pledged exclusively to this claim.
    pub collateral_value: Option<f64>,
    /// Collateral haircut in the inclusive range `[0, 1]`.
    pub collateral_haircut: f64,
}

impl RecoveryClaim {
    /// Total allowed claim amount: principal plus accrued amounts and penalties.
    #[must_use]
    pub fn total_claim(&self) -> f64 {
        self.principal + self.accrued + self.penalties
    }
}

/// Recovery allocated to one claim.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RecoveryAllocation {
    /// Stable claim identifier.
    pub id: String,
    /// Caller-defined seniority label.
    pub seniority: String,
    /// Absolute-priority rank.
    pub priority: u32,
    /// Total allowed claim amount.
    pub total_claim: f64,
    /// Recovery funded by net pledged collateral.
    pub collateral_recovery: f64,
    /// Recovery funded by the residual general estate.
    pub general_recovery: f64,
    /// Total recovery from collateral and the general estate.
    pub total_recovery: f64,
    /// Total recovery divided by the allowed claim; zero for a zero claim.
    pub recovery_rate: f64,
    /// Unrecovered allowed claim amount.
    pub deficiency: f64,
}

/// Result of allocating a distributable estate across claims.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RecoveryWaterfallResult {
    /// Total value distributed to claims.
    pub total_distributed: f64,
    /// Estate value remaining after all allowed claims are satisfied.
    pub undistributed_estate: f64,
    /// Whether the residual estate was allocated in absolute-priority order.
    pub apr_satisfied: bool,
    /// Per-claim allocations ordered by priority, then original input order.
    pub allocations: Vec<RecoveryAllocation>,
}

/// Allocate an estate across claims using collateral and absolute priority.
///
/// Net collateral is `collateral_value * (1 - collateral_haircut)`, capped at
/// the allowed claim amount. Because `estate_value` includes collateral, the
/// aggregate net collateral entitlement must not exceed the estate. After
/// collateral is deducted, each priority class receives residual estate in
/// rank order. Claims at the same priority share an insufficient class
/// allocation pro rata by their remaining deficiencies.
///
/// # Errors
///
/// Returns [`Error::Validation`] when an amount is negative or non-finite, an
/// identifier or seniority label is empty, a haircut lies outside `[0, 1]`,
/// claim totals overflow, or net collateral exceeds the inclusive estate.
///
/// # Arguments
///
/// * `estate_value` - Total distributable estate in the claims' monetary
///   units, before collateral and priority allocations.
/// * `claims` - Recovery claims with identifiers, seniority, exposure, and
///   collateral data. Claims at equal priority share a shortfall pro rata.
pub fn allocate_recovery(
    estate_value: f64,
    claims: &[RecoveryClaim],
) -> Result<RecoveryWaterfallResult> {
    validate_non_negative_finite("estate_value", estate_value)?;

    let mut ordered = Vec::with_capacity(claims.len());
    let mut seen_ids = HashSet::new();
    seen_ids.try_reserve(claims.len()).map_err(|error| {
        Error::Internal(format!(
            "could not reserve {} recovery claim identifiers: {error}",
            claims.len()
        ))
    })?;
    let mut total_collateral_sum = NeumaierAccumulator::new();
    for (input_index, claim) in claims.iter().enumerate() {
        validate_claim(claim)?;
        let trimmed_id = claim.id.trim();
        if !seen_ids.insert(trimmed_id) {
            return Err(validation_error(format!(
                "duplicate recovery claim id after trimming: '{trimmed_id}'"
            )));
        }
        let total_claim = claim.total_claim();
        if !total_claim.is_finite() {
            return Err(validation_error(format!(
                "claim '{}' total amount must be finite",
                claim.id
            )));
        }
        let collateral_recovery = claim
            .collateral_value
            .map(|value| value * (1.0 - claim.collateral_haircut))
            .unwrap_or(0.0)
            .min(total_claim);
        total_collateral_sum.add(collateral_recovery);
        if !total_collateral_sum.current().is_finite() {
            return Err(validation_error(
                "aggregate collateral recovery must be finite",
            ));
        }
        ordered.push((input_index, claim, total_claim, collateral_recovery));
    }

    let mut total_collateral = total_collateral_sum.total();
    let collateral_tolerance = comparison_tolerance(total_collateral, estate_value);
    if total_collateral > estate_value + collateral_tolerance {
        return Err(validation_error(format!(
            "net collateral recovery ({total_collateral}) exceeds estate_value ({estate_value}); \
             estate_value must include pledged collateral"
        )));
    }

    ordered.sort_by_key(|(input_index, claim, _, _)| (claim.priority, *input_index));
    if total_collateral > estate_value {
        let mut excess = total_collateral - estate_value;
        for (_, _, _, collateral_recovery) in ordered.iter_mut().rev() {
            let reduction = excess.min(*collateral_recovery);
            *collateral_recovery -= reduction;
            excess -= reduction;
            if excess == 0.0 {
                break;
            }
        }
        total_collateral = estate_value;
    }
    let mut allocations = ordered
        .iter()
        .map(|(_, claim, total_claim, collateral_recovery)| {
            allocation_from_claim(claim, *total_claim, *collateral_recovery)
        })
        .collect::<Vec<_>>();

    let mut residual_estate = estate_value - total_collateral;
    let mut group_start = 0;
    while group_start < allocations.len() && residual_estate > 0.0 {
        let priority = allocations[group_start].priority;
        let mut group_end = group_start + 1;
        while group_end < allocations.len() && allocations[group_end].priority == priority {
            group_end += 1;
        }

        let mut group_deficiency_sum = NeumaierAccumulator::new();
        for allocation in &allocations[group_start..group_end] {
            group_deficiency_sum.add(allocation.deficiency);
        }
        let group_deficiency = group_deficiency_sum.total();
        if !group_deficiency.is_finite() {
            return Err(validation_error(
                "aggregate priority-class deficiency must be finite",
            ));
        }

        if group_deficiency <= residual_estate {
            for allocation in &mut allocations[group_start..group_end] {
                allocation.general_recovery = allocation.deficiency;
            }
            residual_estate -= group_deficiency;
        } else if group_deficiency > 0.0 {
            let class_estate = residual_estate;
            let mut class_remaining = class_estate;
            let group = &mut allocations[group_start..group_end];
            let group_len = group.len();
            for (index, allocation) in group.iter_mut().enumerate() {
                let general_recovery = if index + 1 == group_len {
                    class_remaining.min(allocation.deficiency)
                } else {
                    (class_estate * (allocation.deficiency / group_deficiency))
                        .min(allocation.deficiency)
                        .min(class_remaining)
                };
                allocation.general_recovery = general_recovery;
                class_remaining -= general_recovery;
            }
            residual_estate = 0.0;
        }

        group_start = group_end;
    }

    for allocation in &mut allocations {
        finalize_allocation(allocation);
    }

    let mut total_distributed = sum_recoveries(&allocations);
    let distribution_tolerance = comparison_tolerance(total_distributed, estate_value);
    if total_distributed > estate_value + distribution_tolerance {
        return Err(validation_error(format!(
            "aggregate recovery ({total_distributed}) exceeds estate_value ({estate_value})"
        )));
    }
    if total_distributed > estate_value {
        reduce_last_recovery(&mut allocations, total_distributed - estate_value);
        total_distributed = sum_recoveries(&allocations);
    }
    let undistributed_estate = (estate_value - total_distributed).max(0.0);
    let conserved = compensated_sum([total_distributed, undistributed_estate]);
    if (conserved - estate_value).abs() > comparison_tolerance(conserved, estate_value) {
        return Err(validation_error(format!(
            "recovery conservation failed: distributed ({total_distributed}) plus undistributed \
             ({undistributed_estate}) does not equal estate_value ({estate_value})"
        )));
    }

    Ok(RecoveryWaterfallResult {
        total_distributed,
        undistributed_estate,
        apr_satisfied: true,
        allocations,
    })
}

fn allocation_from_claim(
    claim: &RecoveryClaim,
    total_claim: f64,
    collateral_recovery: f64,
) -> RecoveryAllocation {
    RecoveryAllocation {
        id: claim.id.clone(),
        seniority: claim.seniority.clone(),
        priority: claim.priority,
        total_claim,
        collateral_recovery,
        general_recovery: 0.0,
        total_recovery: collateral_recovery,
        recovery_rate: if total_claim > 0.0 {
            collateral_recovery / total_claim
        } else {
            0.0
        },
        deficiency: total_claim - collateral_recovery,
    }
}

fn finalize_allocation(allocation: &mut RecoveryAllocation) {
    let total_recovery = allocation.collateral_recovery + allocation.general_recovery;
    if total_recovery > allocation.total_claim {
        let mut excess = total_recovery - allocation.total_claim;
        let general_reduction = excess.min(allocation.general_recovery);
        allocation.general_recovery -= general_reduction;
        excess -= general_reduction;
        allocation.collateral_recovery -= excess.min(allocation.collateral_recovery);
    }
    allocation.total_recovery = allocation.collateral_recovery + allocation.general_recovery;
    allocation.recovery_rate = if allocation.total_claim > 0.0 {
        allocation.total_recovery / allocation.total_claim
    } else {
        0.0
    };
    allocation.deficiency = (allocation.total_claim - allocation.total_recovery).max(0.0);
}

fn sum_recoveries(allocations: &[RecoveryAllocation]) -> f64 {
    compensated_sum(
        allocations
            .iter()
            .map(|allocation| allocation.total_recovery),
    )
}

fn reduce_last_recovery(allocations: &mut [RecoveryAllocation], mut excess: f64) {
    for allocation in allocations.iter_mut().rev() {
        let general_reduction = excess.min(allocation.general_recovery);
        allocation.general_recovery -= general_reduction;
        excess -= general_reduction;

        let collateral_reduction = excess.min(allocation.collateral_recovery);
        allocation.collateral_recovery -= collateral_reduction;
        excess -= collateral_reduction;
        finalize_allocation(allocation);
        if excess == 0.0 {
            break;
        }
    }
}

fn compensated_sum<I>(values: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let mut sum = NeumaierAccumulator::new();
    for value in values {
        sum.add(value);
    }
    sum.total()
}

fn comparison_tolerance(left: f64, right: f64) -> f64 {
    CONSERVATION_REL_TOLERANCE * left.abs().max(right.abs())
}

fn validate_claim(claim: &RecoveryClaim) -> Result<()> {
    if claim.id.trim().is_empty() {
        return Err(validation_error("recovery claim id must not be empty"));
    }
    if claim.seniority.trim().is_empty() {
        return Err(validation_error(
            "recovery claim seniority must not be empty",
        ));
    }
    validate_non_negative_finite("principal", claim.principal)?;
    validate_non_negative_finite("accrued", claim.accrued)?;
    validate_non_negative_finite("penalties", claim.penalties)?;
    if let Some(collateral_value) = claim.collateral_value {
        validate_non_negative_finite("collateral_value", collateral_value)?;
    }
    if !claim.collateral_haircut.is_finite() || !(0.0..=1.0).contains(&claim.collateral_haircut) {
        return Err(validation_error(format!(
            "collateral_haircut must be finite and in [0, 1], got {}",
            claim.collateral_haircut
        )));
    }
    Ok(())
}

fn validate_non_negative_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(validation_error(format!(
            "{field} must be finite and non-negative, got {value}"
        )))
    }
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}
