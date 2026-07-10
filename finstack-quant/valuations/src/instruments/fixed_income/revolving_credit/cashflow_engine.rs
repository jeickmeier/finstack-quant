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
    emit_revolving_credit_fees, CashFlowSchedule, Notional, RevolvingFeeEmissionConfig,
};
use finstack_quant_core::cashflow::{CFKind, CashFlow};

use super::types::{BaseRateSpec, DrawRepaySpec, RevolvingCredit};

/// Canonical sort rank for cashflow kinds within the same date.
///
/// Interest/reset flows first, then fees (commitment → facility → usage),
/// then structural flows (PIK, amortization), and finally notional exchanges last.
/// This ordering ensures deterministic and stochastic engines produce identical
/// cashflow sequences for the same dates.
fn cashflow_kind_rank(kind: &CFKind) -> usize {
    match kind {
        CFKind::Fixed => 0,
        CFKind::Stub => 1,
        CFKind::FloatReset => 2,
        CFKind::CommitmentFee => 3,
        CFKind::FacilityFee => 4,
        CFKind::UsageFee => 5,
        CFKind::Fee => 6,
        CFKind::PIK => 7,
        CFKind::Amortization => 8,
        CFKind::Notional => 9,
        // Any new/unclassified CFKind goes last
        _ => 100,
    }
}

