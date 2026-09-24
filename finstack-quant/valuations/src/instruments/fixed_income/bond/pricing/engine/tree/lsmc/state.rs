//! Replay-template state: cash, balance, coupon, exercise and checkpoint records.

use super::*;

#[derive(Clone)]
pub(super) struct CashEvent {
    pub(super) amount_at_step: f64,
    pub(super) event_minus_step: f64,
}

#[derive(Clone, Copy)]
pub(super) struct BalanceEvent {
    pub(super) delta: f64,
}

#[derive(Clone)]
pub(super) struct AccrualClaim {
    pub(super) start: Date,
    pub(super) end: Date,
    pub(super) payment: Date,
    pub(super) day_count: finstack_quant_core::dates::DayCount,
    pub(super) amount: f64,
    pub(super) pik: bool,
}

#[derive(Clone, Copy)]
pub(super) struct DistributionEvent {
    pub(super) date: Date,
    pub(super) amount: f64,
}

#[derive(Clone, Copy)]
pub(super) struct ReturnFloorTemplate {
    pub(super) kind: ReturnFloorKind,
    pub(super) issue_price: f64,
    pub(super) issue_date: Date,
    pub(super) day_count: DayCount,
}

impl ReturnFloorTemplate {
    pub(super) fn redemption(
        self,
        date: Date,
        outstanding: f64,
        cumulative_cash: f64,
        cumulative_target_pv: f64,
        accrued: f64,
    ) -> Result<f64> {
        let required = match self.kind {
            ReturnFloorKind::Moic(multiple) => multiple * self.issue_price - cumulative_cash,
            ReturnFloorKind::Xirr(rate) => {
                let target = rate.as_decimal();
                let year_fraction = self.day_count.year_fraction(
                    self.issue_date,
                    date,
                    DayCountContext::default(),
                )?;
                (self.issue_price - cumulative_target_pv) * (1.0 + target).powf(year_fraction)
            }
        };
        if !required.is_finite() {
            return Err(Error::Validation(format!(
                "return-floor redemption at {date} is not finite"
            )));
        }
        Ok((required - accrued).max(outstanding).max(0.0))
    }

