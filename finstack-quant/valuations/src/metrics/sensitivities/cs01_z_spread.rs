//! Z-spread CS01 fallback for discounting-priced credit instruments.
//!
//! Some credit-risky instruments (term loans, and revolving-credit facilities
//! with no hazard curve) are valued by **deterministic cashflow discounting**:
//! their pricer discounts contractual cashflows on a single discount curve and
//! never consumes a hazard/credit curve. For those instruments the canonical
//! [par-spread / hazard-rebootstrap CS01][canonical] is identically zero — a
//! hazard bump never moves the PV — so credit-spread risk would silently read
//! as `0.0`.
//!
//! This module computes CS01 for those instruments using the **market-standard
//! z-spread bump**, mirroring the bond z-spread fallback in
//! [`bond::metrics::cs01`]:
//!
//! ```text
//! CS01 = (PV_z(z* + 1bp) - PV_z(z* - 1bp)) / 2
//! ```
//!
//! where `PV_z(z) = Σ CF_i · DF(settlement, T_i) · shift(z, t_i)` and `shift`
//! adds the spread `z` to the periodically-compounded zero rate (see
//! [`z_spread_discount_factor`]). The anchor `z*` is solved so that `PV_z(z*)`
//! reproduces the instrument's quoted price when one is supplied; otherwise the
//! anchor is `z* = 0` (the model PV), so unquoted instruments report the
//! spread sensitivity at the current market level.
//!
//! # Relationship to DV01
//!
//! For a **fixed-rate** instrument discounted on a single curve, the z-spread
//! bump and a parallel discount-curve DV01 bump move the same zero rates, so
//! CS01 and DV01 have very similar magnitude — this is expected (spread
//! duration ≈ rate duration for fixed cashflows) and matches the bond z-spread
//! fallback. For a **floating-rate** instrument the two differ: DV01 also
//! reprojects the floating coupons off the forward curve, whereas the z-spread
//! shift only touches discounting (coupons are held fixed), isolating the
//! credit-spread component.
//!
//! # Units and sign convention
//!
//! CS01 is in **currency units per basis point**. A long credit holder loses
//! value when spreads widen, so a long loan reports a **negative** CS01 — the
//! same convention as every other CS01 calculator in the workspace.
//!
//! [canonical]: crate::metrics::sensitivities::cs01
//! [`bond::metrics::cs01`]: crate::instruments::fixed_income::bond::metrics::cs01
//! [`z_spread_discount_factor`]: crate::instruments::fixed_income::bond::metrics::price_yield_spread::z_spread::z_spread_discount_factor

use std::marker::PhantomData;
use std::sync::Arc;

use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

use crate::instruments::common_impl::traits::{CurveDependencies, Instrument};
use crate::instruments::fixed_income::bond::metrics::price_yield_spread::z_spread::z_spread_discount_factor;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::{
    sensitivity_central_diff, validate_buckets_strictly_increasing,
};
use crate::metrics::{
    GenericBucketedCs01, GenericParallelCs01, MetricCalculator, MetricContext, MetricId,
};

/// Cashflow inputs an instrument exposes for z-spread CS01.
///
/// The flows are the **holder-view** cashflows the discounting pricer values
/// (e.g. coupons, fees, amortization and redemptions, with PIK and pre-anchor
/// flows already excluded), so `PV_z(0)` reproduces the model PV.
pub(crate) struct ZSpreadCs01Inputs {
    /// Date the PV is anchored to (settlement date for term loans, valuation
    /// date for revolvers). Discount factors and z-spread year fractions are
    /// measured from here, matching the discounting pricer.
    pub settlement: Date,
    /// Discount curve the spread is layered on top of.
    pub discount_curve_id: CurveId,
    /// Compounding frequency for the z-spread shift (payments per year).
    pub compounds_per_year: f64,
    /// Holder-view cashflows `(date, amount)` the pricer discounts.
    pub flows: Vec<(Date, Money)>,
}

