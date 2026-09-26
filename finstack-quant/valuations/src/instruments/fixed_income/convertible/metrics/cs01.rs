//! CS01 calculator for convertible bonds.
//!
//! Convertible bonds are hybrid instruments with both debt and equity
//! components. Their issuer `credit_curve_id` names a `HazardCurve`, but the
//! curve is usually entered directly rather than bootstrapped from a CDS
//! strip, so this calculator deviates from the [canonical CS01
//! convention][canonical] (par CDS curve bump with re-bootstrap). It applies a
//! parallel 1 bp **spread** shock straight to the hazard curve — a hazard shift
//! of `1bp / (1 − R_curve)` (credit triangle), no re-bootstrap — and uses the
//! same symmetric (central) finite difference as the canonical helpers:
//!
//! ```text
//! CS01 = (PV(s + 1bp) - PV(s - 1bp)) / 2
//! ```
//!
//! When `credit_curve_id` is `None`, the calculator falls back to the
//! **z-spread bump method** (the market convention for bonds without a
//! hazard curve): a zero-hazard, zero-recovery synthetic curve is inserted
//! under a derived ID and a bond clone is repointed at it (zero recovery), so
//! a 1 bp spread shock is a 1 bp hazard shift that hits only the
//! cash-component (Tsiveriotis-Zhang risky) discounting while leaving the
//! equity leg's drift and discounting unchanged — a true credit-spread
//! sensitivity, not rho. A hazard rate cannot go below zero, so the fallback
//! uses the forward difference `PV(s + 1bp) − PV(s)`, the same deviation the
//! bond z-spread CS01 documents (`O(bump²)` from the central value). The
//! fallback is logged at debug level.
//!
//! Sign convention is identical to the canonical reference:
//! - Long convertible → CS01 negative (wider spreads reduce PV).
//! - Short convertible → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::convertible::ConvertibleBond;
use crate::metrics::sensitivities::config::{format_bucket_label_cow, STANDARD_BUCKETS_YEARS};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::borrow::Cow;

/// Finite-difference stencil for a spread shock.
#[derive(Clone, Copy)]
enum Stencil {
    /// `(PV(+h) − PV(−h)) / 2h`: the canonical central difference.
    Central,
    /// `(PV(+h) − PV(0)) / h`: the z-spread fallback, whose zero-hazard curve
    /// cannot be shocked downward.
    Forward,
}

/// Pricing setup for a CS01 shock: the bond to reprice, the market holding
/// the hazard curve to shock, that curve's ID and the stencil.
struct Cs01Setup {
    bond: ConvertibleBond,
    market: MarketContext,
    curve_id: CurveId,
    stencil: Stencil,
}

/// Resolve the curve to shock: the bond's own hazard curve, or the z-spread
/// fallback for a convertible with no credit curve.
///
/// The fallback inserts a zero-hazard, zero-recovery curve under a derived ID
/// (knots on the standard CS01 buckets so key-rate shocks resolve per bucket,
/// base date and day count of the risk-free curve) and repoints a bond clone
/// at it with explicit zero recovery. Its base PV equals the bond's PV with no
/// credit curve, and a 1 bp spread shock is a 1 bp z-spread on the cash
/// component.
///
/// # Arguments
///
/// * `bond` - Convertible bond to shock; cloned (and, for the fallback,
///   repointed at the synthetic curve).
/// * `market` - Base market context supplying the risk-free discount curve and,
///   when set, the bond's hazard curve; cloned with the synthetic curve
///   inserted for the fallback.
fn cs01_setup(bond: &ConvertibleBond, market: &MarketContext) -> Result<Cs01Setup> {
    if let Some(curve_id) = &bond.credit_curve_id {
        return Ok(Cs01Setup {
            bond: bond.clone(),
            market: market.clone(),
            curve_id: curve_id.clone(),
            stencil: Stencil::Central,
        });
    }
    tracing::debug!(
        instrument = %bond.id.as_str(),
        "convertible CS01: no credit curve configured; falling back to a z-spread \
         bump of the cash component"
    );
    let rf_curve = market.get_discount(bond.discount_curve_id.as_str())?;
    let curve_id = CurveId::new(format!("{}::zspread-hazard", bond.id.as_str()));
    let synthetic = HazardCurve::builder(curve_id.clone())
        .base_date(rf_curve.base_date())
        .day_count(rf_curve.day_count())
        .recovery_rate(0.0)
        .knots(STANDARD_BUCKETS_YEARS.iter().map(|&t| (t, 0.0)))
        .build()?;
    let mut shocked = bond.clone();
    shocked.credit_curve_id = Some(curve_id.clone());
    // Pure z-spread bump: explicit zero recovery makes the full shock hit the
    // cash component without introducing an implicit recovery assumption.
    shocked.recovery_rate = Some(0.0);
    Ok(Cs01Setup {
        bond: shocked,
        market: market.clone().insert(synthetic),
        curve_id,
        stencil: Stencil::Forward,
    })
}

