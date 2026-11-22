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

mod deterministic;
mod fees;
mod stochastic;
#[cfg(test)]
mod tests;

use finstack_quant_core::Result;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::money::Money;

use finstack_quant_cashflows::builder::{CashFlowSchedule, SchedulePeriod};
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};

use super::types::{DrawRepaySpec, RevolvingCredit};

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
    /// Rate trajectory for floating coupons. Deterministic-forward paths
    /// store raw curve-basis index rates; their baseline is annualized on each
    /// contractual coupon before spread, gearing and bounds are applied.
    /// Stochastic-rate paths store the ACT/365F model short rate, while explicit
    /// zero-volatility constant-rate paths store contractual rates.
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
    /// Principal changes already settled outside the schedule. They move the
    /// drawn balance like draws and repayments but emit no cash.
    postings: &'a [crate::loan_terms::PrincipalPosting],
}

impl<'a> CashflowEngine<'a> {
    fn schedule_meta(&self) -> finstack_quant_cashflows::builder::CashFlowMeta {
        finstack_quant_cashflows::builder::CashFlowMeta {
            projected_fixings: Vec::new(),
            representation: finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
            calendar_ids: Vec::new(),
            commitment: Some(self.facility.commitment),
            issue_date: Some(self.facility.issue_date),
            maturity: None,
        }
    }

    fn term_reset_window(
        &self,
        period: &SchedulePeriod,
        reset_effective: Date,
    ) -> Result<(Date, Date, f64)> {
        let period_start = period.accrual_start;
        let period_end = period.accrual_end;
        let reset_end = self
            .reset_dates
            .as_ref()
            .and_then(|grid| {
                grid.iter()
                    .copied()
                    .find(|&d| d > reset_effective && d < period_end)
            })
            .unwrap_or(period_end);
        if reset_effective == period_start && reset_end == period_end {
            return Ok((period_start, period_end, period.accrual_year_fraction));
        }
        let accrual_ctx = if self.day_count == DayCount::Act365L {
            finstack_quant_core::dates::DayCountContext {
                frequency: Some(self.facility.frequency),
                coupon_period: Some((period_start, period_end)),
                ..Default::default()
            }
        } else {
            finstack_quant_core::dates::DayCountContext::default()
        };
        let reset_accrual =
            self.day_count
                .year_fraction(reset_effective, reset_end, accrual_ctx)?;
        Ok((reset_effective, reset_end, reset_accrual))
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
            postings: &[],
        })
    }

    /// Deterministic schedule of `facility` on `market`, with the index's
    /// historical fixings applied to resets before `as_of` and `postings`
    /// applied to the drawn balance.
    pub(crate) fn deterministic(
        facility: &'a RevolvingCredit,
        market: &'a MarketContext,
        as_of: Date,
        postings: &'a [crate::loan_terms::PrincipalPosting],
    ) -> Result<PathAwareCashflowSchedule> {
        let fixings = super::utils::resolve_fixings(facility, market)?;
        let mut engine = Self::new(facility, Some(market), as_of, fixings)?;
        engine.postings = postings;
        engine.generate_deterministic()
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

    /// The drawn balance at the anchor must fit the commitment in force there.
    fn check_anchor_capacity(&self, anchor: Date) -> Result<()> {
        let commitment = self.facility.commitment_at(anchor);
        let lc = self.facility.lc_outstanding_at(anchor);
        if self.facility.drawn.amount() + lc.amount() > commitment.amount() + 1e-9 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit drawn {} plus letters of credit {} exceeds the \
                 commitment {} in force on the simulation anchor {anchor}",
                self.facility.drawn, lc, commitment
            )));
        }
        if let Some(event) = self
            .facility
            .lc
            .iter()
            .flat_map(|lc| lc.events.iter())
            .find(|event| event.date <= anchor)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit LC event dated {} is on or before the simulation anchor \
                 {anchor}; the LC outstanding at the anchor is defined by lc.outstanding, so \
                 date events strictly after it",
                event.date
            )));
        }
        Ok(())
    }

    /// Last reset-effective date at or before `date` (falls back to `date`
    /// itself when the facility has no reset grid).
    fn reset_effective_at(&self, date: Date) -> Date {
        self.reset_dates
            .as_ref()
            .and_then(|dates| dates.iter().rev().find(|&&reset| reset <= date).copied())
            .unwrap_or(date)
    }

    /// Append the commitment-reduction fees on step-downs and the scheduled
    /// fixed fees (amendment, waiver, extension) dated after `as_of`.
    fn push_scheduled_fees(&self, flows: &mut Vec<CashFlow>) -> Result<()> {
        for (date, fee) in self.facility.commitment_step_fees(self.as_of)? {
            flows.push(CashFlowParams::new(date, None, fee, CFKind::Fee, 0.0, None).build()?);
        }
        for (date, fee) in self.facility.scheduled_fees_after(self.as_of) {
            flows.push(CashFlowParams::new(date, None, fee, CFKind::Fee, 0.0, None).build()?);
        }
        Ok(())
    }
}

/// Drawn balance of a deterministic facility on `target_date`.
///
/// Starts from `drawn`, the balance at the simulation anchor, and
/// replays the draw/repay events dated after `as_of` up to and including
/// `target_date`. Dates on or before the anchor return `drawn`.
///
/// # Arguments
///
/// * `facility` - Facility with a `DrawRepaySpec::Deterministic` schedule;
///   a stochastic facility is a validation error.
/// * `as_of` - Valuation date; with the commitment date it defines the
///   anchor `drawn` refers to. Events dated on or before `as_of` are
///   not replayed (the cashflow engine rejects them).
/// * `target_date` - Date the balance is wanted for; events dated on it are
///   applied.
///
/// # Errors
///
/// Returns a validation error for a stochastic facility, or when a replayed
/// event would breach the commitment or repay more than the balance.
pub fn calculate_drawn_balance_at_date(
    facility: &RevolvingCredit,
    as_of: Date,
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

    let anchor = facility.issue_date.max(as_of);
    facility.drawn_balance_at(&draw_repay_events, anchor, target_date)
}
