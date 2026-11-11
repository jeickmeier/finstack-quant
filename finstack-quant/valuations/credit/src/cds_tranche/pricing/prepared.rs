//! Prepared tranche pricing with an immutable market and contract snapshot.
use super::expected_loss::PreparedExpectedLoss;
use super::pricer::{CdsTranchePricer, DiscountAt, ElWdPoint, ProjectedDiscountedRow};
use crate::cds_tranche::CdsTranche;
use finstack_quant_cashflows::builder::SchedulePeriod;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_core::{FxHashMap, Result};
use std::sync::Arc;

/// Immutable contract, schedule and market inputs for repeated correlation trials.
///
/// Preparation retains default probabilities/thresholds, default timing and
/// discount factors. Evaluations use the ordinary pricing kernel and retain
/// one raw attachment loss/recovery vector keyed by its actual smoothed
/// correlation. Interpolated attachment changes invalidate that last-value cache;
/// no trial curves or unbounded history are retained. Storage is bounded by
/// coupon count times constituent count. Reprepare after changing any market
/// or contract input other than base correlation. This is a Rust pricing
/// workspace, not a serializable market object.
pub struct PreparedTranche {
    pricer: CdsTranchePricer,
    state: PreparedState,
}
enum PreparedState {
    Live {
        projection: Box<PreparedProjection>,
        discounts: FxHashMap<Date, Result<f64>>,
    },
    Wiped(Money),
}
impl std::fmt::Debug for PreparedTranche {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedTranche").finish_non_exhaustive()
    }
}

impl PreparedTranche {
    /// Price the snapshotted trade at a trial base-correlation curve.
    ///
    /// # Arguments
    /// * `correlation` - Trial curve with detachments in percentage points and
    ///   decimal asset correlations. Its ID need not match the original curve;
    ///   interpolation and the established boundary smoothing apply at the
    ///   trade's original attachment/detachment. A wiped-out trade ignores it.
    ///
    /// # Returns
    /// Signed present value in the trade's original notional currency, including
    /// outstanding upfront cash and the original settlement/day-count rules.
    ///
    /// # Errors
    /// Returns the ordinary pricing errors for non-finite expectations,
    /// quadrature exhaustion, materially negative maturity tranchelet losses,
    /// or invalid discount factors needed by nonzero projected cashflows.
    pub fn price(&self, correlation: &BaseCorrelationCurve) -> Result<Money> {
        match &self.state {
            PreparedState::Wiped(value) => Ok(*value),
            PreparedState::Live {
                projection,
                discounts,
            } => {
                let rows = self.pricer.project_prepared_rows(
                    projection,
                    correlation,
                    projection.tranche.coupon_bp / BASIS_POINTS_PER_UNIT,
                    false,
                )?;
                let mut pv = 0.0;
                for row in rows {
                    let date = row.discount_date(projection.as_of);
                    let df = discounts
                        .get(&date)
                        .ok_or_else(|| {
                            finstack_quant_core::Error::Validation(
                                "prepared tranche is missing a projected discount date".into(),
                            )
                        })?
                        .clone()?;
                    pv += row.cashflow.amount.amount() * df;
                }
                Money::from_model_value(pv, projection.tranche.notional.currency())
            }
        }
    }
    pub(super) fn price_at_snapshot(&self) -> Result<Money> {
        match &self.state {
            PreparedState::Wiped(value) => Ok(*value),
            PreparedState::Live { projection, .. } => {
                self.price(projection.index.base_correlation_curve())
            }
        }
    }
}

pub(super) struct PreparedProjection {
    pub(super) tranche: CdsTranche,
    pub(super) index: Arc<CreditIndexData>,
    pub(super) as_of: Date,
    pub(super) valuation_date: Date,
    pub(super) periods: Vec<SchedulePeriod>,
    pub(super) timing: Vec<PeriodTiming>,
    pub(super) losses: PreparedExpectedLoss,
    pub(super) known_count: usize,
}
#[derive(Clone, Copy)]
pub(super) struct PeriodTiming {
    pub(super) protection_start: Date,
    pub(super) remaining_accrual: f64,
    pub(super) default_fraction: f64,
}
impl PreparedProjection {
    pub(super) fn el_curve(
        &self,
        pricer: &CdsTranchePricer,
        correlation: &BaseCorrelationCurve,
    ) -> Result<Vec<ElWdPoint>> {
        let mut curve = Vec::with_capacity(self.periods.len());
        for _ in 0..self.known_count {
            curve.push(ElWdPoint {
                el_fraction: pricer.calculate_prior_tranche_loss(&self.tranche),
                wd_fraction: pricer
                    .calculate_prior_tranche_writedown(&self.tranche, self.index.recovery_rate()),
            });
        }
        curve.extend(self.losses.evaluate(pricer, correlation)?);
        Ok(curve)
    }
}

