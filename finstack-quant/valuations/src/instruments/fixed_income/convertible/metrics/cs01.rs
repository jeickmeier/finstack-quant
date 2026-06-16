//! CS01 calculator for convertible bonds.
//!
//! Convertible bonds are hybrid instruments with both debt and equity
//! components and are typically priced without a separate hazard curve, so
//! this calculator deviates from the [canonical CS01 convention][canonical]
//! (par CDS curve bump). It instead applies a parallel 1 bp shock to the
//! configured **credit curve ID** (resolved against the discount-curve
//! container, which may already embed the credit spread) and uses the same
//! symmetric (central) finite difference as the canonical helpers:
//!
//! ```text
//! CS01 = (PV(s + 1bp) - PV(s - 1bp)) / 2
//! ```
//!
//! When `credit_curve_id` is `None`, credit risk is not modelled
//! independently and CS01 is reported as `0.0` (bumping a generic discount
//! curve would produce rho, not CS01).
//!
//! Sign convention is identical to the canonical reference:
//! - Long convertible → CS01 negative (wider spreads reduce PV).
//! - Short convertible → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::bump_discount_curve_parallel;
use crate::metrics::sensitivities::config::{format_bucket_label_cow, STANDARD_BUCKETS_YEARS};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;
use std::borrow::Cow;
use std::sync::Arc;

/// CS01 calculator for convertible bonds.
pub(crate) struct Cs01Calculator;

impl MetricCalculator for Cs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        // Check if expired
        if as_of >= bond.maturity {
            return Ok(0.0);
        }

        let bump_bp = 1.0;

        let Some(curve_to_bump) = &bond.credit_curve_id else {
            return Ok(0.0);
        };

        let curves_up = bump_discount_curve_parallel(&context.curves, curve_to_bump, bump_bp)?;
        let curves_down = bump_discount_curve_parallel(&context.curves, curve_to_bump, -bump_bp)?;

        let pv_up = bond.value(&curves_up, as_of)?.amount();
        let pv_down = bond.value(&curves_down, as_of)?.amount();

        let cs01 = (pv_up - pv_down) / 2.0;

        Ok(cs01)
    }
}

/// Triangular key-rate bump spec for bucket `i` of `buckets`.
///
/// The triangular weight runs `buckets[i-1] → buckets[i] → buckets[i+1]`, so
/// the sum of all bucket bumps is a parallel bump (partition of unity) — hence
/// the per-bucket CS01s sum to the parallel CS01. The wing buckets use the
/// dedicated half-triangle constructors: a `prev = 0.0` sentinel would break
/// the partition below the first bucket, and an infinite `next` sentinel is
/// rejected by the curve bump paths (NaN weight beyond the last bucket).
fn key_rate_spec(i: usize, bump_bp: f64, buckets: &[f64]) -> BumpSpec {
    let target = buckets[i];
    if i == 0 && buckets.len() == 1 {
        return BumpSpec::parallel_bp(bump_bp);
    }
    if i == 0 {
        return BumpSpec::triangular_key_rate_first_bp(target, buckets[1], bump_bp);
    }
    let prev = buckets[i - 1];
    if i + 1 == buckets.len() {
        return BumpSpec::triangular_key_rate_last_bp(prev, target, bump_bp);
    }
    BumpSpec::triangular_key_rate_bp(prev, target, buckets[i + 1], bump_bp)
}

/// Key-rate (bucketed) CS01 calculator for convertible bonds.
///
/// Mirrors [`Cs01Calculator`] but applies a *triangular key-rate* shock to the
/// credit curve at each standard bucket tenor instead of a single parallel
/// shock, producing a per-tenor CS01 series. The per-bucket CS01s sum (within
/// the usual key-rate tolerance) to the parallel CS01.
///
/// The series is stored under `bucketed_cs01::{credit_curve_id}` so downstream
/// consumers read it exactly like the generic `BucketedCs01`. Like the parallel
/// calculator, when `credit_curve_id` is `None` (or the bond has expired) CS01
/// is `0.0` and no series is stored.
pub(crate) struct BucketedCs01Calculator;

impl MetricCalculator for BucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        let (credit_curve_id, maturity) = {
            let bond: &ConvertibleBond = context.instrument_as()?;
            (bond.credit_curve_id.clone(), bond.maturity)
        };

        if as_of >= maturity {
            return Ok(0.0);
        }
        let Some(curve_id) = credit_curve_id else {
            return Ok(0.0);
        };

        let bump_bp = 1.0;
        // Clone the Arcs so no borrow of `context` outlives the reprice loop —
        // `store_bucketed_series` below needs `&mut context`.
        let curves = Arc::clone(&context.curves);
        let instrument = Arc::clone(&context.instrument);

        let mut series: Vec<(Cow<'static, str>, f64)> = Vec::new();
        let mut total = 0.0;
        for (i, &t) in STANDARD_BUCKETS_YEARS.iter().enumerate() {
            let up = curves.bump([MarketBump::Curve {
                id: curve_id.clone(),
                spec: key_rate_spec(i, bump_bp, &STANDARD_BUCKETS_YEARS),
            }])?;
            let down = curves.bump([MarketBump::Curve {
                id: curve_id.clone(),
                spec: key_rate_spec(i, -bump_bp, &STANDARD_BUCKETS_YEARS),
            }])?;
            let pv_up = instrument.value(&up, as_of)?.amount();
            let pv_down = instrument.value(&down, as_of)?.amount();
            // Central difference, $ per bp of spread at this bucket.
            let cs01 = (pv_up - pv_down) / (2.0 * bump_bp);
            series.push((format_bucket_label_cow(t), cs01));
            total += cs01;
        }

        context.store_bucketed_series(
            MetricId::custom(format!("bucketed_cs01::{}", curve_id.as_str())),
            series,
        );
        Ok(total)
    }
}