impl Cs01Setup {
    /// Reprice after shocking the hazard curve with `spec`.
    fn pv_with(&self, spec: BumpSpec, as_of: Date) -> Result<f64> {
        let bumped = self.market.bump([MarketBump::Curve {
            id: self.curve_id.clone(),
            spec,
        }])?;
        Ok(self.bond.value(&bumped, as_of)?.amount())
    }

    /// CS01 in currency per 1 bp of spread for the shock built by `spec_for`
    /// from a signed basis-point size.
    fn cs01(&self, as_of: Date, spec_for: impl Fn(f64) -> BumpSpec) -> Result<f64> {
        let bump_bp = 1.0;
        let pv_up = self.pv_with(spec_for(bump_bp), as_of)?;
        match self.stencil {
            Stencil::Central => {
                let pv_down = self.pv_with(spec_for(-bump_bp), as_of)?;
                Ok((pv_up - pv_down) / (2.0 * bump_bp))
            }
            Stencil::Forward => {
                let pv_base = self.bond.value(&self.market, as_of)?.amount();
                Ok((pv_up - pv_base) / bump_bp)
            }
        }
    }
}

/// CS01 calculator for convertible bonds.
pub(crate) struct Cs01Calculator;

impl MetricCalculator for Cs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        let as_of = context.as_of;

        if as_of >= bond.maturity {
            return Ok(0.0);
        }

        cs01_setup(bond, context.curves.as_ref())?.cs01(as_of, BumpSpec::parallel_bp)
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
/// Mirrors [`Cs01Calculator`] but applies a *triangular key-rate* spread shock
/// to the hazard curve at each standard bucket tenor instead of a single
/// parallel shock, producing a per-tenor CS01 series. The bucket weights sum to
/// one at every hazard knot, so the per-bucket CS01s sum (within the usual
/// key-rate tolerance) to the parallel CS01.
///
/// The series is stored under `bucketed_cs01::{credit_curve_id}` so downstream
/// consumers read it exactly like the generic `BucketedCs01`. When
/// `credit_curve_id` is `None`, the same z-spread fallback as the parallel
/// calculator applies and the series is stored under
/// `bucketed_cs01::{instrument_id}` (matching the z-spread CS01 keying
/// convention for bonds). When the bond has expired, CS01 is `0.0` and no
/// series is stored.
pub(crate) struct BucketedCs01Calculator;

