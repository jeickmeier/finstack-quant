//! CDS Index pricing engine and helpers.
//!
//! Provides deterministic valuation for CDS indices with two pricing modes:
//! 1) Single-curve mode: price the index off a single index hazard curve by
//!    delegating to a synthetic single-name `CreditDefaultSwap` constructed
//!    from the index fields.
//! 2) Constituents mode: price each underlying issuer as a CDS with its own
//!    hazard curve and weight, then aggregate the results.
//!
//! Public API mirrors the CDS pricer surface for parity: NPV, par spread,
//! risky PV01, and leg PVs. Heavy numerical work is delegated to
//! `crate::instruments::credit_derivatives::cds::pricer::CDSPricer`.

use crate::calibration::bumps::hazard::{bump_hazard_shift, bump_hazard_spreads};
use crate::calibration::bumps::BumpRequest;
use crate::cashflow::builder::schedule::merge_cashflow_schedules;
use crate::cashflow::builder::{CashFlowSchedule, Notional};
use crate::cashflow::primitives::{CFKind, CashFlow};
use crate::cashflow::traits::{
    schedule_from_classified_flows, CashflowProvider, ScheduleBuildOpts,
};
use crate::constants::{credit, BASIS_POINTS_PER_UNIT};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::pricer::{
    date_from_hazard_time, CDSPricer, CDSPricerConfig,
};
use crate::instruments::credit_derivatives::cds::{CreditDefaultSwap, PayReceive};
use crate::instruments::credit_derivatives::cds_index::{
    CDSIndex, ConstituentResult, IndexParSpreadResult, IndexPricing, IndexResult, ParSpreadMethod,
};
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};

/// Tolerance applied to the constituent weight sum when validating that ∑w ≈ 1.
///
/// Loose enough to accommodate fp64 accumulated rounding for 125-name baskets
/// with hand-entered fractional weights (e.g. `1.0/125.0` repeated 125 times).
const WEIGHT_SUM_TOL: f64 = 1e-6;

/// Tolerance for validating index_factor consistency with defaulted weights.
const INDEX_FACTOR_CONSISTENCY_TOL: f64 = 1e-6;

/// CDS Index pricing engine. Aggregates single-name CDS pricing according to
/// the index's configured pricing mode.
///
/// All priced quantities (NPV, leg PVs, par spread, RPV01, CS01) are
/// derived by delegating to the single-name `CDSPricer` per-name (Constituents
/// mode) or via a single synthetic CDS (SingleCurve mode), then aggregating.
/// This guarantees that `npv ≈ par_spread × notional × risky_pv01 − pv_protection_leg`
/// holds to numerical tolerance.
///
/// `project_cds_flows` is retained as an informational projection used only
/// by `build_projected_schedule` to expose expected cashflow timing to
/// `CashflowProvider` consumers; it is intentionally a coarser approximation
/// than the priced values and is not used to compute any reported PV.
pub(crate) struct CDSIndexPricer {
    /// Configuration carried so inner `CDSPricer` instances and the
    /// informational flow projection stay in sync.
    cds_config: CDSPricerConfig,
}

#[derive(Debug, Clone)]
struct ResolvedConstituent {
    cds: CreditDefaultSwap,
    credit_curve_id: CurveId,
    recovery_rate: f64,
    weight_raw: f64,
    weight_effective: f64,
}

/// Projected per-constituent cashflows used only by `build_projected_schedule`
/// for the informational `CashflowProvider` view. Pricing does NOT consume these.
#[derive(Debug, Clone)]
struct ProjectedConstituentFlows {
    flows: Vec<CashFlow>,
}

#[derive(Debug, Clone)]
struct ProjectedIndexFlows {
    single_curve: Option<Vec<CashFlow>>,
    constituents: Vec<ProjectedConstituentFlows>,
}

impl Default for CDSIndexPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl CDSIndexPricer {
    /// Create a new CDS Index pricer with default ISDA-compliant CDS config.
    pub(crate) fn new() -> Self {
        Self {
            cds_config: CDSPricerConfig::default(),
        }
    }

    /// Create a CDS Index pricer with a custom CDS pricer configuration.
    ///
    /// Allows callers to plumb through ISDA vs Bloomberg-CDSW par spread
    /// methodology, regional `business_days_per_year`, etc.
    #[cfg(test)]
    pub(crate) fn with_config(cds_config: CDSPricerConfig) -> Self {
        Self { cds_config }
    }