/// Instruments whose CS01 is computed via a z-spread (parallel discount-curve
/// spread) bump because their pricer discounts on a single curve.
pub(crate) trait ZSpreadCs01 {
    /// Build the holder-view cashflow inputs for the z-spread bump.
    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<ZSpreadCs01Inputs>;

    /// Quoted dirty price (settlement, currency units) used to anchor the
    /// spread solve. Returns `None` to anchor the sensitivity at the model PV
    /// (`z* = 0`).
    fn z_spread_cs01_quoted_dirty(
        &self,
        _curves: &MarketContext,
        _as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        Ok(None)
    }
}

/// Precomputed `(year_fraction, base_df, amount)` for each future cashflow,
/// measured from the settlement anchor on the discount curve.
type CachedFlow = (f64, f64, f64);

/// Build the cached future-flow tuples for repeated z-spread repricing.
fn cache_flows(
    inputs: &ZSpreadCs01Inputs,
    disc: &finstack_quant_core::market_data::term_structures::DiscountCurve,
) -> finstack_quant_core::Result<Vec<CachedFlow>> {
    let dc = disc.day_count();
    inputs
        .flows
        .iter()
        .filter(|(d, _)| *d > inputs.settlement)
        .map(|(d, amt)| -> finstack_quant_core::Result<CachedFlow> {
            let t = dc.year_fraction(inputs.settlement, *d, DayCountContext::default())?;
            let df = disc.df_between_dates(inputs.settlement, *d)?;
            Ok((t, df, amt.amount()))
        })
        .collect()
}

/// Present value of cached flows with a uniform spread `z` (decimal) applied to
/// every flow.
fn price_at_spread(
    cached: &[CachedFlow],
    z: f64,
    compounds_per_year: f64,
) -> finstack_quant_core::Result<f64> {
    let mut pv = NeumaierAccumulator::new();
    for (t, df_base, amt) in cached {
        let df_z = z_spread_discount_factor(*df_base, *t, z, compounds_per_year)?;
        pv.add(amt * df_z);
    }
    Ok(pv.total())
}

/// Present value with the base spread `base_z`, bumping only the flows whose
/// index is in `bumped` by an additional `dz`.
fn price_bucket_bumped(
    cached: &[CachedFlow],
    base_z: f64,
    bumped: &[usize],
    dz: f64,
    compounds_per_year: f64,
) -> finstack_quant_core::Result<f64> {
    let mut pv = NeumaierAccumulator::new();
    for (i, (t, df_base, amt)) in cached.iter().enumerate() {
        let z = if bumped.contains(&i) {
            base_z + dz
        } else {
            base_z
        };
        let df_z = z_spread_discount_factor(*df_base, *t, z, compounds_per_year)?;
        pv.add(amt * df_z);
    }
    Ok(pv.total())
}

/// Solve the anchor spread `z*` so that `PV_z(z*)` matches `target` (a quoted
/// dirty price). Falls back to `0.0` if the solve fails to converge.
fn solve_anchor_spread(cached: &[CachedFlow], compounds_per_year: f64, target: f64) -> f64 {
    let objective = |z: f64| -> f64 {
        match price_at_spread(cached, z, compounds_per_year) {
            Ok(pv) => pv - target,
            // Diverge positively for spreads below the compounding floor so
            // Brent does not manufacture a spurious bracket.
            Err(_) => 1e12,
        }
    };
    let solver = BrentSolver::new()
        .tolerance(1e-10)
        .initial_bracket_size(Some(0.05)); // ±500 bp
                                           // A failed anchor solve must not silently anchor CS01 at the model-PV point
                                           // (z=0): that computes the bucket sensitivities at the wrong base spread with
                                           // no signal to the caller. Surface the failure as a warning before falling
                                           // back so a bad mark / illiquid quote is observable in production.
    solver.solve(objective, 0.0).unwrap_or_else(|err| {
        tracing::warn!(
            error = %err,
            target,
            "Z-spread anchor solve failed to converge; falling back to z=0.0 \
             (CS01 buckets will be anchored at the model-PV point, not the \
             quoted price)"
        );
        0.0
    })
}