impl MetricCalculator for BucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let as_of = context.as_of;
        // Clone the bond so no borrow of `context` outlives the reprice loop —
        // `store_bucketed_series` below needs `&mut context`.
        let bond: ConvertibleBond = context.instrument_as::<ConvertibleBond>()?.clone();

        if as_of >= bond.maturity {
            return Ok(0.0);
        }

        let series_key = bond
            .credit_curve_id
            .as_ref()
            .map_or_else(|| bond.id.as_str(), CurveId::as_str)
            .to_string();
        let setup = cs01_setup(&bond, context.curves.as_ref())?;

        let mut series: Vec<(Cow<'static, str>, f64)> = Vec::new();
        let mut total = 0.0;
        for (i, &t) in STANDARD_BUCKETS_YEARS.iter().enumerate() {
            // $ per bp of spread at this bucket.
            let cs01 = setup.cs01(as_of, |bump_bp| {
                key_rate_spec(i, bump_bp, &STANDARD_BUCKETS_YEARS)
            })?;
            series.push((format_bucket_label_cow(t), cs01));
            total += cs01;
        }

        context.store_bucketed_series(
            MetricId::composite(&MetricId::BucketedCs01, &[&series_key]),
            series,
        );
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
    };
    use crate::metrics::{MetricCalculator, MetricContext};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::prelude::FinstackConfig;
    use time::Month;

    fn make_bond_without_credit_curve() -> ConvertibleBond {
        let issue = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("valid date");

        let fixed_coupon = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            schedule: finstack_quant_cashflows::builder::ScheduleParams {
                frequency: Tenor::semi_annual(),
                day_count: DayCount::Act365F,
                business_day_convention: BusinessDayConvention::Following,
                calendar_id: "weekends_only".into(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: crate::cashflow::builder::specs::RollRule::None,
            },
        };

        ConvertibleBond {
            id: "TEST_CB_CS01".to_string().into(),
            notional: Money::from((1000_i64, Currency::USD)),
            issue_date: issue,
            maturity,
            discount_curve_id: "USD-OIS".into(),
            credit_curve_id: None,
            settlement_days: None,
            recovery_rate: None,
            conversion: ConversionSpec {
                ratio: Some(10.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            },
            spot_id: "AAPL".into(),
            vol_surface_id: "AAPL-VOL".into(),
            div_yield_id: Some("AAPL-DIVYIELD".into()),
            call_put: None,
            soft_call_trigger: None,
            fixed_coupon: Some(fixed_coupon),
            floating_coupon: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        }
    }

    fn make_context(bond: ConvertibleBond, as_of: Date) -> MetricContext {
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.90)])
            .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
            .build()
            .expect("valid curve");

        // Busted-convert setup (spot=50, conversion value=500 vs face=1000):
        // the cash component dominates, so the z-spread CS01 must be material.
        let market = finstack_quant_core::market_data::MarketContext::new()
            .insert(discount_curve)
            .insert_price("AAPL", MarketScalar::Unitless(50.0))
            .insert_price("AAPL-VOL", MarketScalar::Unitless(0.25))
            .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(0.0));

        let instrument: Arc<dyn Instrument> = Arc::new(bond);
        let base_value = instrument.value(&market, as_of).expect("base value");
        MetricContext::new(
            instrument,
            Arc::new(market),
            as_of,
            base_value,
            Arc::new(FinstackConfig::default()),
        )
    }

    /// Regression: with no credit curve, CS01 previously returned a silent 0.0.
    /// The z-spread fallback must produce a materially negative CS01 for a
    /// long busted convert (wider spreads reduce the cash-component PV).
    #[test]
    fn cs01_zspread_fallback_is_negative_and_material() {
        let as_of = Date::from_calendar_date(2025, Month::June, 2).expect("valid date");
        let mut ctx = make_context(make_bond_without_credit_curve(), as_of);
        let cs01 = super::Cs01Calculator
            .calculate(&mut ctx)
            .expect("fallback CS01 must compute");
        // ~4.6y busted convert on 1000 face: parallel 1bp on the cash leg is
        // roughly -0.4 USD/bp; assert sign and a sane magnitude band.
        assert!(
            cs01 < -0.1 && cs01 > -1.0,
            "z-spread fallback CS01 must be materially negative; got {cs01}"
        );
    }

    /// The bucketed z-spread fallback must sum (within key-rate tolerance) to
    /// the parallel z-spread CS01: the triangular bucket bumps partition a
    /// parallel bump.
    #[test]
    fn bucketed_cs01_zspread_fallback_sums_to_parallel() {
        let as_of = Date::from_calendar_date(2025, Month::June, 2).expect("valid date");
        let mut ctx = make_context(make_bond_without_credit_curve(), as_of);
        let parallel = super::Cs01Calculator
            .calculate(&mut ctx)
            .expect("parallel CS01");
        let bucketed_total = super::BucketedCs01Calculator
            .calculate(&mut ctx)
            .expect("bucketed CS01");
        assert!(
            (bucketed_total - parallel).abs() < 0.05 * parallel.abs().max(1e-6),
            "bucketed z-spread CS01 ({bucketed_total}) must sum to parallel ({parallel})"
        );
    }
}