    /// Compute instrument NPV from the perspective of `PayReceive`.
    pub(crate) fn npv(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.npv_detailed(index, curves, as_of)?.total)
    }

    /// Compute instrument NPV with optional per-constituent breakdown.
    ///
    /// NPV is computed by delegating to `CDSPricer::npv_full` per resolved
    /// position (one synthetic CDS in `SingleCurve` mode, N constituents
    /// otherwise) and then summing. The index-level upfront override
    /// (`pricing_overrides.market_quotes.upfront_payment`) is applied once at
    /// the aggregate, with the same sign convention used by `CDSPricer::npv_full`
    /// for single-name CDS upfronts.
    ///
    /// `CDSIndex` does not currently model a dated upfront (`Option<(Date, Money)>`)
    /// — only the already-discounted PV-adjustment override is honored.
    pub(crate) fn npv_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexResult<Money>> {
        let currency = index.notional.currency();
        let mut result = self.aggregate_money_detailed(
            index,
            curves,
            as_of,
            |pricer, cds, disc, surv, as_of| {
                let raw = pricer.npv_full(cds, disc, surv, as_of)?;
                Ok(Money::new(raw, cds.notional.currency()))
            },
        )?;
        if let Some(upfront) = index.pricing_overrides.market_quotes.upfront_payment {
            result.total = match index.side {
                PayReceive::Pay => result.total.checked_sub(upfront)?,
                PayReceive::Receive => result.total.checked_add(upfront)?,
            };
        }
        // Ensure consistent currency handling even when constituents list is empty.
        if result.total.currency() != currency {
            result.total = Money::new(result.total.amount(), currency);
        }
        Ok(result)
    }

    /// Present value of the protection leg (aggregated by pricing mode).
    ///
    /// Returns the unsigned protection-leg PV (always non-negative).
    pub(crate) fn pv_protection_leg(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.pv_protection_leg_detailed(index, curves, as_of)?.total)
    }

    /// Present value of the protection leg with optional per-constituent breakdown.
    ///
    /// Delegates to `CDSPricer::pv_protection_leg` per resolved position and
    /// sums. Result is the unsigned (non-negative) protection-leg PV.
    pub(crate) fn pv_protection_leg_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexResult<Money>> {
        self.aggregate_money_detailed(index, curves, as_of, |pricer, cds, disc, surv, as_of| {
            pricer.pv_protection_leg(cds, disc, surv, as_of)
        })
    }

    /// Present value of the premium leg (aggregated by pricing mode).
    ///
    /// Returns the unsigned premium-leg PV (always non-negative).
    pub(crate) fn pv_premium_leg(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        Ok(self.pv_premium_leg_detailed(index, curves, as_of)?.total)
    }

    /// Present value of the premium leg with optional per-constituent breakdown.
    ///
    /// Delegates to `CDSPricer::pv_premium_leg` per resolved position and
    /// sums. Result is the unsigned (non-negative) premium-leg PV.
    pub(crate) fn pv_premium_leg_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexResult<Money>> {
        self.aggregate_money_detailed(index, curves, as_of, |pricer, cds, disc, surv, as_of| {
            pricer.pv_premium_leg(cds, disc, surv, as_of)
        })
    }

    /// Par spread in basis points that sets NPV to zero.
    pub(crate) fn par_spread(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        Ok(self
            .par_spread_detailed(index, curves, as_of)?
            .total_spread_bp)
    }

    /// Par spread in basis points with optional per-constituent breakdown.
    ///
    /// Honours `self.cds_config.par_spread_uses_full_premium`: when set,
    /// the per-position denominator is the full premium-leg PV per unit
    /// spread (with accrual-on-default, Bloomberg CDSW convention);
    /// otherwise the ISDA-standard risky annuity is used. Both branches
    /// (`SingleCurve` and `Constituents`) honour the flag identically so
    /// a single config field controls index par-spread methodology.
    pub(crate) fn par_spread_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexParSpreadResult> {
        let pricer = CDSPricer::with_config(self.cds_config.clone());
        let method = if self.cds_config.par_spread_uses_full_premium {
            ParSpreadMethod::FullPremiumAoD
        } else {
            ParSpreadMethod::RiskyAnnuity
        };
        match index.pricing {
            IndexPricing::SingleCurve => {
                let cds = self.synthetic_cds(index);
                let disc = curves.get_discount(&cds.premium.discount_curve_id)?;
                let surv = curves.get_hazard(&cds.protection.credit_curve_id)?;
                let numerator_protection_pv =
                    pricer.pv_protection_leg(&cds, disc.as_ref(), surv.as_ref(), as_of)?;
                let denom_per_unit =
                    pricer.par_spread_denominator(&cds, disc.as_ref(), surv.as_ref(), as_of)?;
                let denominator = denom_per_unit * cds.notional.amount();
                if denominator.abs() < credit::PAR_SPREAD_DENOM_TOLERANCE {
                    return Err(Error::Validation(
                        "CDS Index par spread denominator near zero. This may indicate \
                         zero survival probability or expired protection."
                            .to_string(),
                    ));
                }
                let total_spread_bp =
                    numerator_protection_pv.amount() / denominator * BASIS_POINTS_PER_UNIT;
                Ok(IndexParSpreadResult {
                    total_spread_bp,
                    constituents_spread_bp: Vec::new(),
                    method,
                    numerator_protection_pv,
                    denominator,
                })
            }
            IndexPricing::Constituents => {
                let positions = self.constituent_positions(index)?;
                let mut numerator_protection_pv = Money::new(0.0, index.notional.currency());
                let mut denominator = 0.0;
                let mut constituents_spread_bp = Vec::with_capacity(positions.len());
                for position in positions {
                    let cds = &position.cds;
                    let disc = curves.get_discount(&cds.premium.discount_curve_id)?;
                    let surv = curves.get_hazard(&cds.protection.credit_curve_id)?;
                    let prot_pv =
                        pricer.pv_protection_leg(cds, disc.as_ref(), surv.as_ref(), as_of)?;
                    numerator_protection_pv = numerator_protection_pv.checked_add(prot_pv)?;
                    let denom_per_unit =
                        pricer.par_spread_denominator(cds, disc.as_ref(), surv.as_ref(), as_of)?;
                    let local_denom = denom_per_unit * cds.notional.amount();
                    denominator += local_denom;
                    // Guard per-constituent division: if the local denominator is near zero
                    // (e.g., for a near-defaulted name with negligible survival probability),
                    // report NaN rather than propagating Inf which corrupts aggregation.
                    let constituent_spread_bp =
                        if local_denom.abs() < credit::PAR_SPREAD_DENOM_TOLERANCE {
                            f64::NAN
                        } else {
                            prot_pv.amount() / local_denom * BASIS_POINTS_PER_UNIT
                        };
                    constituents_spread_bp.push(ConstituentResult {
                        credit_curve_id: position.credit_curve_id,
                        recovery_rate: position.recovery_rate,
                        weight_raw: position.weight_raw,
                        weight_effective: position.weight_effective,
                        value: constituent_spread_bp,
                    });
                }
                if denominator.abs() < credit::PAR_SPREAD_DENOM_TOLERANCE {
                    return Err(Error::Validation(
                        "CDS Index par spread denominator near zero (risky annuity sum ≈ 0). \
                         This may indicate zero survival probability across all constituents."
                            .to_string(),
                    ));
                }
                let total_spread_bp =
                    numerator_protection_pv.amount() / denominator * BASIS_POINTS_PER_UNIT;
                Ok(IndexParSpreadResult {
                    total_spread_bp,
                    constituents_spread_bp,
                    method,
                    numerator_protection_pv,
                    denominator,
                })
            }
        }
    }

    /// Risky PV01 (absolute currency units) aggregated by pricing mode.
    pub(crate) fn risky_pv01(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        Ok(self.risky_pv01_detailed(index, curves, as_of)?.total)
    }

    /// Build an informational projected premium/default schedule for the index.
    ///
    /// The schedule exposes expected cashflow timing for `CashflowProvider`
    /// consumers (treasury reports, schedule listings). It is a coarser
    /// approximation than the priced PV (which uses ISDA Standard Model
    /// integration via `CDSPricer`); discounting this schedule and summing
    /// will agree with `npv()` only to within a few percent for benign curves.
    pub(crate) fn build_projected_schedule(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<CashFlowSchedule> {
        let projected = self.project_resolved_flows(index, curves, as_of)?;
        match projected.single_curve {
            Some(flows) => Ok(schedule_from_classified_flows(
                flows,
                index.premium.day_count,
                ScheduleBuildOpts {
                    notional_hint: Some(index.notional),
                    ..Default::default()
                },
            )),
            None => {
                let schedules = projected
                    .constituents
                    .into_iter()
                    .map(|projection| {
                        schedule_from_classified_flows(
                            projection.flows,
                            index.premium.day_count,
                            ScheduleBuildOpts {
                                notional_hint: Some(index.notional),
                                ..Default::default()
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                merge_cashflow_schedules(
                    schedules,
                    Notional::par(index.notional.amount(), index.notional.currency()),
                    index.premium.day_count,
                )
            }
        }
    }

    /// Risky PV01 with optional per-constituent breakdown.
    pub(crate) fn risky_pv01_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexResult<f64>> {
        self.aggregate_f64_detailed(index, curves, as_of, |pricer, cds, disc, surv, as_of| {
            pricer.risky_pv01(cds, disc, surv, as_of)
        })
    }

    /// CS01 (approximate) aggregated by pricing mode.
    pub(crate) fn cs01(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        Ok(self.cs01_detailed(index, curves, as_of)?.total)
    }

    /// CS01 (approximate) with optional per-constituent breakdown.
    pub(crate) fn cs01_detailed(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<IndexResult<f64>> {
        self.aggregate_f64_detailed(index, curves, as_of, |_, cds, _, _, _| {
            self.compute_cds_cs01(cds, curves, as_of)
        })
    }

    fn compute_cds_cs01(
        &self,
        cds: &CreditDefaultSwap,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        let credit_id = &cds.protection.credit_curve_id;
        let discount_id = &cds.premium.discount_curve_id;
        let bump_bp = 1.0_f64;

        let pricer = CDSPricer::with_config(self.cds_config.clone());
        let hazard = curves.get_hazard(credit_id)?;
        let hazard_ref = hazard.as_ref();
        let has_par_points = hazard_ref.par_spread_points().next().is_some();

        // Par-spread re-bootstrap is the CS01 contract when par points exist.
        // A silent fallback to a direct hazard-λ shift on re-bootstrap failure
        // would change units by ≈1/(1−R) with no signal, so the error is
        // propagated instead. Curves without par points use the hazard shift
        // explicitly (the only available definition).
        let bump_hazard_for = |bp: f64| -> Result<_> {
            if has_par_points {
                bump_hazard_spreads(
                    hazard_ref,
                    curves,
                    &BumpRequest::Parallel(bp),
                    Some(discount_id),
                )
                .map_err(|e| finstack_quant_core::Error::Calibration {
                    message: format!(
                        "CDS index CS01: par-spread re-bootstrap failed for curve '{}' \
                             ({e}); refusing silent fallback to a hazard-rate bump \
                             (≈1/(1−R) unit mismatch). Fix the par-spread quotes or strip \
                             them to opt into hazard-shift CS01.",
                        credit_id
                    ),
                    category: "cs01_rebootstrap".to_string(),
                })
            } else {
                bump_hazard_shift(hazard_ref, &BumpRequest::Parallel(bp))
            }
        };

        let bumped_up = bump_hazard_for(bump_bp)?;
        let ctx_up = curves.clone().insert(bumped_up);
        let disc_up = ctx_up.get_discount(discount_id)?;
        let surv_up = ctx_up.get_hazard(credit_id)?;
        let pv_up = pricer.npv_full(cds, disc_up.as_ref(), surv_up.as_ref(), as_of)?;

        let bumped_down = bump_hazard_for(-bump_bp)?;
        let ctx_down = curves.clone().insert(bumped_down);
        let disc_down = ctx_down.get_discount(discount_id)?;
        let surv_down = ctx_down.get_hazard(credit_id)?;
        let pv_down = pricer.npv_full(cds, disc_down.as_ref(), surv_down.as_ref(), as_of)?;

        Ok((pv_up - pv_down) / (2.0 * bump_bp))
    }

    // ----- internals -----

    /// Aggregate a per-CDS scalar metric across the resolved positions.
    ///
    /// Caches discount-curve lookups by `CurveId`: the common case is that
    /// every constituent shares the index discount curve, in which case the
    /// `MarketContext::get_discount` call is performed exactly once.
    fn aggregate_f64_detailed<F>(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
        f: F,
    ) -> Result<IndexResult<f64>>
    where
        F: Fn(&CDSPricer, &CreditDefaultSwap, &DiscountCurve, &HazardCurve, Date) -> Result<f64>,
    {
        if as_of >= index.premium.end {
            return Ok(IndexResult {
                total: 0.0,
                constituents: Vec::new(),
            });
        }
        let pricer = CDSPricer::with_config(self.cds_config.clone());
        match index.pricing {
            IndexPricing::SingleCurve => {
                let cds = self.synthetic_cds(index);
                let disc = curves.get_discount(&cds.premium.discount_curve_id)?;
                let surv = curves.get_hazard(&cds.protection.credit_curve_id)?;
                let total = f(&pricer, &cds, disc.as_ref(), surv.as_ref(), as_of)?;
                Ok(IndexResult::single_curve(total))
            }
            IndexPricing::Constituents => {
                let positions = self.constituent_positions(index)?;
                let mut total = 0.0;
                let mut constituents = Vec::with_capacity(positions.len());
                let mut cached_discount: Option<(CurveId, std::sync::Arc<DiscountCurve>)> = None;
                for position in positions {
                    let disc_id = &position.cds.premium.discount_curve_id;
                    let disc = match &cached_discount {
                        Some((id, handle)) if id == disc_id => std::sync::Arc::clone(handle),
                        _ => {
                            let handle = curves.get_discount(disc_id)?;
                            cached_discount =
                                Some((disc_id.clone(), std::sync::Arc::clone(&handle)));
                            handle
                        }
                    };
                    let surv = curves.get_hazard(&position.cds.protection.credit_curve_id)?;
                    let value = f(&pricer, &position.cds, disc.as_ref(), surv.as_ref(), as_of)?;
                    total += value;
                    constituents.push(ConstituentResult {
                        credit_curve_id: position.credit_curve_id,
                        recovery_rate: position.recovery_rate,
                        weight_raw: position.weight_raw,
                        weight_effective: position.weight_effective,
                        value,
                    });
                }
                Ok(IndexResult {
                    total,
                    constituents,
                })
            }
        }
    }

    /// Aggregate a per-CDS Money metric across the resolved positions.
    ///
    /// Same dispatch as `aggregate_f64_detailed` but for currency-typed
    /// outputs. Returns the aggregate in the index notional currency.
    fn aggregate_money_detailed<F>(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
        f: F,
    ) -> Result<IndexResult<Money>>
    where
        F: Fn(&CDSPricer, &CreditDefaultSwap, &DiscountCurve, &HazardCurve, Date) -> Result<Money>,
    {
        let currency = index.notional.currency();
        if as_of >= index.premium.end {
            return Ok(IndexResult {
                total: Money::new(0.0, currency),
                constituents: Vec::new(),
            });
        }
        let pricer = CDSPricer::with_config(self.cds_config.clone());
        match index.pricing {
            IndexPricing::SingleCurve => {
                let cds = self.synthetic_cds(index);
                let disc = curves.get_discount(&cds.premium.discount_curve_id)?;
                let surv = curves.get_hazard(&cds.protection.credit_curve_id)?;
                let total = f(&pricer, &cds, disc.as_ref(), surv.as_ref(), as_of)?;
                Ok(IndexResult::single_curve(total))
            }
            IndexPricing::Constituents => {
                let positions = self.constituent_positions(index)?;
                let mut total = Money::new(0.0, currency);
                let mut constituents = Vec::with_capacity(positions.len());
                let mut cached_discount: Option<(CurveId, std::sync::Arc<DiscountCurve>)> = None;
                for position in positions {
                    let disc_id = &position.cds.premium.discount_curve_id;
                    let disc = match &cached_discount {
                        Some((id, handle)) if id == disc_id => std::sync::Arc::clone(handle),
                        _ => {
                            let handle = curves.get_discount(disc_id)?;
                            cached_discount =
                                Some((disc_id.clone(), std::sync::Arc::clone(&handle)));
                            handle
                        }
                    };
                    let surv = curves.get_hazard(&position.cds.protection.credit_curve_id)?;
                    let value = f(&pricer, &position.cds, disc.as_ref(), surv.as_ref(), as_of)?;
                    total = total.checked_add(value)?;
                    constituents.push(ConstituentResult {
                        credit_curve_id: position.credit_curve_id,
                        recovery_rate: position.recovery_rate,
                        weight_raw: position.weight_raw,
                        weight_effective: position.weight_effective,
                        value,
                    });
                }
                Ok(IndexResult {
                    total,
                    constituents,
                })
            }
        }
    }

    fn constituent_positions(&self, index: &CDSIndex) -> Result<Vec<ResolvedConstituent>> {
        if index.constituents.is_empty() {
            return Err(Error::Validation(format!(
                "CDS Index '{}' has pricing=Constituents but no constituents supplied",
                index.id
            )));
        }
        if !index.index_factor.is_finite() || index.index_factor < 0.0 {
            return Err(Error::Validation(format!(
                "CDS Index '{}' has invalid index_factor {} (must be finite and >= 0)",
                index.id, index.index_factor
            )));
        }
        for (i, c) in index.constituents.iter().enumerate() {
            if !c.weight.is_finite() || c.weight < 0.0 {
                return Err(Error::Validation(format!(
                    "CDS Index '{}' constituent #{} ('{}') has invalid weight {} \
                     (must be finite and >= 0)",
                    index.id,
                    i + 1,
                    c.credit.credit_curve_id,
                    c.weight
                )));
            }
            if !(0.0..=1.0).contains(&c.credit.recovery_rate) {
                return Err(Error::Validation(format!(
                    "CDS Index '{}' constituent #{} ('{}') has recovery_rate {} \
                     outside [0, 1]",
                    index.id,
                    i + 1,
                    c.credit.credit_curve_id,
                    c.credit.recovery_rate
                )));
            }
        }
        let sum_w: f64 = index.constituents.iter().map(|c| c.weight).sum();
        if (sum_w - 1.0).abs() > WEIGHT_SUM_TOL {
            return Err(Error::Validation(format!(
                "CDS Index '{}' constituent weights sum to {} (expected 1.0 \
                 within tolerance {})",
                index.id, sum_w, WEIGHT_SUM_TOL
            )));
        }
        // Validate index_factor consistency with defaulted weights.
        //
        // The check is two-sided when defaults are explicitly declared on
        // the constituent list, but degenerate to one-sided when the user
        // is modelling defaults purely via `index_factor` (e.g. SingleCurve
        // mode, or an aggregate post-default snapshot without constituent
        // attribution):
        //
        //   * Over-statement (`factor > 1 − sum_defaulted_weights`): always
        //     rejected — the surviving notional cannot exceed
        //     `original − defaulted`. This catches double-counting bugs
        //     irrespective of whether defaults are listed.
        //   * Under-statement (`factor < 1 − sum_defaulted_weights`):
        //     rejected only when `sum_defaulted_weights > 0`. With no
        //     declared defaults the user is allowed to encode externally-
        //     tracked defaults via `index_factor` alone (this is how
        //     SingleCurve mode and many constituents-mode tests express a
        //     post-default snapshot).
        let defaulted_sum_w: f64 = index
            .constituents
            .iter()
            .filter(|c| c.defaulted)
            .map(|c| c.weight)
            .sum();
        let expected_factor_max = 1.0 - defaulted_sum_w;
        if index.index_factor > expected_factor_max + INDEX_FACTOR_CONSISTENCY_TOL {
            return Err(Error::Validation(format!(
                "CDS Index '{}' index_factor {} exceeds 1 - sum_defaulted_weights = {} \
                 (defaulted weights total {})",
                index.id, index.index_factor, expected_factor_max, defaulted_sum_w
            )));
        }
        if defaulted_sum_w > INDEX_FACTOR_CONSISTENCY_TOL
            && index.index_factor < expected_factor_max - INDEX_FACTOR_CONSISTENCY_TOL
        {
            return Err(Error::Validation(format!(
                "CDS Index '{}' index_factor {} is below 1 - sum_defaulted_weights = {} \
                 with defaults declared (defaulted weights total {}, tolerance {}). \
                 If you intend to model additional externally-tracked defaults, mark \
                 the corresponding constituents as defaulted instead of shrinking the \
                 factor below the declared total.",
                index.id,
                index.index_factor,
                expected_factor_max,
                defaulted_sum_w,
                INDEX_FACTOR_CONSISTENCY_TOL
            )));
        }
        let active_constituents: Vec<_> =
            index.constituents.iter().filter(|c| !c.defaulted).collect();
        if active_constituents.is_empty() {
            return Ok(Vec::new());
        }
        // When some constituents have defaulted, renormalize the surviving
        // weights so they sum to 1 over the live names; otherwise leave the
        // declared weights as-is. Combined with index_factor scaling on the
        // notional, this yields per-name notional = total × index_factor × eff_w.
        let active_sum_w: f64 = active_constituents.iter().map(|c| c.weight).sum();
        let norm = if active_constituents.len() != index.constituents.len() && active_sum_w > 0.0 {
            active_sum_w
        } else {
            1.0
        };
        let mut out = Vec::with_capacity(active_constituents.len());
        for (i, con) in active_constituents.into_iter().enumerate() {
            let eff_w = con.weight / norm;
            let notional = Money::new(
                index.notional.amount() * index.index_factor * eff_w,
                index.notional.currency(),
            );
            let id = format!("{}-{:03}", index.id, i + 1);
            let cds = CreditDefaultSwap::new_isda(
                id,
                notional,
                index.side,
                index.convention,
                index.premium.spread_bp,
                index.premium.start,
                index.premium.end,
                con.credit.recovery_rate,
                index.premium.discount_curve_id.to_owned(),
                con.credit.credit_curve_id.to_owned(),
            )?;
            out.push(ResolvedConstituent {
                cds,
                credit_curve_id: con.credit.credit_curve_id.to_owned(),
                recovery_rate: con.credit.recovery_rate,
                weight_raw: con.weight,
                weight_effective: eff_w,
            });
        }
        Ok(out)
    }

    fn project_resolved_flows(
        &self,
        index: &CDSIndex,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<ProjectedIndexFlows> {
        match index.pricing {
            IndexPricing::SingleCurve => {
                let cds = self.synthetic_cds(index);
                let surv = curves.get_hazard(&cds.protection.credit_curve_id)?;
                Ok(ProjectedIndexFlows {
                    single_curve: Some(self.project_cds_flows(&cds, surv.as_ref(), as_of)?),
                    constituents: Vec::new(),
                })
            }
            IndexPricing::Constituents => {
                let constituents = self
                    .constituent_positions(index)?
                    .into_iter()
                    .map(|position| {
                        let surv = curves.get_hazard(&position.cds.protection.credit_curve_id)?;
                        Ok(ProjectedConstituentFlows {
                            flows: self.project_cds_flows(&position.cds, surv.as_ref(), as_of)?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(ProjectedIndexFlows {
                    single_curve: None,
                    constituents,
                })
            }
        }
    }

    fn synthetic_cds(&self, index: &CDSIndex) -> CreditDefaultSwap {
        // `to_synthetic_cds()` already applies `index_factor` to notional.
        let mut cds = index.to_synthetic_cds();
        // The index applies its `upfront_payment` override once at the
        // aggregate level in `npv_detailed`; clear it on the synthetic CDS
        // so `CDSPricer::npv_full` does not subtract it a second time.
        cds.pricing_overrides.market_quotes.upfront_payment = None;
        cds
    }

    fn project_cds_flows(
        &self,
        cds: &CreditDefaultSwap,
        survival: &HazardCurve,
        as_of: Date,
    ) -> Result<Vec<CashFlow>> {
        let mut schedule = CashflowProvider::cashflow_schedule(cds, &MarketContext::new(), as_of)?;
        let premium_sign = match cds.side {
            PayReceive::Pay => -1.0,
            PayReceive::Receive => 1.0,
        };
        let protection_sign = -premium_sign;
        let loss_given_default = 1.0 - cds.protection.recovery_rate;

        schedule.flows.retain(|flow| flow.date > as_of);
        let mut prev_survival = if as_of <= survival.base_date() {
            1.0
        } else {
            let t = survival.day_count().year_fraction(
                survival.base_date(),
                as_of,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;
            survival.sp(t)
        };
        let conditioning_survival = prev_survival.max(f64::EPSILON);
        let mut projected_flows = Vec::with_capacity(schedule.flows.len() * 2);
        let mut previous_premium_date = as_of;

        for flow in schedule.flows {
            if matches!(flow.kind, CFKind::Fixed | CFKind::Stub) {
                let t = survival.day_count().year_fraction(
                    survival.base_date(),
                    flow.date,
                    finstack_quant_core::dates::DayCountContext::default(),
                )?;
                let current_survival = survival.sp(t);
                let delta_default = (prev_survival - current_survival).max(0.0);
                let conditional_default = delta_default / conditioning_survival;
                let conditional_survival = current_survival / conditioning_survival;
                let projected_survival = if self.cds_config.include_accrual {
                    conditional_survival + 0.5 * conditional_default
                } else {
                    conditional_survival
                };
                let projected_premium = flow.amount.amount().abs() * projected_survival;
                if projected_premium.abs() > f64::EPSILON {
                    projected_flows.push(CashFlow {
                        amount: Money::new(
                            projected_premium * premium_sign,
                            flow.amount.currency(),
                        ),
                        ..flow
                    });
                }
                if delta_default > 0.0 {
                    let default_date =
                        Self::midpoint_default_date(survival, previous_premium_date, flow.date)?;
                    let settlement_date = Self::settlement_date_with_delay(
                        default_date,
                        cds.protection.settlement_delay,
                    );
                    projected_flows.push(CashFlow {
                        date: settlement_date,
                        reset_date: None,
                        amount: Money::new(
                            cds.notional.amount()
                                * loss_given_default
                                * conditional_default
                                * protection_sign,
                            cds.notional.currency(),
                        ),
                        kind: CFKind::DefaultedNotional,
                        accrual_factor: 0.0,
                        rate: None,
                    });
                }
                previous_premium_date = flow.date;
                prev_survival = current_survival;
            } else if flow.kind == CFKind::Fee {
                projected_flows.push(CashFlow {
                    amount: Money::new(flow.amount.amount() * premium_sign, flow.amount.currency()),
                    ..flow
                });
            }
        }

        Ok(projected_flows)
    }

    fn midpoint_default_date(
        survival: &HazardCurve,
        start_date: Date,
        end_date: Date,
    ) -> Result<Date> {
        let t_start = survival.day_count().year_fraction(
            survival.base_date(),
            start_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        let t_end = survival.day_count().year_fraction(
            survival.base_date(),
            end_date,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        Ok(date_from_hazard_time(survival, 0.5 * (t_start + t_end)))
    }

    /// Resolve the cash-settlement date for a default occurring on
    /// `default_date`, given a business-day settlement delay.
    ///
    /// `settlement_delay` is a count of *business days* (T+N per ISDA). With
    /// no holiday calendar available on this projection path, the delay is
    /// applied via [`DateExt::add_weekdays`], which advances by whole
    /// weekdays — so the result is **always a weekday**. The previous
    /// implementation converted the business-day count to calendar days via
    /// a `365 / business_days_per_year` ratio and added that as a raw
    /// `Duration`, which could land the settlement on a Saturday or Sunday.
    /// `add_weekdays` is both weekend-safe and an exact (rather than
    /// ratio-approximated) weekday count.
    fn settlement_date_with_delay(default_date: Date, settlement_delay: u16) -> Date {
        if settlement_delay == 0 {
            return default_date;
        }
        default_date.add_weekdays(i32::from(settlement_delay))
    }
}

// ========================= REGISTRY PRICER =========================

/// Registry pricer for CDS Index using the engine
pub(crate) struct SimpleCdsIndexHazardPricer {
    model_key: crate::pricer::ModelKey,
}

impl SimpleCdsIndexHazardPricer {
    /// Create a new CDS index pricer with default hazard rate model
    pub(crate) fn new() -> Self {
        Self {
            model_key: crate::pricer::ModelKey::HazardRate,
        }
    }
}

impl Default for SimpleCdsIndexHazardPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for SimpleCdsIndexHazardPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(crate::pricer::InstrumentType::CDSIndex, self.model_key)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let cds_index = instrument
            .as_any()
            .downcast_ref::<crate::instruments::credit_derivatives::cds_index::CDSIndex>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::CDSIndex,
                    instrument.key(),
                )
            })?;

        // Use the provided as_of date for valuation
        // Compute present value using the engine
        let pv = CDSIndexPricer::new()
            .npv(cds_index, market, as_of)
            .map_err(|e| {
                crate::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    crate::pricer::PricingErrorContext::default(),
                )
            })?;

        // Return stamped result
        Ok(
            crate::results::ValuationResult::stamped(cds_index.id(), as_of, pv).with_details(
                crate::results::ValuationDetails::CreditDerivative(
                    crate::results::CreditDerivativeValuationDetails {
                        model_key: format!("{:?}", self.model_key),
                        integration_method: Some("isda_standard_model".to_string()),
                    },
                ),
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use crate::cashflow::primitives::CFKind;
    use crate::instruments::common_impl::parameters::CreditParams;
    use crate::instruments::credit_derivatives::cds_index::CDSIndexConstituent;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use test_utils::{date, flat_discount_with_tenor};

    fn sample_market(as_of: Date) -> MarketContext {
        let hazard = HazardCurve::builder("CDX.NA.IG.HAZARD")
            .base_date(as_of)
            .currency(Currency::USD)
            .recovery_rate(0.40)
            .knots([(0.0, 0.02), (5.0, 0.02)])
            .build()
            .expect("hazard curve should build");

        MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 5.0))
            .insert(hazard)
    }

    #[test]
    fn constituent_positions_skip_defaulted_names_and_renormalize_live_weights() {
        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        index.index_factor = 0.6;
        index.constituents = vec![
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("LIVE", "LIVE-HAZARD"),
                weight: 0.6,
                defaulted: false,
            },
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("DEFAULTED", "DEFAULTED-HAZARD"),
                weight: 0.4,
                defaulted: true,
            },
        ];

        let positions = CDSIndexPricer::new()
            .constituent_positions(&index)
            .expect("constituent positions should resolve");

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].credit_curve_id.as_str(), "LIVE-HAZARD");
        assert!((positions[0].weight_effective - 1.0).abs() < 1e-12);
        assert!(
            (positions[0].cds.notional.amount() - index.notional.amount() * index.index_factor)
                .abs()
                < 1e-8
        );
    }

    #[test]
    fn upfront_override_respects_pay_receive_sign() {
        let as_of = date(2024, 1, 1);
        let market = sample_market(as_of);
        let pricer = CDSIndexPricer::new();
        let upfront = Money::new(125_000.0, Currency::USD);

        let mut pay = CDSIndex::example();
        pay.pricing_overrides.market_quotes.upfront_payment = Some(upfront);
        let pay_base = pricer
            .npv(&CDSIndex::example(), &market, as_of)
            .expect("base pay npv");
        let pay_with_upfront = pricer
            .npv(&pay, &market, as_of)
            .expect("pay npv with upfront");

        let mut receive = CDSIndex::example();
        receive.side = crate::instruments::credit_derivatives::cds::PayReceive::Receive;
        let mut receive_with_upfront = receive.clone();
        receive_with_upfront
            .pricing_overrides
            .market_quotes
            .upfront_payment = Some(upfront);
        let receive_base = pricer
            .npv(&receive, &market, as_of)
            .expect("base receive npv");
        let receive_total = pricer
            .npv(&receive_with_upfront, &market, as_of)
            .expect("receive npv with upfront");

        assert!((pay_with_upfront.amount() - (pay_base.amount() - upfront.amount())).abs() < 1e-8);
        assert!((receive_total.amount() - (receive_base.amount() + upfront.amount())).abs() < 1e-8);
    }

    #[test]
    fn projected_schedule_contains_premium_and_default_rows() {
        let as_of = date(2024, 1, 1);
        let market = sample_market(as_of);
        let schedule = CDSIndexPricer::new()
            .build_projected_schedule(&CDSIndex::example(), &market, as_of)
            .expect("projected schedule should build");

        assert!(schedule
            .flows
            .iter()
            .any(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub)));
        assert!(schedule
            .flows
            .iter()
            .any(|cf| cf.kind == CFKind::DefaultedNotional));
    }

    #[test]
    fn npv_close_to_discounted_projected_schedule() {
        // The cashflow schedule is an informational mid-period Riemann
        // projection while the priced NPV uses the ISDA Standard Model
        // integration via `CDSPricer`. They should agree to a few percent
        // for benign curves but are not numerically identical.
        let as_of = date(2024, 1, 1);
        let market = sample_market(as_of);
        let index = CDSIndex::example();
        let pricer = CDSIndexPricer::new();
        let schedule = pricer
            .build_projected_schedule(&index, &market, as_of)
            .expect("projected schedule should build");
        let discount = market.get_discount("USD-OIS").expect("discount curve");
        let discounted_total = schedule
            .flows
            .iter()
            .try_fold(Money::new(0.0, Currency::USD), |acc, flow| {
                let df = discount.df_between_dates(as_of, flow.date)?;
                acc.checked_add(flow.amount * df)
            })
            .expect("discounted projected rows should sum");
        let npv = pricer.npv(&index, &market, as_of).expect("index npv");

        let denom = npv.amount().abs().max(discounted_total.amount().abs());
        let rel_err = (npv.amount() - discounted_total.amount()).abs() / denom.max(1.0);
        // The Riemann-style schedule projection samples survival at coupon dates
        // and assumes a midpoint default; the priced NPV uses ISDA Standard
        // Model integration plus (when applicable) Bloomberg-CDSW clean-price
        // accrued add-back. A ~10% gap is expected for a 5y IG-style index.
        assert!(
            rel_err < 0.15,
            "schedule projection should approximate ISDA NPV within 15%: \
             npv={:.4}, schedule={:.4}, rel_err={:.4}",
            npv.amount(),
            discounted_total.amount(),
            rel_err
        );
    }

    #[test]
    fn leg_decomposition_matches_npv_single_curve() {
        // Verify npv ≈ ±(pv_protection - pv_premium) - upfront_adjustment
        // for the single-curve mode. This is the core consistency property
        // that the dual-pathway design previously violated.
        let as_of = date(2024, 1, 1);
        let market = sample_market(as_of);
        let pricer = CDSIndexPricer::new();
        let index = CDSIndex::example(); // Pay by default

        let npv = pricer.npv(&index, &market, as_of).expect("npv");
        let pv_prot = pricer
            .pv_protection_leg(&index, &market, as_of)
            .expect("pv protection");
        let pv_prem = pricer
            .pv_premium_leg(&index, &market, as_of)
            .expect("pv premium");

        // Pay: NPV = protection - premium
        let recomposed = pv_prot.amount() - pv_prem.amount();
        assert!(
            (npv.amount() - recomposed).abs() < 1e-6,
            "Pay leg decomposition: npv={:.6}, prot={:.6}, prem={:.6}, recomposed={:.6}",
            npv.amount(),
            pv_prot.amount(),
            pv_prem.amount(),
            recomposed
        );
    }

    #[test]
    fn leg_decomposition_matches_npv_constituents() {
        let as_of = date(2024, 1, 1);
        let mut market = sample_market(as_of);
        // Add per-constituent hazard curves (re-use the index curve id for
        // simplicity since flat hazards make modes agree).
        market = market.insert(
            HazardCurve::builder("HZ-A")
                .base_date(as_of)
                .currency(Currency::USD)
                .recovery_rate(0.40)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard A"),
        );
        market = market.insert(
            HazardCurve::builder("HZ-B")
                .base_date(as_of)
                .currency(Currency::USD)
                .recovery_rate(0.40)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard B"),
        );

        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        index.constituents = vec![
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("A", "HZ-A"),
                weight: 0.5,
                defaulted: false,
            },
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("B", "HZ-B"),
                weight: 0.5,
                defaulted: false,
            },
        ];

        let pricer = CDSIndexPricer::new();
        let npv = pricer.npv(&index, &market, as_of).expect("npv");
        let pv_prot = pricer
            .pv_protection_leg(&index, &market, as_of)
            .expect("pv protection");
        let pv_prem = pricer
            .pv_premium_leg(&index, &market, as_of)
            .expect("pv premium");

        let recomposed = pv_prot.amount() - pv_prem.amount();
        assert!(
            (npv.amount() - recomposed).abs() < 1e-6,
            "Constituents leg decomposition: npv={:.6}, prot={:.6}, prem={:.6}, recomposed={:.6}",
            npv.amount(),
            pv_prot.amount(),
            pv_prem.amount(),
            recomposed
        );
    }

    #[test]
    fn rejects_index_factor_inconsistent_with_defaulted_weights() {
        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        // No defaults but index_factor > 1 should be rejected.
        index.index_factor = 1.2;
        index.constituents = vec![CDSIndexConstituent {
            credit: CreditParams::corporate_standard("A", "HZ-A"),
            weight: 1.0,
            defaulted: false,
        }];
        let err = CDSIndexPricer::new()
            .constituent_positions(&index)
            .expect_err("inconsistent index_factor should fail");
        let msg = format!("{err}");
        assert!(msg.contains("index_factor"), "got: {msg}");
    }

    #[test]
    fn rejects_index_factor_understated_relative_to_declared_defaults() {
        // Q4: when the constituent list declares defaults, the
        // consistency check is two-sided — an index_factor strictly below
        // `1 − sum_defaulted_weights` would silently shrink the surviving
        // notional further than the declared defaults justify.
        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        // 20% declared default → factor should be 0.8; we set 0.5 to
        // simulate the bug.
        index.index_factor = 0.5;
        index.constituents = vec![
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("A", "HZ-A"),
                weight: 0.8,
                defaulted: false,
            },
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("B", "HZ-B"),
                weight: 0.2,
                defaulted: true,
            },
        ];
        let err = CDSIndexPricer::new()
            .constituent_positions(&index)
            .expect_err("understated index_factor should fail when defaults are declared");
        let msg = format!("{err}");
        assert!(msg.contains("index_factor"), "got: {msg}");
    }

    #[test]
    fn allows_index_factor_below_one_with_no_declared_defaults() {
        // Complementary invariant: `index_factor < 1.0` is permitted when
        // no constituents are flagged as defaulted (e.g. SingleCurve mode
        // or external default tracking). This must NOT be rejected by the
        // Q4 lower-bound check.
        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        index.index_factor = 0.8;
        index.constituents = vec![CDSIndexConstituent {
            credit: CreditParams::corporate_standard("A", "HZ-A"),
            weight: 1.0,
            defaulted: false,
        }];
        CDSIndexPricer::new()
            .constituent_positions(&index)
            .expect("factor < 1 with no declared defaults must be accepted");
    }

    #[test]
    fn with_config_changes_par_spread_methodology_single_curve() {
        // Verify that `CDSIndexPricer::with_config` plumbs the CDS pricer
        // config through to par-spread calculations on the SingleCurve
        // branch. Switching from clean-price (Bloomberg default) to
        // full-premium-AoD must produce a measurably DIFFERENT (typically
        // slightly lower) par spread, confirming the flag is honoured.
        let as_of = date(2024, 1, 1);
        let market = sample_market(as_of);
        let index = CDSIndex::example();
        assert_eq!(index.pricing, IndexPricing::SingleCurve);

        let baseline = CDSIndexPricer::new()
            .par_spread_detailed(&index, &market, as_of)
            .expect("baseline detailed");

        let alt = CDSIndexPricer::with_config(CDSPricerConfig {
            par_spread_uses_full_premium: true,
            ..CDSPricerConfig::default()
        })
        .par_spread_detailed(&index, &market, as_of)
        .expect("alt detailed");

        assert_eq!(baseline.method, ParSpreadMethod::RiskyAnnuity);
        assert_eq!(alt.method, ParSpreadMethod::FullPremiumAoD);

        assert!(baseline.total_spread_bp.is_finite() && alt.total_spread_bp.is_finite());
        // Numerator (protection PV) is independent of the denominator
        // convention and must match exactly.
        assert!(
            (baseline.numerator_protection_pv.amount() - alt.numerator_protection_pv.amount())
                .abs()
                < 1e-6,
            "protection-leg PV should not depend on par-spread methodology"
        );
        // The denominator must change when the flag flips. Full-premium
        // includes accrual-on-default, so its denom is strictly larger and
        // its par spread strictly smaller for a positive-spread credit.
        assert!(
            alt.denominator > baseline.denominator,
            "full-premium denominator should exceed clean-price denom (includes AoD): \
             baseline={}, alt={}",
            baseline.denominator,
            alt.denominator
        );
        assert!(
            alt.total_spread_bp < baseline.total_spread_bp,
            "full-premium par spread should be strictly smaller than clean-price: \
             baseline={}, alt={}",
            baseline.total_spread_bp,
            alt.total_spread_bp
        );
        // And the numerator/denominator/total-bp triple must be internally
        // consistent (regression guard for the bug where the reported
        // denominator was risky_annuity while total_spread_bp came from
        // pricer.par_spread()).
        for r in [&baseline, &alt] {
            let implied =
                r.numerator_protection_pv.amount() / r.denominator * BASIS_POINTS_PER_UNIT;
            assert!(
                (implied - r.total_spread_bp).abs() < 1e-6,
                "IndexParSpreadResult fields must be internally consistent: \
                 numerator={}, denominator={}, implied={}, total_bp={}",
                r.numerator_protection_pv.amount(),
                r.denominator,
                implied,
                r.total_spread_bp
            );
        }
    }

    #[test]
    fn with_config_changes_par_spread_methodology_constituents() {
        // Same invariants on the Constituents branch — this is the path
        // that previously hardcoded `risky_annuity` and silently ignored
        // the config flag.
        let as_of = date(2024, 1, 1);
        let mut market = sample_market(as_of);
        market = market.insert(
            HazardCurve::builder("HZ-A")
                .base_date(as_of)
                .currency(Currency::USD)
                .recovery_rate(0.40)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard A"),
        );
        market = market.insert(
            HazardCurve::builder("HZ-B")
                .base_date(as_of)
                .currency(Currency::USD)
                .recovery_rate(0.40)
                .knots([(0.0, 0.02), (5.0, 0.02)])
                .build()
                .expect("hazard B"),
        );

        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        index.constituents = vec![
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("A", "HZ-A"),
                weight: 0.5,
                defaulted: false,
            },
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("B", "HZ-B"),
                weight: 0.5,
                defaulted: false,
            },
        ];

        let baseline = CDSIndexPricer::new()
            .par_spread_detailed(&index, &market, as_of)
            .expect("baseline constituents");
        let alt = CDSIndexPricer::with_config(CDSPricerConfig {
            par_spread_uses_full_premium: true,
            ..CDSPricerConfig::default()
        })
        .par_spread_detailed(&index, &market, as_of)
        .expect("alt constituents");

        assert_eq!(baseline.method, ParSpreadMethod::RiskyAnnuity);
        assert_eq!(alt.method, ParSpreadMethod::FullPremiumAoD);
        assert!(
            (baseline.total_spread_bp - alt.total_spread_bp).abs() > 0.0,
            "constituents-mode par spread must respond to par_spread_uses_full_premium: \
             baseline={}, alt={}",
            baseline.total_spread_bp,
            alt.total_spread_bp
        );
    }

    #[test]
    fn rejects_negative_constituent_weight() {
        let mut index = CDSIndex::example();
        index.pricing = IndexPricing::Constituents;
        index.constituents = vec![
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("A", "HZ-A"),
                weight: 1.5,
                defaulted: false,
            },
            CDSIndexConstituent {
                credit: CreditParams::corporate_standard("B", "HZ-B"),
                weight: -0.5,
                defaulted: false,
            },
        ];
        let err = CDSIndexPricer::new()
            .constituent_positions(&index)
            .expect_err("negative weight should fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid weight"),
            "expected weight error, got: {msg}"
        );
    }

    /// Item 10: the settlement-delay fallback must never land on a weekend.
    /// `settlement_date_with_delay` applies the business-day delay via
    /// weekday arithmetic, so the result is always a weekday regardless of
    /// where the default date falls.
    #[test]
    fn settlement_delay_fallback_never_lands_on_weekend() {
        use finstack_quant_core::dates::DateExt;
        // Cover every weekday/weekend default date across a fortnight so the
        // weekend-crossing cases are exercised.
        let start = date(2025, 1, 1); // Wednesday
        for offset in 0..14 {
            let default_date = start + time::Duration::days(offset);
            for delay in [1_u16, 2, 3, 5] {
                let settle = CDSIndexPricer::settlement_date_with_delay(default_date, delay);
                assert!(
                    !settle.is_weekend(),
                    "settlement (default {default_date:?}, T+{delay}) must be a \
                     weekday, got {settle:?}"
                );
                // The settlement must be strictly after the default date.
                assert!(
                    settle > default_date,
                    "T+{delay} settlement must follow the default date"
                );
            }
        }
        // T+0 is a pass-through (same date, no business-day roll).
        let d = date(2025, 1, 4); // Saturday
        assert_eq!(
            CDSIndexPricer::settlement_date_with_delay(d, 0),
            d,
            "T+0 settlement is a pass-through of the default date"
        );
    }
}