/// Resolve the anchor spread for an instrument: solve to the quoted price when
/// present, else `0.0` (model PV).
fn anchor_spread<I: ZSpreadCs01>(
    instrument: &I,
    curves: &MarketContext,
    as_of: Date,
    cached: &[CachedFlow],
    compounds_per_year: f64,
) -> finstack_quant_core::Result<f64> {
    Ok(
        match instrument.z_spread_cs01_quoted_dirty(curves, as_of)? {
            Some(target) => solve_anchor_spread(cached, compounds_per_year, target),
            None => 0.0,
        },
    )
}

/// Assign each cached flow to a key-rate bucket by its year fraction.
///
/// A flow at time `t` is assigned to the first bucket boundary `>= t`, or the
/// final bucket when `t` exceeds the grid. Every flow lands in exactly one
/// bucket, so the per-bucket CS01s sum to the parallel CS01.
fn assign_buckets(cached: &[CachedFlow], buckets: &[f64]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); buckets.len()];
    if buckets.is_empty() {
        return groups;
    }
    for (i, (t, _, _)) in cached.iter().enumerate() {
        let idx = buckets
            .iter()
            .position(|b| *b >= *t)
            .unwrap_or(buckets.len() - 1);
        groups[idx].push(i);
    }
    groups
}

/// Resolve whether the instrument declares a credit (hazard) curve.
fn has_credit_curve<I: CurveDependencies>(instrument: &I) -> finstack_quant_core::Result<bool> {
    Ok(!instrument.curve_dependencies()?.credit_curves.is_empty())
}

// =============================================================================
// Parallel z-spread CS01
// =============================================================================

/// Parallel CS01 via a z-spread bump for discounting-priced instruments.
///
/// When `delegate_to_hazard_when_credit_curve` is set and the instrument
/// declares a credit curve, this delegates to [`GenericParallelCs01`] (the
/// canonical hazard-rebootstrap CS01) — used by revolving credit, whose pricer
/// consumes the hazard curve when one is present. Otherwise CS01 is the
/// z-spread bump, keyed `cs01::<instrument_id>`.
pub(crate) struct ZSpreadParallelCs01<I> {
    delegate_to_hazard_when_credit_curve: bool,
    _phantom: PhantomData<I>,
}

impl<I> ZSpreadParallelCs01<I> {
    /// Always use the z-spread bump (the instrument's pricer never consumes a
    /// hazard curve — e.g. term loans).
    pub(crate) fn always() -> Self {
        Self {
            delegate_to_hazard_when_credit_curve: false,
            _phantom: PhantomData,
        }
    }

