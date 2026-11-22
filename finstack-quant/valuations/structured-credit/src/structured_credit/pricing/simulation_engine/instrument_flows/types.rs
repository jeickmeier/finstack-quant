//! Bucketed schedule, coupon/fee component, revolver-term, and per-name state types.

use super::super::exercise::ExerciseTerms;
use finstack_quant_cashflows::builder::FloatingRateParams;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_models::monte_carlo::process::cir::CirProcess;

/// Utilization volatility below which the process is frozen (parity mode),
/// mirroring the standalone facility's Monte Carlo path generator.
pub(in super::super) const FROZEN_UTILIZATION_VOL: f64 = 1e-8;

/// Instrument family, used where the engines treat the kinds differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum CollateralKind {
    /// Universal bond.
    Bond,
    /// Term loan.
    TermLoan,
    /// Revolving credit facility.
    Revolver,
}

/// One coupon flow kept as components so a simulated rate path can re-project
/// it through the instrument's own floors, caps and gearing, and a simulated
/// balance can rescale it.
#[derive(Debug, Clone)]
pub(in super::super) struct InterestComponent {
    /// Coupon amount projected on the valuation-date forward curve.
    pub(in super::super) projected: f64,
    /// Balance the coupon accrues on (`projected / (rate × accrual)`), zero
    /// when the schedule recorded no rate.
    pub(in super::super) basis: f64,
    /// Accrual fraction of the coupon.
    pub(in super::super) accrual: f64,
    /// Projected index rate behind a floating coupon, when recorded.
    pub(in super::super) index_rate: Option<f64>,
    /// Fixing date of a floating coupon, when known.
    pub(in super::super) fixing_date: Option<Date>,
    /// All-in annual rate recorded on the flow, when known.
    pub(in super::super) rate: Option<f64>,
}

impl InterestComponent {
    /// Re-projected all-in rate at `index_rate + shift` when the coupon is
    /// floating, its index rate is known, and its fixing is still ahead of
    /// `as_of`.
    fn reprojected_rate(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> Option<f64> {
        if self.basis <= 0.0
            || self.accrual <= 0.0
            || self.fixing_date.is_some_and(|fixing| fixing <= as_of)
        {
            return None;
        }
        let (index_rate, params) = (self.index_rate?, params?);
        Some(finstack_quant_cashflows::builder::calculate_floating_rate(
            index_rate + shift,
            params,
        ))
    }

    /// Coupon amount under an additive shift of the index rate.
    ///
    /// A zero shift, a fixing already observed on or before `as_of`, or a
    /// component without a recorded index rate returns the projected amount
    /// unchanged; otherwise the coupon is re-projected through the
    /// instrument's floors, caps and gearing at `index_rate + shift`.
    pub(in super::super) fn amount_with_shift(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        if shift == 0.0 {
            return self.projected;
        }
        match self.reprojected_rate(shift, as_of, params) {
            Some(all_in) => self.basis * self.accrual * all_in,
            None => self.projected,
        }
    }

    /// Coupon amount accrued on a simulated `balance` instead of the
    /// projected one: re-projected at `index_rate + shift` when the coupon
    /// is floating and unfixed, otherwise rescaled by `balance / basis`.
    pub(in super::super) fn amount_on_balance(
        &self,
        balance: f64,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        if let Some(all_in) = self.reprojected_rate(shift, as_of, params) {
            return balance * self.accrual * all_in;
        }
        if self.basis > 0.0 {
            self.projected * balance / self.basis
        } else {
            self.projected
        }
    }
}

/// Fee family, which decides the balance a simulated fee accrues on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum FeeKind {
    /// Commitment fee on the undrawn commitment.
    Commitment,
    /// Usage fee on the drawn balance.
    Usage,
    /// Facility fee on the total commitment, and upfront fees.
    Fixed,
}

/// One fee flow kept as components so a simulated drawn balance can rescale
/// it. Tiered fee rates enter through the schedule's period-average rate.
#[derive(Debug, Clone)]
pub(in super::super) struct FeeComponent {
    /// Fee amount projected on the contractual (or expected) schedule.
    pub(in super::super) projected: f64,
    /// Balance the fee accrued on (`projected / (rate × accrual)`), zero when
    /// the schedule recorded no rate.
    pub(in super::super) basis: f64,
    /// Accrual fraction of the fee.
    pub(in super::super) accrual: f64,
    /// Annual fee rate recorded on the flow, when known.
    pub(in super::super) rate: Option<f64>,
    /// Fee family.
    pub(in super::super) kind: FeeKind,
}

impl FeeComponent {
    /// Fee amount on a simulated position: `drawn` outstanding against
    /// `commitment` available, with `scale` the survival scale applied to
    /// fees that do not depend on utilization.
    pub(in super::super) fn amount_on_balance(
        &self,
        drawn: f64,
        commitment: f64,
        scale: f64,
    ) -> f64 {
        let base = match self.kind {
            FeeKind::Commitment => (commitment - drawn).max(0.0),
            FeeKind::Usage => drawn.max(0.0),
            FeeKind::Fixed => return self.projected * scale,
        };
        match self.rate {
            Some(rate) if self.accrual > 0.0 => base * rate * self.accrual,
            _ if self.basis > 0.0 => self.projected * base / self.basis,
            _ => self.projected * scale,
        }
    }
}

