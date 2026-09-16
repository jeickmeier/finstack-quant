//! Unified cashflow generation engine for revolving credit facilities.
//!
//! This module consolidates cashflow generation logic for both deterministic and stochastic modes,
//! producing `CashFlowSchedule` objects with optional embedded path data from 3-factor Monte Carlo simulations.
//!
//! # Architecture
//!
//! - **Deterministic Mode**: Generates cashflows from pre-defined draw/repay events
//! - **Stochastic Mode**: Generates cashflows from utilization/rate/spread trajectories
//! - **Unified Output**: Both modes produce `CashFlowSchedule` for consistent downstream processing
//!
//! # Three-Factor Model
//!
//! For stochastic paths, the engine processes trajectories from three correlated factors:
//! - **Utilization**: Mean-reverting usage rate of the facility
//! - **Short Rate**: Interest rate dynamics (fixed or floating)
//! - **Credit Spread**: Default risk premium

use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::cashflow::builder::{
    emit_revolving_credit_fees, periods::SchedulePeriod, CashFlowSchedule,
    RevolvingFeeEmissionConfig,
};
use finstack_quant_core::cashflow::{CFKind, CashFlow};

use super::types::{BaseRateSpec, DrawRepaySpec, RevolvingCredit};

/// Path data from 3-factor Monte Carlo simulation.
///
/// Contains the full trajectory of utilization, interest rates, and credit spreads
/// at each observation date (contractual accrual boundaries plus term-index
/// reset dates), enabling cashflow generation and survival probability
/// computation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ThreeFactorPathData {
    /// Utilization trajectory at each payment date [0, 1]
    pub utilization_path: Vec<f64>,
    /// Short rate trajectory (for floating rates)
    pub short_rate_path: Vec<f64>,
    /// Credit spread trajectory (for survival probability)
    pub credit_spread_path: Vec<f64>,
    /// Time points corresponding to each value: years from the commitment date
    /// on the ACT/365F model clock (`MC_CLOCK_DAY_COUNT`).
    pub time_points: Vec<f64>,
    /// Observation dates aligned with the trajectories: the contractual
    /// accrual boundaries plus every term-index reset date inside the facility
    /// life, sorted ascending. The name predates the separation of accrual,
    /// reset and adjusted payment dates.
    #[serde(with = "finstack_quant_core::wire::dates")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
    )]
    pub payment_dates: Vec<Date>,
    /// Whether `short_rate_path` was simulated by a stochastic rate process
    /// (Hull-White with σ > 0). When true the pricer discounts pathwise on
    /// the simulated bank account; when false the static discount curve is
    /// used (deterministic-forward or fixed-rate modes).
    #[serde(default)]
    pub stochastic_rates: bool,
}