    /// Delegate to the canonical hazard CS01 when a credit curve is present,
    /// falling back to the z-spread bump otherwise (e.g. revolving credit).
    pub(crate) fn hazard_when_credit_curve() -> Self {
        Self {
            delegate_to_hazard_when_credit_curve: true,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for ZSpreadParallelCs01<I>
where
    I: Instrument + CurveDependencies + ZSpreadCs01 + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst_arc = Arc::clone(&context.instrument);
        let instrument = inst_arc
            .as_any()
            .downcast_ref::<I>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        if self.delegate_to_hazard_when_credit_curve && has_credit_curve(instrument)? {
            return GenericParallelCs01::<I>::default().calculate(context);
        }

        let curves = Arc::clone(&context.curves);
        let inputs = instrument.z_spread_cs01_inputs(curves.as_ref(), context.as_of)?;
        let disc = curves.get_discount(inputs.discount_curve_id.as_str())?;
        let cached = cache_flows(&inputs, disc.as_ref())?;

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .credit_spread_bump_bp;

        let inst_id = instrument.id();
        let cs01 = if cached.is_empty() {
            0.0
        } else {
            let base_z = anchor_spread(
                instrument,
                curves.as_ref(),
                context.as_of,
                &cached,
                inputs.compounds_per_year,
            )?;
            let dz = bump_bp * 1e-4;
            let pv_up = price_at_spread(&cached, base_z + dz, inputs.compounds_per_year)?;
            let pv_down = price_at_spread(&cached, base_z - dz, inputs.compounds_per_year)?;
            sensitivity_central_diff(pv_up, pv_down, bump_bp)
        };

        context
            .computed
            .insert(MetricId::custom(format!("cs01::{}", inst_id)), cs01);
        Ok(cs01)
    }
}

// =============================================================================
// Bucketed (key-rate) z-spread CS01
// =============================================================================

/// Key-rate CS01 via per-tenor z-spread bumps for discounting-priced
/// instruments.
///
/// Bumps the z-spread one tenor bucket at a time and stores a per-bucket series
/// `bucketed_cs01::<instrument_id>::<tenor>` whose sum reconciles to the
/// parallel z-spread CS01. Delegates to [`GenericBucketedCs01`] when configured
/// and a credit curve is present (revolving credit).
pub(crate) struct ZSpreadBucketedCs01<I> {
    delegate_to_hazard_when_credit_curve: bool,
    _phantom: PhantomData<I>,
}

impl<I> ZSpreadBucketedCs01<I> {
    /// Always use the z-spread bump (term loans).
    pub(crate) fn always() -> Self {
        Self {
            delegate_to_hazard_when_credit_curve: false,
            _phantom: PhantomData,
        }
    }