    pub(super) fn target_pv(self, date: Date, amount: f64) -> Result<f64> {
        match self.kind {
            ReturnFloorKind::Moic(_) => Ok(0.0),
            ReturnFloorKind::Xirr(rate) => {
                let year_fraction = self.day_count.year_fraction(
                    self.issue_date,
                    date,
                    DayCountContext::default(),
                )?;
                Ok(amount / (1.0 + rate.as_decimal()).powf(year_fraction))
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct FloatingCoupon {
    pub(super) reset_step: usize,
    pub(super) accrual_start_step: usize,
    pub(super) payment_step: usize,
    pub(super) compiled: CompiledFloatingCoupon,
    pub(super) rate_model: FloatingRateModel,
    pub(super) initial_notional: Option<f64>,
    pub(super) initial_term_rate: Option<f64>,
}

#[derive(Clone)]
pub(super) enum FloatingRateModel {
    Term(ObservedRateSource),
    Overnight(OvernightCoupon),
}

#[derive(Clone)]
pub(super) struct ConditionalRate {
    pub(super) observation_step: usize,
    pub(super) accrual: f64,
    pub(super) base_index_rate: f64,
    pub(super) base_discount_forward: f64,
    pub(super) conditional_discount_factors: Vec<f64>,
}

impl ConditionalRate {
    pub(super) fn rate(&self, path: &[RatesCreditPathState]) -> Result<f64> {
        let first_step = path
            .first()
            .map(|state| state.step)
            .ok_or_else(|| Error::internal("bond hazard LSMC conditional rate path is empty"))?;
        let offset = self.observation_step.checked_sub(first_step).ok_or_else(|| {
            Error::internal(format!(
                "bond hazard LSMC conditional rate step {} precedes sampled segment start {first_step}",
                self.observation_step
            ))
        })?;
        let state = path
            .get(offset)
            .filter(|state| state.step == self.observation_step)
            .ok_or_else(|| {
                Error::internal(
                    "bond hazard LSMC conditional rate step is outside the sampled path",
                )
            })?;
        let node_df = self
            .conditional_discount_factors
            .get(state.rate_node)
            .copied()
            .ok_or_else(|| {
                Error::internal(format!(
                    "bond hazard LSMC rate node {} is outside conditional-forward slice {}",
                    state.rate_node, self.observation_step
                ))
            })?;
        if !node_df.is_finite() || node_df <= 0.0 {
            return Err(Error::internal(
                "bond hazard LSMC conditional discount factor is not positive and finite",
            ));
        }
        let node_forward = (1.0 / node_df - 1.0) / self.accrual;
        let rate = self.base_index_rate + node_forward - self.base_discount_forward;
        if rate.is_finite() {
            Ok(rate)
        } else {
            Err(Error::internal(
                "bond hazard LSMC produced a non-finite conditional index rate",
            ))
        }
    }
}

#[derive(Clone)]
pub(super) enum OvernightRateSource {
    Fixed(f64),
    Conditional(ConditionalRate),
}

pub(super) type ObservedRateSource = OvernightRateSource;

impl OvernightRateSource {
    pub(super) fn rate(&self, path: &[RatesCreditPathState]) -> Result<f64> {
        match self {
            Self::Fixed(rate) => Ok(*rate),
            Self::Conditional(rate) => rate.rate(path),
        }
    }
}

#[derive(Clone)]
pub(super) struct OvernightCoupon {
    pub(super) sources: BTreeMap<(Date, u32), OvernightRateSource>,
    pub(super) max_history_steps: usize,
}

#[derive(Clone)]
pub(super) struct ExerciseDate {
    pub(super) date: Date,
    pub(super) calls: Vec<ExerciseCall>,
    pub(super) puts: Vec<CallPut>,
    pub(super) return_floor: bool,
}

#[derive(Clone)]
pub(super) struct ExerciseCall {
    pub(super) price_pct_of_par: f64,
    pub(super) make_whole: Option<MakeWholeExercise>,
}

#[derive(Clone)]
pub(super) enum MakeWholeExercise {
    Deterministic(f64),
    Conditional(usize),
}

#[derive(Clone)]
pub(super) struct MakeWholeClaim {
    pub(super) exercise_step: usize,
    pub(super) decision_index: usize,
    pub(super) basis_index: usize,
}

#[derive(Clone)]
pub(super) struct MakeWholeBasis {
    pub(super) interval_adjustments: Vec<f64>,
}

impl MakeWholeBasis {
    #[cfg(test)]
    pub(super) fn realized_reference_value(
        &self,
        exercise_step: usize,
        path: &[RatesCreditPathState],
        cash: &[f64],
    ) -> Result<f64> {
        if path.len() != cash.len() || path.len() != self.interval_adjustments.len() + 1 {
            return Err(Error::internal(
                "bond hazard LSMC make-whole replay does not match its reference grid",
            ));
        }
        let terminal = path.len() - 1;
        if exercise_step >= terminal {
            return Ok(0.0);
        }
        let mut value = cash[terminal];
        for step in (exercise_step..terminal).rev() {
            let discount = path[step].discount_to_next * self.interval_adjustments[step];
            if !discount.is_finite() || discount <= 0.0 {
                return Err(Error::internal(
                    "bond hazard LSMC make-whole reference discount is invalid",
                ));
            }
            value *= discount;
            if step > exercise_step {
                value += cash[step];
            }
        }
        if value.is_finite() && value >= 0.0 {
            Ok(value)
        } else {
            Err(Error::internal(
                "bond hazard LSMC make-whole reference value is invalid",
            ))
        }
    }
}

/// Per-path scratch arrays of [`ReplayTemplate::replay`], reused across the
/// paths one worker prices.
#[derive(Debug, Default)]
pub(super) struct ReplayBuffers {
    pub(super) step_cash: Vec<f64>,
    pub(super) outstanding_after_events: Vec<f64>,
}

#[derive(Clone)]
pub(super) struct ReplayTemplate {
    pub(super) times: Vec<f64>,
    pub(super) step_dates: Vec<Option<Date>>,
    pub(super) static_cash: Vec<Vec<CashEvent>>,
    pub(super) balance_events: Vec<Vec<BalanceEvent>>,
    pub(super) floating: Vec<FloatingCoupon>,
    pub(super) floating_reset_ids: Vec<Vec<usize>>,
    pub(super) floating_accrual_start_ids: Vec<Vec<usize>>,
    pub(super) floating_payment_ids: Vec<Vec<usize>>,
    pub(super) static_accruals: Vec<AccrualClaim>,
    pub(super) static_distributions: Vec<Vec<DistributionEvent>>,
    pub(super) exercise: Vec<Vec<ExerciseDate>>,
    pub(super) decision_steps: Vec<usize>,
    pub(super) initial_outstanding: f64,
    pub(super) redemption_step: Option<usize>,
    pub(super) call_friction_cents: f64,
    pub(super) recovery_rate: f64,
    pub(super) return_floor: Option<ReturnFloorTemplate>,
    pub(super) historical_distribution_cash: f64,
    pub(super) historical_distribution_target_pv: f64,
    pub(super) make_whole_claims: Vec<MakeWholeClaim>,
    pub(super) make_whole_bases: Vec<MakeWholeBasis>,
    pub(super) max_rate_history_steps: usize,
}

#[derive(Default)]
pub(super) struct FloatingBuild {
    pub(super) reset: Option<Date>,
    pub(super) start: Option<Date>,
    pub(super) end: Option<Date>,
    pub(super) payment: Option<Date>,
    pub(super) day_count: Option<finstack_quant_core::dates::DayCount>,
    pub(super) accrual: f64,
    pub(super) base_index_rate: Option<f64>,
}

pub(super) type FloatingRuntimeState = FloatingCouponReplayState;

#[derive(Clone)]
pub(super) struct LiveFloatingCheckpoint {
    pub(super) coupon_id: usize,
    pub(super) state: FloatingRuntimeState,
}

#[derive(Clone)]
pub(super) struct ProductCheckpoint {
    pub(super) outstanding: f64,
    pub(super) cumulative_distribution_cash: f64,
    pub(super) cumulative_distribution_target_pv: f64,
    pub(super) live_floating: SmallVec<[LiveFloatingCheckpoint; 1]>,
}

#[derive(Clone)]
pub(super) struct TrainingCheckpoint {
    pub(super) factor: RatesCreditPathCheckpoint,
    pub(super) product: ProductCheckpoint,
}

pub(super) struct BlockPathRecord {
    pub(super) snapshots: Vec<DecisionSnapshot>,
    pub(super) terminal: Option<DecisionSnapshot>,
    pub(super) step_states: Vec<StepReplay>,
}

#[derive(Clone)]
pub(super) struct ReplayCursor {
    pub(super) outstanding: f64,
    pub(super) cumulative_distribution_cash: f64,
    pub(super) cumulative_distribution_target_pv: f64,
    pub(super) floating: Vec<FloatingRuntimeState>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct StepReplay {
    pub(super) current_cash: f64,
    pub(super) reference_cash: f64,
    pub(super) outstanding: f64,
    pub(super) cumulative_distribution_cash: f64,
    pub(super) cumulative_distribution_target_pv: f64,
}

pub(super) struct ExerciseInputs<'a> {
    pub(super) bond: &'a Bond,
    pub(super) step: usize,
    pub(super) path: &'a [RatesCreditPathState],
    pub(super) outstanding: f64,
    pub(super) locked_coupon: f64,
    pub(super) coupon_states: &'a [FloatingRuntimeState],
    pub(super) cumulative_distribution_cash: f64,
    pub(super) cumulative_distribution_target_pv: f64,
    pub(super) make_whole_policies: Option<&'a [RegressionPolicy]>,
}