impl ThreeFactorPathData {
    /// Validate the structural invariants of the path data.
    ///
    /// All four trajectories must be aligned 1:1 with `payment_dates`, there
    /// must be at least two points (a single point cannot define a period),
    /// `payment_dates` must be strictly increasing (the cashflow engine looks
    /// accrual boundaries up by binary search), and `time_points` must be
    /// strictly increasing. Downstream consumers
    /// index these vectors in lockstep — a length mismatch previously
    /// panicked deep inside the cashflow engine instead of returning an
    /// error.
    pub fn validate(&self) -> Result<()> {
        let n = self.payment_dates.len();
        if n < 2 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ThreeFactorPathData requires at least 2 payment dates, got {n}"
            )));
        }
        let lens = [
            ("utilization_path", self.utilization_path.len()),
            ("short_rate_path", self.short_rate_path.len()),
            ("credit_spread_path", self.credit_spread_path.len()),
            ("time_points", self.time_points.len()),
        ];
        for (name, len) in lens {
            if len != n {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ThreeFactorPathData {name} length ({len}) must match payment_dates ({n})"
                )));
            }
        }
        if self.payment_dates.windows(2).any(|w| w[1] <= w[0]) {
            return Err(finstack_quant_core::Error::Validation(
                "ThreeFactorPathData payment_dates must be strictly increasing".to_string(),
            ));
        }
        if self
            .time_points
            .windows(2)
            .any(|w| !w[0].is_finite() || !w[1].is_finite() || w[1] <= w[0])
        {
            return Err(finstack_quant_core::Error::Validation(
                "ThreeFactorPathData time_points must be finite and strictly increasing"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// Enhanced cashflow schedule with embedded 3-factor path data.
///
/// Wraps a standard `CashFlowSchedule` with optional path data for stochastic simulations.
/// This enables downstream pricers to access both cashflows and the underlying state trajectories.
#[derive(Debug, Clone)]
pub struct PathAwareCashflowSchedule {
    /// Standard cashflow schedule
    pub schedule: CashFlowSchedule,
    /// Optional 3-factor path data (present for stochastic paths)
    pub path_data: Option<ThreeFactorPathData>,
}

/// Unified cashflow generator for revolving credit facilities.
///
/// Supports both deterministic (event-based) and stochastic (path-based) cashflow generation
/// with a single implementation of core calculation logic.
pub struct CashflowEngine<'a> {
    /// Reference to the facility being priced
    facility: &'a RevolvingCredit,
    /// Optional market context for curve-based rate projections
    market: Option<&'a MarketContext>,
    /// Canonical periods retaining contractual accrual and adjusted payment dates.
    payment_periods: Vec<SchedulePeriod>,
    /// Reset dates for floating rate fixings (if applicable)
    reset_dates: Option<Vec<Date>>,
    /// Day count convention for accrual calculations
    day_count: DayCount,
    /// Valuation date (cashflows before this are excluded)
    as_of: Date,
    /// Optional historical fixing series for the floating rate index.
    /// When present and a reset date falls before `as_of`, the observed
    /// fixing is used instead of projecting from the forward curve.
    fixing_series: Option<&'a ScalarTimeSeries>,
}

impl<'a> CashflowEngine<'a> {
    fn schedule_meta(&self) -> crate::cashflow::builder::CashFlowMeta {
        crate::cashflow::builder::CashFlowMeta {
            projected_fixings: Vec::new(),
            representation: crate::cashflow::builder::CashflowRepresentation::Projected,
            calendar_ids: Vec::new(),
            facility_limit: Some(self.facility.commitment_amount),
            issue_date: Some(self.facility.commitment_date),
            maturity_date: None,
        }
    }

    /// Create a new cashflow engine.
    ///
    /// # Arguments
    ///
    /// * `facility` - The revolving credit facility
    /// * `market` - Optional market context for floating rate projections
    /// * `as_of` - Valuation date
    /// * `fixing_series` - Optional historical fixings for the floating rate index.
    ///   When provided and a reset date falls before `as_of`, the observed fixing
    ///   rate is used instead of projecting from the forward curve.
    ///
    /// # Returns
    ///
    /// A new engine instance ready to generate cashflows
    pub fn new(
        facility: &'a RevolvingCredit,
        market: Option<&'a MarketContext>,
        as_of: Date,
        fixing_series: Option<&'a ScalarTimeSeries>,
    ) -> Result<Self> {
        let payment_periods = super::utils::build_payment_periods(facility)?;
        let reset_dates = super::utils::build_reset_dates(facility)?;
        let day_count = facility.day_count;

        Ok(Self {
            facility,
            market,
            payment_periods,
            reset_dates,
            day_count,
            as_of,
            fixing_series,
        })
    }

    /// Generate deterministic cashflows (no path data).
    ///
    /// Uses the facility's `DrawRepaySpec::Deterministic` events to construct
    /// the cashflow schedule with intra-period event slicing.
    ///
    /// # Returns
    ///
    /// A schedule with no embedded path data
    pub fn generate_deterministic(&self) -> Result<PathAwareCashflowSchedule> {
        let schedule = self.build_deterministic_schedule()?;
        Ok(PathAwareCashflowSchedule {
            schedule,
            path_data: None,
        })
    }

    /// Generate cashflows for a single MC path from 3-factor model.
    ///
    /// Uses utilization, rate, and spread trajectories to generate cashflows
    /// period by period.
    ///
    /// # Arguments
    ///
    /// * `path_data` - 3-factor trajectories for this path
    ///
    /// # Returns
    ///
    /// A schedule with embedded path data
    pub fn generate_stochastic_path(
        &self,
        path_data: ThreeFactorPathData,
    ) -> Result<PathAwareCashflowSchedule> {
        path_data.validate()?;
        let schedule = self.build_path_schedule(&path_data)?;
        Ok(PathAwareCashflowSchedule {
            schedule,
            path_data: Some(path_data),
        })
    }

    /// Build deterministic cashflow schedule from draw/repay events.
    ///
    /// This is the core deterministic cashflow generation logic, migrated from
    /// the original `cashflows.rs::generate_deterministic_cashflows_internal`.
    fn build_deterministic_schedule(&self) -> Result<CashFlowSchedule> {
        let mut projected_fixings = Vec::new();
        let mut draw_repay_events = match &self.facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(events) => events.clone(),
            DrawRepaySpec::Stochastic(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Deterministic cashflows require DrawRepaySpec::Deterministic".to_string(),
                ));
            }
        };
        draw_repay_events.sort_by_key(|event| event.date);

        // Events dated on the commitment date are rejected outright: the
        // initial position on the commitment date is defined by
        // `drawn_amount` alone. The previous "dedup" special case skipped
        // the initial-draw outflow when a commitment-date draw event existed,
        // but the period balance replay and the terminal repayment still
        // added the event on top of `drawn_amount` — interest accrued on 2X
        // and 2X was repaid at maturity. One canonical semantic: encode the
        // initial draw in `drawn_amount`, date all events strictly after the
        // commitment date.
        if let Some(event) = draw_repay_events
            .iter()
            .find(|e| e.date <= self.facility.commitment_date)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit draw/repay event dated {} is on or before the commitment date \
                 ({}); the position at commitment is defined by drawn_amount — date events \
                 strictly after the commitment date",
                event.date, self.facility.commitment_date
            )));
        }

        let mut flows = Vec::new();
        let rc = RoundingContext::default();
        let ccy = self.facility.commitment_amount.currency();

        // Add initial draw at commitment_date (from lender perspective: negative cashflow)
        if self.facility.commitment_date > self.as_of
            && !rc.is_effectively_zero(self.facility.drawn_amount.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow::new(
                self.facility.commitment_date,
                None,
                self.facility.drawn_amount * -1.0,
                CFKind::Notional,
                0.0,
                None,
            ));
        }

        // Generate interest and fee cashflows with intra-period event slicing
        flows.reserve(self.payment_periods.len() * 4 + draw_repay_events.len() + 2);

        // Resolve forward curve once if floating rate (required for rate projection)
        let fwd_curve = match &self.facility.base_rate_spec {
            BaseRateSpec::Floating(spec) => {
                if let Some(market) = self.market {
                    Some(market.get_forward(&spec.index_id)?)
                } else {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Market context required for floating rate facility (index: {})",
                        spec.index_id
                    )));
                }
            }
            BaseRateSpec::Fixed { .. } => None,
        };

        // Term-index facilities re-fix the coupon at every reset date, so the
        // sub-period timeline must also be sliced on resets that fall inside
        // an accrual period (reset frequency shorter than payment frequency).
        // Overnight facilities compound daily fixings over the whole window.
        let slice_on_resets = match &self.facility.base_rate_spec {
            BaseRateSpec::Floating(spec) => {
                super::utils::resolved_overnight_compounding(spec)?.is_none()
            }
            BaseRateSpec::Fixed { .. } => false,
        };

        for (i, period) in self.payment_periods.iter().enumerate() {
            let period_start = period.accrual_start;
            let period_end = period.accrual_end;
            let payment_date = period.payment_date;

            // Apply as_of filtering for non-principal cashflows
            if payment_date <= self.as_of {
                continue;
            }

            // Build sub-period timeline with events
            // Events at period_end are excluded - they happen AFTER interest calculation
            let mut timeline = vec![period_start];
            for event in draw_repay_events.iter() {
                if event.date > period_start && event.date < period_end {
                    timeline.push(event.date);
                }
            }
            if slice_on_resets {
                if let Some(ref reset_grid) = self.reset_dates {
                    timeline.extend(
                        reset_grid
                            .iter()
                            .copied()
                            .filter(|&reset| reset > period_start && reset < period_end),
                    );
                }
            }
            timeline.push(period_end);
            timeline.sort();
            timeline.dedup();

            // Track balance through sub-periods. The replay routes every
            // event through `apply_draw_repay_event` so limit validation
            // (draw ≤ commitment, repay ≤ balance) also covers events dated
            // exactly on a period boundary — the in-period loop below only
            // sees strictly-interior events, so boundary-dated events would
            // otherwise bypass validation and let the balance exceed the
            // commitment (negative undrawn fees).
            let mut current_balance = if i == 0 {
                self.facility.drawn_amount
            } else {
                let mut balance = self.facility.drawn_amount;
                for event in draw_repay_events.iter() {
                    if event.date <= period_start {
                        balance = super::utils::apply_draw_repay_event(
                            balance,
                            event,
                            self.facility.commitment_amount,
                        )?;
                    }
                }
                balance
            };

            // Accumulators for aggregated accruals
            let mut total_interest = Money::from((0_i64, ccy));
            let mut total_commitment_fee = Money::from((0_i64, ccy));
            let mut total_usage_fee = Money::from((0_i64, ccy));
            let mut total_facility_fee = Money::from((0_i64, ccy));
            let mut total_accrual = 0.0;
            let mut reset_date_opt: Option<Date> = None;

            // Track weighted average rates for this period
            let mut weighted_interest_rate = 0.0;
            let mut weighted_commitment_fee_rate = 0.0;
            let mut weighted_usage_fee_rate = 0.0;

            for window in timeline.windows(2) {
                let sub_start = window[0];
                let sub_end = window[1];

                let dt =
                    self.day_count
                        .year_fraction(sub_start, sub_end, DayCountContext::default())?;
                total_accrual += dt;

                let current_undrawn = self
                    .facility
                    .commitment_amount
                    .checked_sub(current_balance)?;
                let utilization = if self.facility.commitment_amount.amount() > 0.0 {
                    current_balance.amount() / self.facility.commitment_amount.amount()
                } else {
                    0.0
                };

                // Determine reset date for floating rates
                let sub_reset_effective_date = match &self.facility.base_rate_spec {
                    BaseRateSpec::Floating(_) => {
                        if let Some(ref reset_grid) = self.reset_dates {
                            reset_grid
                                .iter()
                                .rev()
                                .find(|&&d| d <= sub_start)
                                .copied()
                                .or(Some(period_start))
                        } else {
                            Some(period_start)
                        }
                    }
                    BaseRateSpec::Fixed { .. } => None,
                };

                if reset_date_opt.is_none() {
                    reset_date_opt = match (&self.facility.base_rate_spec, sub_reset_effective_date)
                    {
                        (BaseRateSpec::Floating(spec), Some(date)) => {
                            Some(super::utils::floating_fixing_date(
                                spec,
                                date,
                                &self.facility.attributes,
                            )?)
                        }
                        _ => None,
                    };
                }

                let interest_rate = match &self.facility.base_rate_spec {
                    BaseRateSpec::Fixed { rate } => {
                        let interest = current_balance * (*rate * dt);
                        total_interest = total_interest.checked_add(interest)?;
                        *rate
                    }
                    BaseRateSpec::Floating(spec) => {
                        let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                        let reset_effective = sub_reset_effective_date.unwrap_or(period_start);
                        let fixing_date = super::utils::floating_fixing_date(
                            spec,
                            reset_effective,
                            &self.facility.attributes,
                        )?;
                        let overnight = super::utils::resolved_overnight_compounding(spec)?;

                        let coupon_rate = if overnight.is_some() {
                            let fwd = fwd_curve.as_ref().ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "forward curve required for floating rate".into(),
                                )
                            })?;
                            super::utils::project_revolver_floating_rate(
                                super::utils::RevolverFloatingProjection {
                                    accrual_start: sub_start,
                                    accrual_end: sub_end,
                                    as_of: self.as_of,
                                    spec,
                                    fwd: fwd.as_ref(),
                                    day_count: self.day_count,
                                    coupon_frequency: self.facility.frequency,
                                    currency: ccy,
                                    attributes: &self.facility.attributes,
                                    fixings: self.fixing_series,
                                },
                                Some(&mut projected_fixings),
                            )?
                        } else if fixing_date < self.as_of {
                            let fixing_rate =
                                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                    self.fixing_series,
                                    spec.index_id.as_ref(),
                                    fixing_date,
                                    self.as_of,
                                )?;
                            projected_fixings.push(crate::cashflow::fixings::ProjectedFixing {
                                series_id: format!("FIXING:{}", spec.index_id),
                                date: fixing_date,
                                value: Some(fixing_rate),
                            });
                            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                                fixing_rate,
                                &params,
                            )
                        } else {
                            let fwd = fwd_curve.as_ref().ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "forward curve required for floating rate".into(),
                                )
                            })?;
                            let index_rate =
                                crate::cashflow::builder::rate_helpers::project_index_rate(
                                    reset_effective,
                                    fwd.as_ref(),
                                )?;
                            projected_fixings.push(crate::cashflow::fixings::ProjectedFixing {
                                series_id: format!("FIXING:{}", spec.index_id),
                                date: fixing_date,
                                value: Some(index_rate),
                            });
                            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                                index_rate, &params,
                            )
                        };

                        let interest = current_balance * (coupon_rate * dt);
                        total_interest = total_interest.checked_add(interest)?;
                        coupon_rate
                    }
                };
                weighted_interest_rate += interest_rate * dt;

                let commitment_fee_bp = self.facility.fees.commitment_fee_bp(utilization);
                if commitment_fee_bp > 0.0 {
                    let commitment_fee = current_undrawn * (commitment_fee_bp * 1e-4 * dt);
                    total_commitment_fee = total_commitment_fee.checked_add(commitment_fee)?;
                    weighted_commitment_fee_rate += (commitment_fee_bp * 1e-4) * dt;
                }

                let usage_fee_bp = self.facility.fees.usage_fee_bp(utilization);
                if usage_fee_bp > 0.0 {
                    let usage_fee = current_balance * (usage_fee_bp * 1e-4 * dt);
                    total_usage_fee = total_usage_fee.checked_add(usage_fee)?;
                    weighted_usage_fee_rate += (usage_fee_bp * 1e-4) * dt;
                }

                if self.facility.fees.facility_fee_bp > 0.0 {
                    let facility_fee = self.facility.commitment_amount
                        * (self.facility.fees.facility_fee_bp * 1e-4 * dt);
                    total_facility_fee = total_facility_fee.checked_add(facility_fee)?;
                }

                // Apply events at sub_end (but not at period_end - those happen after interest)
                if sub_end != period_end {
                    for event in draw_repay_events.iter() {
                        if event.date == sub_end {
                            current_balance = super::utils::apply_draw_repay_event(
                                current_balance,
                                event,
                                self.facility.commitment_amount,
                            )?;
                        }
                    }
                }
            }

            // Post aggregated cashflows at period_end
            // For revolving credit with intra-period events, we use time-weighted average rates
            // The rate_base is the period start balance, so the formula amount = rate_base × rate × accrual
            // is approximate when there are draws/repays during the period
            let avg_interest_rate = if total_accrual > 0.0 {
                Some(weighted_interest_rate / total_accrual)
            } else {
                None
            };
            let avg_commitment_fee_rate =
                if total_accrual > 0.0 && weighted_commitment_fee_rate > 0.0 {
                    Some(weighted_commitment_fee_rate / total_accrual)
                } else {
                    None
                };
            let avg_usage_fee_rate = if total_accrual > 0.0 && weighted_usage_fee_rate > 0.0 {
                Some(weighted_usage_fee_rate / total_accrual)
            } else {
                None
            };

            if !rc.is_effectively_zero_money(total_interest.amount(), ccy) {
                flows.push(CashFlow::new(
                    payment_date,
                    reset_date_opt,
                    total_interest,
                    match &self.facility.base_rate_spec {
                        BaseRateSpec::Fixed { .. } => CFKind::Fixed,
                        BaseRateSpec::Floating(_) => CFKind::FloatReset,
                    },
                    total_accrual,
                    avg_interest_rate,
                ));
            }

            if !rc.is_effectively_zero_money(total_commitment_fee.amount(), ccy) {
                flows.push(CashFlow::new(
                    payment_date,
                    None,
                    total_commitment_fee,
                    CFKind::CommitmentFee,
                    total_accrual,
                    avg_commitment_fee_rate,
                ));
            }

            if !rc.is_effectively_zero_money(total_usage_fee.amount(), ccy) {
                flows.push(CashFlow::new(
                    payment_date,
                    None,
                    total_usage_fee,
                    CFKind::UsageFee,
                    total_accrual,
                    avg_usage_fee_rate,
                ));
            }

            if !rc.is_effectively_zero_money(total_facility_fee.amount(), ccy) {
                flows.push(CashFlow::new(
                    payment_date,
                    None,
                    total_facility_fee,
                    CFKind::FacilityFee,
                    total_accrual,
                    Some(self.facility.fees.facility_fee_bp * 1e-4),
                ));
            }
        }

        // Add principal flows from draw/repay events
        for event in draw_repay_events.iter() {
            if event.date > self.as_of {
                flows.push(CashFlow::new(
                    event.date,
                    None,
                    if event.is_draw {
                        event.amount * -1.0
                    } else {
                        event.amount
                    },
                    CFKind::Notional,
                    0.0,
                    None,
                ));
            }
        }

        // Add terminal repayment. Same validated replay as the period
        // balances — maturity-dated events are boundary events too.
        let mut final_balance = self.facility.drawn_amount;
        for event in draw_repay_events.iter() {
            if event.date < self.facility.maturity {
                final_balance = super::utils::apply_draw_repay_event(
                    final_balance,
                    event,
                    self.facility.commitment_amount,
                )?;
            }
        }

        let mut final_balance_for_terminal = final_balance;
        for event in draw_repay_events.iter() {
            if event.date == self.facility.maturity {
                final_balance_for_terminal = super::utils::apply_draw_repay_event(
                    final_balance_for_terminal,
                    event,
                    self.facility.commitment_amount,
                )?;
            }
        }

        let terminal_payment_date = self
            .payment_periods
            .last()
            .map(|period| period.payment_date)
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        if terminal_payment_date > self.as_of
            && !rc.is_effectively_zero(final_balance_for_terminal.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow::new(
                terminal_payment_date,
                None,
                final_balance_for_terminal,
                CFKind::Notional,
                0.0,
                None,
            ));
        }

        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.facility.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(Money::from((
                    0_i64,
                    self.facility.commitment_amount.currency(),
                ))),
                meta: crate::cashflow::builder::CashFlowMeta {
                    projected_fixings,
                    ..self.schedule_meta()
                },
            },
        ))
    }

    /// Last reset-effective date at or before `date` (falls back to `date`
    /// itself when the facility has no reset grid).
    fn reset_effective_at(&self, date: Date) -> Date {
        self.reset_dates
            .as_ref()
            .and_then(|dates| dates.iter().rev().find(|&&reset| reset <= date).copied())
            .unwrap_or(date)
    }

    /// Build cashflow schedule from 3-factor path trajectory.
    ///
    /// The path is observed on the facility's observation grid (contractual
    /// accrual boundaries plus term-index reset dates). Utilization is read at
    /// each accrual boundary; a term-index coupon is re-fixed at every reset
    /// date inside the period, so reset frequencies shorter than the payment
    /// frequency are honoured exactly as in the deterministic engine.
    fn build_path_schedule(&self, path: &ThreeFactorPathData) -> Result<CashFlowSchedule> {
        let observation_index = |date: Date| -> Result<usize> {
            path.payment_dates.binary_search(&date).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "RevolvingCredit path observation grid does not contain the date {date}"
                ))
            })
        };
        let mut flows = Vec::new();
        let rc = RoundingContext::default();
        let ccy = self.facility.commitment_amount.currency();

        // Overnight facilities compound the forward curve over the whole
        // period. Term-index facilities on a stochastic (Hull-White) short-rate
        // path rebuild the index fixing as `F_index(t) + (r_t − f_OIS(t))`: the
        // simulated rate is the OIS numeraire rate, so the deterministic
        // index-over-OIS basis must be added back or the index forward curve
        // is silently ignored.
        let (overnight_fwd, term_basis_curves) = match &self.facility.base_rate_spec {
            BaseRateSpec::Floating(spec) => {
                let overnight = super::utils::resolved_overnight_compounding(spec)?.is_some();
                match self.market {
                    Some(market) if overnight => {
                        (Some(market.get_forward(spec.index_id.as_str())?), None)
                    }
                    Some(market) if path.stochastic_rates => (
                        None,
                        Some((
                            market.get_forward(spec.index_id.as_str())?,
                            market.get_discount(self.facility.discount_curve_id.as_str())?,
                        )),
                    ),
                    None if path.stochastic_rates => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "Market context required to project the '{}' index over a \
                             stochastic short-rate path",
                            spec.index_id
                        )));
                    }
                    Some(_) | None => (None, None),
                }
            }
            BaseRateSpec::Fixed { .. } => (None, None),
        };

        // Add initial draw at commitment_date (from lender perspective: negative cashflow)
        if self.facility.commitment_date > self.as_of
            && !rc.is_effectively_zero(self.facility.drawn_amount.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow::new(
                self.facility.commitment_date,
                None,
                self.facility.drawn_amount * -1.0,
                CFKind::Notional,
                0.0,
                None,
            ));
        }

        // Track previous utilization for principal flows
        let mut prev_utilization = if path.payment_dates[0] <= self.as_of {
            path.utilization_path[0].clamp(0.0, 1.0)
        } else {
            self.facility.utilization_rate()
        };

        // Process each contractual accrual period using path data observed on
        // its unadjusted boundaries. Payment adjustment changes settlement,
        // never the accrual interval.
        for period in self.payment_periods.iter() {
            let period_start = period.accrual_start;
            let period_end = period.accrual_end;
            let payment_date = period.payment_date;
            let idx_start = observation_index(period_start)?;
            let idx_end = observation_index(period_end)?;

            let utilization_start = path.utilization_path[idx_start].clamp(0.0, 1.0);
            let utilization_end = path.utilization_path[idx_end].clamp(0.0, 1.0);

            // Use average utilization for interest calculation (time-weighted approximation).
            // This better captures the balance evolution within each period when utilization
            // changes between period start and end, avoiding systematic underestimation of
            // interest when utilization is rising.
            let avg_utilization = (utilization_start + utilization_end) / 2.0;
            let drawn_balance = self.facility.commitment_amount * avg_utilization;
            let undrawn_balance = self.facility.commitment_amount * (1.0 - avg_utilization);

            let dt = self.day_count.year_fraction(
                period_start,
                period_end,
                DayCountContext::default(),
            )?;

            // Contractual fixings override the simulated short rate once the
            // fixing date has passed. Future reset dates remain stochastic.
            let (interest, accrual, interest_rate, fixing_date) = match &self
                .facility
                .base_rate_spec
            {
                BaseRateSpec::Fixed { rate } => (drawn_balance * (*rate * dt), dt, *rate, None),
                BaseRateSpec::Floating(spec) => {
                    let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                    if let Some(fwd) = overnight_fwd.as_ref() {
                        let reset_effective = self.reset_effective_at(period_start);
                        let fixing_date = super::utils::floating_fixing_date(
                            spec,
                            reset_effective,
                            &self.facility.attributes,
                        )?;
                        let coupon_rate = super::utils::project_revolver_floating_rate(
                            super::utils::RevolverFloatingProjection {
                                accrual_start: period_start,
                                accrual_end: period_end,
                                as_of: self.as_of,
                                spec,
                                fwd: fwd.as_ref(),
                                day_count: self.day_count,
                                coupon_frequency: self.facility.frequency,
                                currency: ccy,
                                attributes: &self.facility.attributes,
                                fixings: self.fixing_series,
                            },
                            None,
                        )?;
                        (
                            drawn_balance * (coupon_rate * dt),
                            dt,
                            coupon_rate,
                            Some(fixing_date),
                        )
                    } else {
                        // Term index: slice the period on the observation grid
                        // so every reset inside it re-fixes the coupon.
                        let mut interest = Money::from((0_i64, ccy));
                        let mut weighted_rate = 0.0;
                        let mut accrual = 0.0;
                        let mut first_fixing = None;
                        for k in idx_start..idx_end {
                            let sub_start = path.payment_dates[k];
                            let sub_end = path.payment_dates[k + 1];
                            let reset_effective = self.reset_effective_at(sub_start);
                            let fixing_date = super::utils::floating_fixing_date(
                                spec,
                                reset_effective,
                                &self.facility.attributes,
                            )?;
                            first_fixing.get_or_insert(fixing_date);
                            let base_rate = if fixing_date < self.as_of {
                                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                    self.fixing_series,
                                    spec.index_id.as_ref(),
                                    fixing_date,
                                    self.as_of,
                                )?
                            } else {
                                let simulated =
                                    path.short_rate_path[observation_index(reset_effective)?];
                                match term_basis_curves.as_ref() {
                                    Some((fwd, disc)) => {
                                        simulated
                                            + super::utils::index_basis_at(
                                                reset_effective,
                                                self.facility.commitment_date.max(self.as_of),
                                                fwd.as_ref(),
                                                disc.as_ref(),
                                            )?
                                    }
                                    None => simulated,
                                }
                            };
                            let coupon_rate =
                                crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                                    base_rate, &params,
                                );
                            let sub_dt = self.day_count.year_fraction(
                                sub_start,
                                sub_end,
                                DayCountContext::default(),
                            )?;
                            interest =
                                interest.checked_add(drawn_balance * (coupon_rate * sub_dt))?;
                            weighted_rate += coupon_rate * sub_dt;
                            accrual += sub_dt;
                        }
                        let avg_rate = if accrual > 0.0 {
                            weighted_rate / accrual
                        } else {
                            0.0
                        };
                        (interest, accrual, avg_rate, first_fixing)
                    }
                }
            };

            if payment_date > self.as_of && !rc.is_effectively_zero_money(interest.amount(), ccy) {
                flows.push(CashFlow::new(
                    payment_date,
                    fixing_date,
                    interest,
                    match &self.facility.base_rate_spec {
                        BaseRateSpec::Fixed { .. } => CFKind::Fixed,
                        BaseRateSpec::Floating(_) => CFKind::FloatReset,
                    },
                    accrual,
                    Some(interest_rate),
                ));
            }

            // Calculate and emit fee cashflows using centralized functions.
            // Use average utilization for fee tier determination to match the interest
            // calculation above and avoid tier-boundary artifacts.
            if payment_date > self.as_of {
                emit_revolving_credit_fees(
                    &mut flows,
                    &RevolvingFeeEmissionConfig {
                        payment_date,
                        drawn_balance: drawn_balance.amount(),
                        undrawn_balance: undrawn_balance.amount(),
                        commitment_amount: self.facility.commitment_amount.amount(),
                        commitment_fee_bp: self.facility.fees.commitment_fee_bp(avg_utilization),
                        usage_fee_bp: self.facility.fees.usage_fee_bp(avg_utilization),
                        facility_fee_bp: self.facility.fees.facility_fee_bp,
                        year_fraction: dt,
                        currency: ccy,
                    },
                )?;
            }

            // Handle principal flows from utilization changes. Interest uses
            // average start/end utilization, so book the matching funding leg
            // at the midpoint of the simulated interval rather than deferring
            // it to period end.
            //
            // Convention: the simulated path only observes utilization at
            // period boundaries, so the exact timing of the change within the
            // period is unknown. Midpoint booking is the unbiased choice for
            // a change occurring uniformly within the period and keeps the
            // funding leg aligned with the average-utilization interest
            // accrual above. This intentionally differs from the
            // deterministic engine, which posts principal exactly on
            // contractual draw/repay event dates.
            //
            // For the period containing the valuation date the simulated
            // interval starts at `as_of` (the utilization there is the known
            // t₀ state), so the midpoint is taken over `[as_of, period_end]`;
            // taking it over the full period would drop the funding leg for
            // a valuation past the period midpoint while keeping its interest
            // and terminal repayment.
            let utilization_change = utilization_end - prev_utilization;
            let simulated_from = period_start.max(self.as_of);
            let half_days = ((period_end - simulated_from).whole_days() / 2).max(1);
            let principal_date = simulated_from + time::Duration::days(half_days);
            if principal_date > self.as_of
                && utilization_change.abs() > super::UTILIZATION_CHANGE_THRESHOLD
            {
                let principal_change = self.facility.commitment_amount * utilization_change;
                // Draw (increase) is negative for lender, repay (decrease) is positive
                flows.push(CashFlow::new(
                    principal_date,
                    None,
                    principal_change * -1.0,
                    CFKind::Notional,
                    0.0,
                    None,
                ));
            }

            prev_utilization = utilization_end;
        }

        // Terminal repayment of outstanding balance
        let final_utilization = path
            .utilization_path
            .last()
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let final_balance = self.facility.commitment_amount * final_utilization;

        let terminal_payment_date = self
            .payment_periods
            .last()
            .map(|period| period.payment_date)
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        if terminal_payment_date > self.as_of
            && !rc.is_effectively_zero(final_balance.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow::new(
                terminal_payment_date,
                None,
                final_balance,
                CFKind::Notional,
                0.0,
                None,
            ));
        }

        Ok(crate::cashflow::traits::schedule_from_classified_flows(
            flows,
            self.facility.day_count,
            crate::cashflow::traits::ScheduleBuildOpts {
                notional_hint: Some(Money::from((
                    0_i64,
                    self.facility.commitment_amount.currency(),
                ))),
                meta: self.schedule_meta(),
            },
        ))
    }
}