/// Contractual flows of one instrument inside one legal payment period.
#[derive(Debug, Clone, Default)]
pub(in super::super) struct PeriodBucket {
    /// Coupons (fixed, step-up, inflation, stub and floating), as components.
    pub(in super::super) interest: Vec<InterestComponent>,
    /// Upfront, commitment, usage and facility fees, as components.
    pub(in super::super) fees: Vec<FeeComponent>,
    /// Scheduled principal: amortization, revolver repayments, maturity balloon.
    pub(in super::super) repayment: f64,
    /// Principal advanced to the borrower: revolver and delayed draws.
    pub(in super::super) draw: f64,
    /// Interest capitalized into the balance (no cash).
    pub(in super::super) pik: f64,
}

impl PeriodBucket {
    /// Total contractual coupon of the period on the valuation-date curve.
    pub(in super::super) fn interest(&self) -> f64 {
        self.interest.iter().map(|c| c.projected).sum()
    }

    /// Total contractual fees of the period.
    pub(in super::super) fn fees(&self) -> f64 {
        self.fees.iter().map(|f| f.projected).sum()
    }

    /// Total coupon of the period with floating components re-projected under
    /// an additive index shift (see [`InterestComponent::amount_with_shift`]).
    pub(in super::super) fn interest_with_shift(
        &self,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        self.interest
            .iter()
            .map(|c| c.amount_with_shift(shift, as_of, params))
            .sum()
    }

    /// Total coupon of the period accrued on a simulated balance.
    pub(in super::super) fn interest_on_balance(
        &self,
        balance: f64,
        shift: f64,
        as_of: Date,
        params: Option<&FloatingRateParams>,
    ) -> f64 {
        self.interest
            .iter()
            .map(|c| c.amount_on_balance(balance, shift, as_of, params))
            .sum()
    }

    /// Total fees of the period on a simulated position.
    pub(in super::super) fn fees_on_balance(&self, drawn: f64, commitment: f64, scale: f64) -> f64 {
        self.fees
            .iter()
            .map(|f| f.amount_on_balance(drawn, commitment, scale))
            .sum()
    }
}

/// Where an instrument's period default probability comes from.
#[derive(Debug, Clone)]
pub(in super::super) enum PeriodDefaultSource {
    /// Conditional default probability from the instrument's hazard curve.
    HazardCurve(String),
    /// The deal's `DefaultModelSpec`.
    DealModel,
}

/// Credit-spread state of a simulated revolver.
#[derive(Debug, Clone)]
pub(in super::super) enum SpreadTerms {
    /// Constant spread (no dynamics).
    Constant(f64),
    /// CIR spread process started at `initial`.
    Cir {
        /// Process stepped with the QE scheme.
        process: CirProcess,
        /// Initial spread the relative change is measured from.
        initial: f64,
    },
}

impl SpreadTerms {
    /// Spread at the simulation anchor.
    pub(in super::super) fn initial(&self) -> f64 {
        match self {
            Self::Constant(spread) => *spread,
            Self::Cir { initial, .. } => *initial,
        }
    }
}

/// Utilization process of a revolver with simulated draws.
#[derive(Debug, Clone)]
pub(in super::super) struct UtilizationTerms {
    /// Mean-reversion speed per year.
    pub(in super::super) kappa: f64,
    /// Unlinked target utilization.
    pub(in super::super) theta: f64,
    /// Annualized utilization volatility; below [`FROZEN_UTILIZATION_VOL`]
    /// the utilization is frozen.
    pub(in super::super) sigma: f64,
    /// Target shift per unit of relative spread change.
    pub(in super::super) spread_sensitivity: f64,
    /// Correlation between the utilization shock and the name's spread shock.
    pub(in super::super) util_credit_corr: f64,
    /// Spread process.
    pub(in super::super) spread: SpreadTerms,
}

/// Contractual rate basis of a revolver. The draw option cost is anchored to
/// the margin at the valuation date, so only the fixed all-in rate is read
/// (to accrue a simulated balance when no coupon was projected).
#[derive(Debug, Clone, Copy)]
pub(in super::super) enum MarginTerms {
    /// Spread over the floating index.
    Spread,
    /// Fixed all-in rate (decimal), margin steps included.
    FixedRate(f64),
}