/// Path data from 3-factor Monte Carlo simulation.
///
/// Contains the full trajectory of utilization, interest rates, and credit spreads
/// at each payment date, enabling cashflow generation and survival probability computation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ThreeFactorPathData {
    /// Utilization trajectory at each payment date [0, 1]
    pub utilization_path: Vec<f64>,
    /// Short rate trajectory (for floating rates)
    pub short_rate_path: Vec<f64>,
    /// Credit spread trajectory (for survival probability)
    pub credit_spread_path: Vec<f64>,
    /// Time points corresponding to each value (years from commitment)
    pub time_points: Vec<f64>,
    /// Payment dates aligned with trajectories
    #[schemars(with = "Vec<String>")]
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
    /// and `time_points` must be strictly increasing. Downstream consumers
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
    /// Payment schedule dates
    payment_dates: Vec<Date>,
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
        let payment_dates = super::utils::build_payment_dates(facility, false)?;
        let reset_dates = super::utils::build_reset_dates(facility)?;
        let day_count = facility.day_count;

        Ok(Self {
            facility,
            market,
            payment_dates,
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
        // Validate that we have a deterministic spec
        let draw_repay_events = match &self.facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(events) => events,
            DrawRepaySpec::Stochastic(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Deterministic cashflows require DrawRepaySpec::Deterministic".to_string(),
                ));
            }
        };

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
            flows.push(CashFlow {
                date: self.facility.commitment_date,
                reset_date: None,
                amount: self.facility.drawn_amount * -1.0,
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            });
        }

        // Generate interest and fee cashflows with intra-period event slicing
        flows.reserve(
            (self.payment_dates.len().saturating_sub(1)) * 4 + draw_repay_events.len() + 2,
        );

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

        for i in 0..(self.payment_dates.len() - 1) {
            let period_start = self.payment_dates[i];
            let period_end = self.payment_dates[i + 1];
            let payment_date = match &self.facility.base_rate_spec {
                BaseRateSpec::Floating(spec) => {
                    super::utils::floating_payment_date(spec, period_end)?
                }
                BaseRateSpec::Fixed { .. } => period_end,
            };

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
            let mut total_interest = Money::new(0.0, ccy);
            let mut total_commitment_fee = Money::new(0.0, ccy);
            let mut total_usage_fee = Money::new(0.0, ccy);
            let mut total_facility_fee = Money::new(0.0, ccy);
            let mut total_accrual = 0.0;
            let mut reset_date_opt: Option<Date> = None;

            // Track weighted average rates for this period
            let mut weighted_interest_rate = 0.0;
            let mut weighted_commitment_fee_rate = 0.0;
            let mut weighted_usage_fee_rate = 0.0;

            // Process each sub-period
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
                            Some(super::utils::floating_fixing_date(spec, date)?)
                        }
                        _ => None,
                    };
                }

                // Calculate interest for this sub-period
                let interest_rate = match &self.facility.base_rate_spec {
                    BaseRateSpec::Fixed { rate } => {
                        let interest = current_balance * (*rate * dt);
                        total_interest = total_interest.checked_add(interest)?;
                        *rate
                    }
                    BaseRateSpec::Floating(spec) => {
                        let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                        let reset_effective = sub_reset_effective_date.unwrap_or(period_start);
                        let fixing_date =
                            super::utils::floating_fixing_date(spec, reset_effective)?;

                        let coupon_rate = if reset_effective < self.as_of {
                            let fixing_rate =
                                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                    self.fixing_series,
                                    spec.index_id.as_ref(),
                                    fixing_date,
                                    self.as_of,
                                )?;
                            crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                                fixing_rate,
                                &params,
                            )
                        } else {
                            // Future reset: project from forward curve
                            let fwd = fwd_curve.as_ref().ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "forward curve required for floating rate".into(),
                                )
                            })?;
                            super::utils::project_floating_rate_with_curve(
                                reset_effective,
                                spec,
                                fwd.as_ref(),
                                &self.facility.attributes,
                            )?
                        };

                        let interest = current_balance * (coupon_rate * dt);
                        total_interest = total_interest.checked_add(interest)?;
                        coupon_rate
                    }
                };
                weighted_interest_rate += interest_rate * dt;

                // Calculate fees for this sub-period
                let commitment_fee_bps = self.facility.fees.commitment_fee_bps(utilization);
                if commitment_fee_bps > 0.0 {
                    let commitment_fee = current_undrawn * (commitment_fee_bps * 1e-4 * dt);
                    total_commitment_fee = total_commitment_fee.checked_add(commitment_fee)?;
                    weighted_commitment_fee_rate += (commitment_fee_bps * 1e-4) * dt;
                }

                let usage_fee_bps = self.facility.fees.usage_fee_bps(utilization);
                if usage_fee_bps > 0.0 {
                    let usage_fee = current_balance * (usage_fee_bps * 1e-4 * dt);
                    total_usage_fee = total_usage_fee.checked_add(usage_fee)?;
                    weighted_usage_fee_rate += (usage_fee_bps * 1e-4) * dt;
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
                flows.push(CashFlow {
                    date: payment_date,
                    reset_date: reset_date_opt,
                    amount: total_interest,
                    kind: match &self.facility.base_rate_spec {
                        BaseRateSpec::Fixed { .. } => CFKind::Fixed,
                        BaseRateSpec::Floating(_) => CFKind::FloatReset,
                    },
                    accrual_factor: total_accrual,
                    rate: avg_interest_rate,
                });
            }

            if !rc.is_effectively_zero_money(total_commitment_fee.amount(), ccy) {
                flows.push(CashFlow {
                    date: payment_date,
                    reset_date: None,
                    amount: total_commitment_fee,
                    kind: CFKind::CommitmentFee,
                    accrual_factor: total_accrual,
                    rate: avg_commitment_fee_rate,
                });
            }

            if !rc.is_effectively_zero_money(total_usage_fee.amount(), ccy) {
                flows.push(CashFlow {
                    date: payment_date,
                    reset_date: None,
                    amount: total_usage_fee,
                    kind: CFKind::UsageFee,
                    accrual_factor: total_accrual,
                    rate: avg_usage_fee_rate,
                });
            }

            if !rc.is_effectively_zero_money(total_facility_fee.amount(), ccy) {
                flows.push(CashFlow {
                    date: payment_date,
                    reset_date: None,
                    amount: total_facility_fee,
                    kind: CFKind::FacilityFee,
                    accrual_factor: total_accrual,
                    rate: Some(self.facility.fees.facility_fee_bp * 1e-4),
                });
            }
        }

        // Add principal flows from draw/repay events
        for event in draw_repay_events.iter() {
            if event.date > self.as_of {
                flows.push(CashFlow {
                    date: event.date,
                    reset_date: None,
                    amount: if event.is_draw {
                        event.amount * -1.0
                    } else {
                        event.amount
                    },
                    kind: CFKind::Notional,
                    accrual_factor: 0.0,
                    rate: None,
                });
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

        if self.facility.maturity > self.as_of
            && !rc.is_effectively_zero(final_balance_for_terminal.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow {
                date: self.facility.maturity,
                reset_date: None,
                amount: final_balance_for_terminal,
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            });
        }

        // Sort flows — canonical order shared with stochastic engine
        flows.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| cashflow_kind_rank(&a.kind).cmp(&cashflow_kind_rank(&b.kind)))
        });

        Ok(CashFlowSchedule {
            flows,
            // Start outstanding at zero; principal flows (including initial draw) build the path
            notional: Notional::par(0.0, self.facility.commitment_amount.currency()),
            day_count: self.facility.day_count,
            meta: crate::cashflow::builder::CashFlowMeta {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                calendar_ids: Vec::new(),
                facility_limit: Some(self.facility.commitment_amount),
                // commitment_date is the facility's effective start: the date on which the
                // initial draw is made and interest accrual begins. Providing it here
                // eliminates the inverse day count approximation (±1-2 day error) used
                // by the accrual engine when issue_date is absent.
                issue_date: Some(self.facility.commitment_date),
            },
        })
    }

    /// Build cashflow schedule from 3-factor path trajectory.
    ///
    /// This generates cashflows period by period based on the utilization, rate,
    /// and spread paths from Monte Carlo simulation.
    fn build_path_schedule(&self, path: &ThreeFactorPathData) -> Result<CashFlowSchedule> {
        let mut flows = Vec::new();
        let rc = RoundingContext::default();
        let ccy = self.facility.commitment_amount.currency();

        // Add initial draw at commitment_date (from lender perspective: negative cashflow)
        if self.facility.commitment_date > self.as_of
            && !rc.is_effectively_zero(self.facility.drawn_amount.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow {
                date: self.facility.commitment_date,
                reset_date: None,
                amount: self.facility.drawn_amount * -1.0,
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            });
        }

        // Track previous utilization for principal flows
        let mut prev_utilization = if path.payment_dates[0] <= self.as_of {
            path.utilization_path[0].clamp(0.0, 1.0)
        } else {
            self.facility.utilization_rate()
        };

        // Process each payment period using path data
        for i in 0..(path.payment_dates.len() - 1) {
            let period_start = path.payment_dates[i];
            let period_end = path.payment_dates[i + 1];
            let payment_date = match &self.facility.base_rate_spec {
                BaseRateSpec::Floating(spec) => {
                    super::utils::floating_payment_date(spec, period_end)?
                }
                BaseRateSpec::Fixed { .. } => period_end,
            };

            // Get path values at this step (step function - use period start)
            let utilization_start = path.utilization_path[i].clamp(0.0, 1.0);
            let utilization_end = path.utilization_path[i + 1].clamp(0.0, 1.0);
            let short_rate = path.short_rate_path[i];

            // Use average utilization for interest calculation (time-weighted approximation).
            // This better captures the balance evolution within each period when utilization
            // changes between period start and end, avoiding systematic underestimation of
            // interest when utilization is rising.
            let avg_utilization = (utilization_start + utilization_end) / 2.0;
            let drawn_balance = self.facility.commitment_amount * avg_utilization;
            let undrawn_balance = self.facility.commitment_amount * (1.0 - avg_utilization);

            // Calculate period interest using path's short rate
            let interest_rate = match &self.facility.base_rate_spec {
                BaseRateSpec::Fixed { rate } => *rate,
                BaseRateSpec::Floating(spec) => {
                    let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                    crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                        short_rate, &params,
                    )
                }
            };

            let dt = self.day_count.year_fraction(
                period_start,
                period_end,
                DayCountContext::default(),
            )?;
            let interest = drawn_balance * (interest_rate * dt);

            // Add interest cashflows if non-zero
            if payment_date > self.as_of && !rc.is_effectively_zero_money(interest.amount(), ccy) {
                flows.push(CashFlow {
                    date: payment_date,
                    reset_date: None,
                    amount: interest,
                    kind: match &self.facility.base_rate_spec {
                        BaseRateSpec::Fixed { .. } => CFKind::Fixed,
                        BaseRateSpec::Floating(_) => CFKind::FloatReset,
                    },
                    accrual_factor: dt,
                    rate: Some(interest_rate),
                });
            }

            // Calculate and emit fee cashflows using centralized functions.
            // Use average utilization for fee tier determination to match the interest
            // calculation above and avoid tier-boundary artifacts.
            let avg_util = (utilization_start + utilization_end) / 2.0;
            if payment_date > self.as_of {
                emit_revolving_credit_fees(
                    &mut flows,
                    &RevolvingFeeEmissionConfig {
                        payment_date,
                        drawn_balance: drawn_balance.amount(),
                        undrawn_balance: undrawn_balance.amount(),
                        commitment_amount: self.facility.commitment_amount.amount(),
                        commitment_fee_bp: self.facility.fees.commitment_fee_bps(avg_util),
                        usage_fee_bp: self.facility.fees.usage_fee_bps(avg_util),
                        facility_fee_bp: self.facility.fees.facility_fee_bp,
                        year_fraction: dt,
                        currency: ccy,
                    },
                )?;
            }

            // Handle principal flows from utilization changes
            // At period_end, utilization changes from start to end value for use in the next period
            let utilization_change = utilization_end - prev_utilization;
            if period_end > self.as_of
                && utilization_change.abs() > super::UTILIZATION_CHANGE_THRESHOLD
            {
                let principal_change = self.facility.commitment_amount * utilization_change;
                // Draw (increase) is negative for lender, repay (decrease) is positive
                flows.push(CashFlow {
                    date: period_end,
                    reset_date: None,
                    amount: principal_change * -1.0,
                    kind: CFKind::Notional,
                    accrual_factor: 0.0,
                    rate: None,
                });
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

        if self.facility.maturity > self.as_of
            && !rc.is_effectively_zero(final_balance.amount(), ZeroKind::Money(ccy))
        {
            flows.push(CashFlow {
                date: self.facility.maturity,
                reset_date: None,
                amount: final_balance,
                kind: CFKind::Notional,
                accrual_factor: 0.0,
                rate: None,
            });
        }

        // Sort flows — canonical order matching deterministic engine
        flows.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| cashflow_kind_rank(&a.kind).cmp(&cashflow_kind_rank(&b.kind)))
        });

        Ok(CashFlowSchedule {
            flows,
            // Start outstanding at zero; utilization-driven principal flows build the path
            notional: Notional::par(0.0, self.facility.commitment_amount.currency()),
            day_count: self.facility.day_count,
            meta: crate::cashflow::builder::CashFlowMeta {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                calendar_ids: Vec::new(),
                facility_limit: Some(self.facility.commitment_amount),
                // commitment_date is the facility's effective start: the date on which the
                // initial draw is made and interest accrual begins. Providing it here
                // eliminates the inverse day count approximation (±1-2 day error) used
                // by the accrual engine when issue_date is absent.
                issue_date: Some(self.facility.commitment_date),
            },
        })
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
    let draw_repay_events = match &facility.draw_repay_spec {
        DrawRepaySpec::Deterministic(events) => events,
        DrawRepaySpec::Stochastic(_) => {
            return Err(finstack_quant_core::Error::Validation(
                "calculate_drawn_balance_at_date requires DrawRepaySpec::Deterministic".to_string(),
            ));
        }
    };

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