/// Calculate the outstanding drawn balance at a given date considering draw/repay events.
///
/// This helper function simulates the drawn balance evolution based on the
/// deterministic schedule of draws and repayments.
///
/// **Note**: This is primarily intended for testing and property-based validation.
///
/// # Arguments
/// * `facility` - The revolving credit facility
/// * `target_date` - The date at which to calculate the balance
///
/// # Returns
/// The outstanding drawn balance at the target date
pub fn calculate_drawn_balance_at_date(
    facility: &RevolvingCredit,
    target_date: Date,
) -> Result<Money> {
    let mut draw_repay_events = match &facility.draw_repay_spec {
        DrawRepaySpec::Deterministic(events) => events.clone(),
        DrawRepaySpec::Stochastic(_) => {
            return Err(finstack_quant_core::Error::Validation(
                "calculate_drawn_balance_at_date requires DrawRepaySpec::Deterministic".to_string(),
            ));
        }
    };
    draw_repay_events.sort_by_key(|event| event.date);

    let mut balance = facility.drawn_amount;

    // Apply all events up to the target date
    for event in draw_repay_events.iter() {
        if event.date <= target_date {
            balance =
                super::utils::apply_draw_repay_event(balance, event, facility.commitment_amount)?;
        }
    }

    Ok(balance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::revolving_credit::{BaseRateSpec, RevolvingCreditFees};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, Tenor};
    use time::Month;

    #[test]
    fn terminal_principal_uses_adjusted_payment_date_without_extra_accrual() {
        let start = Date::from_calendar_date(2026, Month::January, 3).expect("date");
        let maturity = Date::from_calendar_date(2027, Month::January, 3).expect("date");
        let adjusted = Date::from_calendar_date(2027, Month::January, 4).expect("date");
        let facility = RevolvingCredit::builder()
            .id("RC-BDC-BOUNDARY".into())
            .commitment_amount(Money::from((1_000_000_i64, Currency::USD)))
            .drawn_amount(Money::from((1_000_000_i64, Currency::USD)))
            .commitment_date(start)
            .maturity(maturity)
            .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
            .day_count(DayCount::Act365F)
            .frequency(Tenor::annual())
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.0)
            .build()
            .expect("facility");

        let path = CashflowEngine::new(&facility, None, start, None)
            .expect("engine")
            .generate_deterministic()
            .expect("cashflows");
        let flows = path.schedule.get_flows();
        let terminal = flows
            .iter()
            .find(|flow| flow.kind == CFKind::Notional && flow.amount.amount() > 0.0)
            .expect("terminal principal");
        let interest = flows
            .iter()
            .find(|flow| flow.kind == CFKind::Fixed)
            .expect("interest");

        assert_eq!(terminal.date, adjusted);
        assert_eq!(interest.date, adjusted);
        assert!((interest.accrual_factor - 1.0).abs() < 1e-12);
        assert!((interest.amount.amount() - 50_000.0).abs() < 1e-8);
    }
}