impl ProjectedDiscountedRow {
    pub(super) fn discount_date(&self, as_of: Date) -> Date {
        match self.discount_at {
            DiscountAt::PaymentDate => self.cashflow.date,
            DiscountAt::WithinPeriod {
                start,
                end,
                fraction,
            } => default_date(as_of, start, end, fraction),
        }
    }
}
fn default_date(as_of: Date, start: Date, end: Date, fraction: f64) -> Date {
    let period_days = (end - start).whole_days().max(0);
    let offset_days = (fraction * period_days as f64).round() as i64;
    start
        .checked_add(time::Duration::days(offset_days))
        .unwrap_or(end)
        .clamp(as_of, end)
}

impl CdsTranchePricer {
    /// Prepare immutable pricing inputs for repeated correlation evaluations.
    ///
    /// # Arguments
    /// * `tranche` - Contract to snapshot, including percent-point attachment
    ///   and detachment, basis-point running coupon, currency notional,
    ///   realized losses, upfront cash and schedule conventions.
    /// * `market` - Snapshot supplying the discount curve and complete credit
    ///   index/issuer curves by the contract's IDs. Credit curves must be based
    ///   on `as_of` for an unexpired trade; discount curves may have another base.
    /// * `as_of` - Valuation date used for settlement, survival and relative
    ///   discounting. Cashflows paid before this date are excluded.
    ///
    /// # Returns
    /// A reusable workspace retaining this pricer's copula specification.
    /// Subsequent market mutations cannot change its results.
    ///
    /// # Errors
    /// Returns ordinary contract, missing-curve, curve-base, schedule and pool
    /// validation errors. A wiped-out trade requires only its discount curve.
    /// Integration and trial-curve arbitrage errors occur when pricing.
    pub fn prepare(
        &self,
        tranche: &CdsTranche,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<PreparedTranche> {
        tranche.validate()?;
        let mut prepared = self.prepare_validated_tranche(tranche, market, as_of)?;
        // Only explicitly reusable workspaces retain trial results. Ordinary
        // one-shot pricing still uses this same kernel without cache allocation.
        if let PreparedState::Live { projection, .. } = &mut prepared.state {
            projection.losses.enable_attachment_cache();
        }
        Ok(prepared)
    }
    pub(super) fn prepare_validated_tranche(
        &self,
        tranche: &CdsTranche,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<PreparedTranche> {
        let discount = market.get_discount(tranche.discount_curve_id.as_ref())?;
        let state = if tranche.realized_loss >= tranche.detach_pct / 100.0 {
            let pv = if let Some((date, amount)) =
                tranche.upfront.filter(|(date, _)| *date >= as_of)
            {
                amount.amount() * tranche.side.sign() * discount.df_between_dates(as_of, date)?
            } else {
                0.0
            };
            PreparedState::Wiped(Money::from_model_value(pv, tranche.notional.currency())?)
        } else {
            let projection = self.prepare_projection_inputs(tranche, market, as_of)?;
            let mut discounts = FxHashMap::default();
            let mut retain = |date| {
                discounts
                    .entry(date)
                    .or_insert_with(|| discount.df_between_dates(as_of, date));
            };
            for (period, timing) in projection.periods.iter().zip(&projection.timing) {
                retain(period.payment_date);
                if period.accrual_end > as_of {
                    retain(default_date(
                        as_of,
                        timing.protection_start,
                        period.accrual_end,
                        timing.default_fraction,
                    ));
                }
            }
            if let Some((date, _)) = tranche.upfront.filter(|(date, _)| *date >= as_of) {
                retain(date);
            }
            // Retain errors until their cashflow is actually used: zero flows
            // must not gain discount-curve errors merely through preparation.
            PreparedState::Live {
                projection: Box::new(projection),
                discounts,
            }
        };
        Ok(PreparedTranche {
            pricer: self.clone(),
            state,
        })
    }

    pub(super) fn project_prepared_rows(
        &self,
        inputs: &PreparedProjection,
        correlation: &BaseCorrelationCurve,
        coupon: f64,
        include_tiny_premiums: bool,
    ) -> Result<Vec<ProjectedDiscountedRow>> {
        let tranche = &inputs.tranche;
        let _index_data_arc = &inputs.index;
        let as_of = inputs.as_of;
        let periods = &inputs.periods;
        let el_curve = inputs.el_curve(self, correlation)?;
        let tranche_notional = tranche.notional.amount();
        let premium_sign = tranche.side.sign();
        let protection_sign = -premium_sign;

        let mut rows =
            Vec::with_capacity(periods.len() * 2 + usize::from(tranche.upfront.is_some()));
        let mut prev_el_fraction = self.calculate_prior_tranche_loss(tranche);
        let mut prev_wd_fraction =
            self.calculate_prior_tranche_writedown(tranche, _index_data_arc.recovery_rate());
        for (i, period) in periods.iter().enumerate() {
            let payment_date = period.payment_date;
            let el_fraction = el_curve[i].el_fraction;
            let wd_fraction = el_curve[i].wd_fraction;
            let delta_el_fraction = (el_fraction - prev_el_fraction).max(0.0);
            let delta_wd_fraction = (wd_fraction - prev_wd_fraction).max(0.0);
            // Premium accrues on the notional surviving BOTH bottom-up loss
            // erosion and top-down recovery writedown (senior amortization).
            let outstanding_notional =
                tranche_notional * (1.0 - prev_el_fraction - prev_wd_fraction).max(0.0);
            let accrual_period = period.accrual_year_fraction;
            let PeriodTiming {
                protection_start,
                remaining_accrual,
                default_fraction,
            } = inputs.timing[i];

            // Premium reduction on the slice that defaults within the period.
            //
            // A name defaulting at the survival-weighted fraction `f` of the
            // period pays accrued premium over `f·Δ` only, so the premium
            // notional lost on the defaulted slice is `(1−f)·Δerosion`
            // (ISDA accrual-on-default). Both the loss increment (bottom-up)
            // and the recovery writedown increment (top-down) occur at
            // default time, so they receive the same treatment.
            let delta_erosion = delta_el_fraction + delta_wd_fraction;
            let aod_adjustment =
                (1.0 - default_fraction) * remaining_accrual * tranche_notional * delta_erosion;
            let premium_amount =
                coupon * (accrual_period * outstanding_notional - aod_adjustment).max(0.0);

            if include_tiny_premiums || premium_amount.abs() > f64::EPSILON {
                rows.push(ProjectedDiscountedRow {
                    cashflow: CashFlow::new(
                        payment_date,
                        None,
                        Money::from_model_value(
                            premium_amount * premium_sign,
                            tranche.notional.currency(),
                        )?,
                        CFKind::Fixed,
                        accrual_period,
                        Some(coupon),
                    ),
                    discount_at: DiscountAt::PaymentDate,
                });
            }

            let default_amount = tranche_notional * delta_el_fraction;
            if default_amount.abs() > f64::EPSILON {
                rows.push(ProjectedDiscountedRow {
                    cashflow: CashFlow::new(
                        payment_date,
                        None,
                        Money::from_model_value(
                            default_amount * protection_sign,
                            tranche.notional.currency(),
                        )?,
                        CFKind::DefaultedNotional,
                        0.0,
                        None,
                    ),
                    // Discount the loss increment at the time the underlying
                    // defaults actually occur, not the period end (ISDA
                    // mid-period protection timing): the within-period loss
                    // is discounted at the SURVIVAL-WEIGHTED mean default
                    // time (Item 6) — `fraction` of the way through the
                    // period — rather than the flat period midpoint, which
                    // over-discounted the increment by assuming all loss
                    // lands at the midpoint. The default fraction is < 0.5
                    // for a positive hazard, so the loss is correctly
                    // discounted slightly earlier than mid-period. The
                    // fraction is measured on the hazard axis (a survival
                    // quantity); the DF lookup itself happens on the
                    // discount curve's axis in `discount_projected_rows`.
                    discount_at: DiscountAt::WithinPeriod {
                        start: protection_start,
                        end: period.accrual_end,
                        fraction: default_fraction,
                    },
                });
            }

            prev_el_fraction = el_fraction;
            prev_wd_fraction = wd_fraction;
        }

        if let Some((date, amount)) = tranche.upfront.filter(|(date, _)| *date >= as_of) {
            rows.push(ProjectedDiscountedRow {
                cashflow: CashFlow::new(
                    date,
                    None,
                    Money::from_model_value(amount.amount() * premium_sign, amount.currency())?,
                    CFKind::Fee,
                    0.0,
                    None,
                ),
                discount_at: DiscountAt::PaymentDate,
            });
        }

        Ok(rows)
    }
}