/// Revolver terms the engines need beyond the bucketed schedule.
///
/// Dated terms are resolved once per legal period at the period start, so a
/// commitment, margin or LC step dated inside a deal period takes effect
/// from the next period; the facility's own engines slice on the step date.
#[derive(Debug, Clone)]
pub(in super::super) struct RevolverTerms {
    /// Commitment in force during each legal period (the facility's
    /// `commitment_steps` read at the period start), aligned with the
    /// prepared periods.
    pub(in super::super) commitments: Vec<f64>,
    /// Loan-equivalent exposure: fraction of the undrawn commitment drawn at
    /// default.
    pub(in super::super) leq: f64,
    /// Letter-of-credit face assumed drawn at default in each legal period
    /// (`lc.leq × LC outstanding` at the period start), aligned with the
    /// periods.
    pub(in super::super) lc_exposure: Vec<f64>,
    /// Letter-of-credit face outstanding in each legal period, aligned with
    /// the periods; it consumes availability.
    pub(in super::super) lc_outstanding: Vec<f64>,
    /// Interest accrual day count.
    pub(in super::super) day_count: DayCount,
    /// Contractual margin in force during each legal period (the facility's
    /// `margin_steps` read at the period start), aligned with the periods.
    pub(in super::super) margins: Vec<MarginTerms>,
    /// Utilization process for stochastic facilities; `None` keeps the
    /// contractual (or expected) draw schedule.
    pub(in super::super) utilization: Option<UtilizationTerms>,
}

impl RevolverTerms {
    /// Commitment in force during period `k` (the last known value past the
    /// end of the schedule).
    pub(in super::super) fn commitment_at(&self, k: usize) -> f64 {
        self.commitments
            .get(k)
            .or(self.commitments.last())
            .copied()
            .unwrap_or(0.0)
    }

    /// Letter-of-credit face drawn at default in period `k`.
    pub(in super::super) fn lc_exposure_at(&self, k: usize) -> f64 {
        self.lc_exposure
            .get(k)
            .or(self.lc_exposure.last())
            .copied()
            .unwrap_or(0.0)
    }

    /// Letter-of-credit face outstanding in period `k`.
    pub(in super::super) fn lc_outstanding_at(&self, k: usize) -> f64 {
        self.lc_outstanding
            .get(k)
            .or(self.lc_outstanding.last())
            .copied()
            .unwrap_or(0.0)
    }

    /// Contractual margin in force during period `k`.
    pub(in super::super) fn margin_at(&self, k: usize) -> MarginTerms {
        self.margins
            .get(k)
            .or(self.margins.last())
            .copied()
            .unwrap_or(MarginTerms::Spread)
    }
}

/// One instrument's bucketed schedule and resolved terms.
#[derive(Debug, Clone)]
pub(in super::super) struct InstrumentSchedule {
    /// Instrument identifier.
    pub(in super::super) id: String,
    /// Instrument family.
    pub(in super::super) kind: CollateralKind,
    /// Outstanding at the valuation date under the contractual schedule.
    pub(in super::super) opening_balance: f64,
    /// Contractual maturity.
    pub(in super::super) maturity: Date,
    /// Contractual flows per legal period, aligned with the prepared periods.
    pub(in super::super) periods: Vec<PeriodBucket>,
    /// Exercise windows and policies.
    pub(in super::super) exercise: ExerciseTerms,
    /// Default-probability source.
    pub(in super::super) default_source: PeriodDefaultSource,
    /// Explicit recovery fraction; `None` uses the hazard curve or deal recovery.
    pub(in super::super) recovery_rate: Option<f64>,
    /// Floating-rate terms for re-projection; `None` for fixed instruments.
    pub(in super::super) floating_params: Option<FloatingRateParams>,
    /// Revolver terms; `None` for bonds and term loans.
    pub(in super::super) revolver: Option<RevolverTerms>,
}

impl InstrumentSchedule {
    /// All-in rate to accrue a simulated revolver balance at when period `k`
    /// carries no projected coupon (the expected schedule had no balance):
    /// the rate recorded on the nearest bucket with a coupon, else the fixed
    /// rate, else zero.
    pub(super) fn fallback_rate(&self, k: usize) -> f64 {
        let recorded = |bucket: &PeriodBucket| bucket.interest.iter().find_map(|c| c.rate);
        let n = self.periods.len();
        for offset in 0..n {
            if let Some(rate) = self.periods.get(k + offset).and_then(recorded) {
                return rate;
            }
            if offset > 0
                && offset <= k
                && let Some(rate) = self.periods.get(k - offset).and_then(recorded)
            {
                return rate;
            }
        }
        match self.revolver.as_ref().map(|r| r.margin_at(k)) {
            Some(MarginTerms::FixedRate(rate)) => rate,
            _ => 0.0,
        }
    }
}

/// Running state of one instrument inside one simulation run.
#[derive(Debug, Clone)]
pub(in super::super) struct NameState {
    /// Survival scale applied to contractual amounts: the product of default
    /// and prepayment survivors realized so far.
    pub(in super::super) scale: f64,
    /// `true` once redeemed, matured or fully defaulted.
    pub(in super::super) retired: bool,
    /// Simulated utilization (fraction of commitment) for stochastic revolvers.
    pub(in super::super) utilization: f64,
    /// Simulated credit spread for stochastic revolvers.
    pub(in super::super) spread: f64,
    /// Balance draw funded for the instrument in the latest period.
    pub(in super::super) period_draw: f64,
}