    /// Delegate to the canonical hazard bucketed CS01 when a credit curve is
    /// present, falling back to the z-spread bump otherwise (revolving credit).
    pub(crate) fn hazard_when_credit_curve() -> Self {
        Self {
            delegate_to_hazard_when_credit_curve: true,
            _phantom: PhantomData,
        }
    }
}

impl<I> MetricCalculator for ZSpreadBucketedCs01<I>
where
    I: Instrument + CurveDependencies + ZSpreadCs01 + 'static,
{
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst_arc = Arc::clone(&context.instrument);
        let instrument = inst_arc
            .as_any()
            .downcast_ref::<I>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        if self.delegate_to_hazard_when_credit_curve && has_credit_curve(instrument)? {
            return GenericBucketedCs01::<I>::default().calculate(context);
        }

        let curves = Arc::clone(&context.curves);
        let inputs = instrument.z_spread_cs01_inputs(curves.as_ref(), context.as_of)?;
        let disc = curves.get_discount(inputs.discount_curve_id.as_str())?;
        let cached = cache_flows(&inputs, disc.as_ref())?;

        let defaults =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?;
        let buckets = defaults.cs01_buckets_years;
        let bump_bp = defaults.credit_spread_bump_bp;
        validate_buckets_strictly_increasing(&buckets)?;

        let inst_id = instrument.id();
        let series_id = MetricId::custom(format!("bucketed_cs01::{}", inst_id));

        if cached.is_empty() {
            context.store_bucketed_series(
                series_id,
                Vec::<(std::borrow::Cow<'static, str>, f64)>::new(),
            );
            return Ok(0.0);
        }

        let base_z = anchor_spread(
            instrument,
            curves.as_ref(),
            context.as_of,
            &cached,
            inputs.compounds_per_year,
        )?;
        let dz = bump_bp * 1e-4;
        let groups = assign_buckets(&cached, &buckets);

        let mut series: Vec<(std::borrow::Cow<'static, str>, f64)> =
            Vec::with_capacity(buckets.len());
        let mut total = NeumaierAccumulator::new();
        for (b, group) in groups.iter().enumerate() {
            let label = super::config::format_bucket_label_cow(buckets[b]);
            let cs01 = if group.is_empty() {
                0.0
            } else {
                let pv_up =
                    price_bucket_bumped(&cached, base_z, group, dz, inputs.compounds_per_year)?;
                let pv_down =
                    price_bucket_bumped(&cached, base_z, group, -dz, inputs.compounds_per_year)?;
                sensitivity_central_diff(pv_up, pv_down, bump_bp)
            };
            series.push((label, cs01));
            total.add(cs01);
        }

        context.store_bucketed_series(series_id, series);
        Ok(total.total())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `PV_z(0)` equals the undiscounted-by-spread base PV (`Σ amt · df_base`).
    #[test]
    fn price_at_spread_zero_matches_base_pv() {
        let cached = vec![(0.5, 0.98, 100.0), (2.0, 0.92, 100.0), (5.0, 0.80, 1000.0)];
        let base: f64 = cached.iter().map(|(_, df, amt)| df * amt).sum();
        let pv = price_at_spread(&cached, 0.0, 4.0).unwrap();
        assert!((pv - base).abs() < 1e-9, "pv={pv} base={base}");
    }

    /// PV is strictly decreasing in the spread for positive cashflows.
    #[test]
    fn price_at_spread_decreases_with_spread() {
        let cached = vec![(1.0, 0.97, 50.0), (3.0, 0.90, 1050.0)];
        let lo = price_at_spread(&cached, 0.0, 1.0).unwrap();
        let hi = price_at_spread(&cached, 0.01, 1.0).unwrap();
        assert!(hi < lo, "widening spread must reduce PV: lo={lo} hi={hi}");
    }

    /// The anchor solve recovers the spread that produced a target price.
    #[test]
    fn solve_anchor_spread_roundtrips() {
        let cached = vec![(1.0, 0.97, 50.0), (2.0, 0.94, 50.0), (3.0, 0.90, 1050.0)];
        let m = 1.0;
        let z_true = 0.0123;
        let target = price_at_spread(&cached, z_true, m).unwrap();
        let z = solve_anchor_spread(&cached, m, target);
        assert!((z - z_true).abs() < 1e-8, "z={z} z_true={z_true}");
    }

    /// Every flow is assigned to exactly one bucket (first boundary >= t, else
    /// the final bucket).
    #[test]
    fn assign_buckets_partitions_all_flows() {
        let cached = vec![
            (0.2, 1.0, 1.0),
            (1.0, 1.0, 1.0),
            (4.0, 1.0, 1.0),
            (40.0, 1.0, 1.0),
        ];
        let buckets = vec![0.25, 0.5, 1.0, 5.0, 30.0];
        let groups = assign_buckets(&cached, &buckets);
        assert_eq!(groups[0], vec![0]); // 0.2 -> 0.25
        assert_eq!(groups[2], vec![1]); // 1.0 -> 1.0
        assert_eq!(groups[3], vec![2]); // 4.0 -> 5.0
        assert_eq!(groups[4], vec![3]); // 40.0 -> last
        let total: usize = groups.iter().map(Vec::len).sum();
        assert_eq!(total, cached.len());
    }

    /// Per-bucket spread bumps sum exactly to the parallel bump (PV is additive
    /// over flows, each flow's contribution depends only on its own spread).
    #[test]
    fn bucketed_bumps_reconcile_to_parallel() {
        let cached = vec![(0.5, 0.98, 100.0), (2.0, 0.92, 100.0), (5.0, 0.80, 1000.0)];
        let buckets = vec![1.0, 3.0, 10.0];
        let groups = assign_buckets(&cached, &buckets);
        let m = 4.0;
        let dz = 1e-4;

        let parallel =
            price_at_spread(&cached, dz, m).unwrap() - price_at_spread(&cached, -dz, m).unwrap();

        let mut bucket_sum = 0.0;
        for group in &groups {
            let up = price_bucket_bumped(&cached, 0.0, group, dz, m).unwrap();
            let down = price_bucket_bumped(&cached, 0.0, group, -dz, m).unwrap();
            bucket_sum += up - down;
        }

        assert!(
            (parallel - bucket_sum).abs() < 1e-9,
            "parallel={parallel} bucket_sum={bucket_sum}"
        );
    }
}
