//! Bond-specific CS01 calculators with z-spread fallback.
//!
//! When a bond has an associated credit (hazard) curve, CS01 follows the
//! [canonical CS01 convention][canonical] — a parallel 1 bp shock to par CDS
//! spreads with a symmetric (central) finite difference — by delegating to
//! `GenericParallelCs01` / `GenericBucketedCs01`.
//!
//! When the active model does not consume a credit curve, no par CDS curve is
//! available to bump. CS01 therefore bumps the spread used by that same model:
//! OAS for option-bearing bonds and the market-standard z-spread for vanilla
//! bonds. Every trial reprices through the request's selected model and pricer
//! registry.
//!
//! ```text
//! CS01_fallback = PV(z + 1bp) - PV(z)        (forward difference)
//! ```
//!
//! where `PV(z)` re-discounts each cashflow with the spread `z` added to the
//! periodically-compounded zero rate implied by the base discount factor (see
//! `z_spread_discount_factor`) — i.e. the same compounding-aware shift used
//! by the Z-spread solver, not a continuous `exp(-z·t)` shift. This is a deliberate
//! deviation from the canonical methodology in two respects, called out here
//! so consumers can audit the regime:
//!
//! 1. The shock is applied to the bond's z-spread rather than to par CDS
//!    spreads (no hazard curve exists to re-bootstrap).
//! 2. A forward difference is used in place of the canonical central
//!    difference `(PV(z+1bp) − PV(z−1bp)) / 2`. The two agree to
//!    `O(bump²) ≈ 10⁻⁸` of CS01 magnitude for a 1 bp shock; the forward
//!    form is preserved for deterministic golden parity.
//!
//! # Settlement-anchored repricing
//!
//! For vanilla bonds, the bumped and base PVs retain the settlement
//! (`quote_date`) and compounding conventions used by [`price_from_z_spread`]
//! and [`ZSpreadCalculator`]. For option-bearing bonds, both values retain the
//! model's exercise policy and differ only by a 1 bp OAS bump.
//!
//! Sign convention is identical to the canonical reference:
//! - Long bond → CS01 negative (wider spreads reduce PV).
//! - Short bond → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01
//! [`price_from_z_spread`]: crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_z_spread
//! [`ZSpreadCalculator`]: crate::instruments::fixed_income::bond::ZSpreadCalculator

use super::risk_view::{active_model_consumes_credit, with_bond_risk_view};
use crate::constants::ONE_BASIS_POINT;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::metrics::price_yield_spread::z_spread::BondZSpreadPricingKernel;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use std::borrow::Cow;

pub(super) fn z_spread_bumped_pvs(
    context: &MetricContext,
) -> finstack_quant_core::Result<(f64, f64)> {
    let base_spread = base_z_spread(context)?;
    let bond: &Bond = context.instrument_as()?;
    let pricing_kernel =
        BondZSpreadPricingKernel::new(bond, context.curves.as_ref(), context.as_of)?;
    let base_npv = pricing_kernel.price(base_spread)?;
    let bumped_npv = pricing_kernel.price(base_spread + ONE_BASIS_POINT)?;
    validate_spread_pvs(base_npv, bumped_npv)?;
    Ok((base_npv, bumped_npv))
}

fn model_z_spread_bumped_pvs(context: &MetricContext) -> finstack_quant_core::Result<(f64, f64)> {
    let base_spread = base_z_spread(context)?;
    let bond: &Bond = context.instrument_as()?;
    let quote_date = crate::instruments::fixed_income::bond::pricing::settlement::settlement_date(
        bond,
        context.as_of,
    )?;
    let mut base_bond = bond.clone();
    crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides(
        &mut base_bond,
    );
    base_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread = Some(base_spread);
    let base_at_as_of =
        context.reprice_instrument_raw(&base_bond, context.curves.as_ref(), context.as_of)?;

    let mut bumped_bond = base_bond.clone();
    bumped_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_z_spread = Some(base_spread + ONE_BASIS_POINT);
    let bumped_at_as_of =
        context.reprice_instrument_raw(&bumped_bond, context.curves.as_ref(), context.as_of)?;

    let base_npv =
        crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
            bond,
            context.curves.as_ref(),
            context.as_of,
            quote_date,
            base_at_as_of,
        )?;
    let bumped_npv =
        crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
            bond,
            context.curves.as_ref(),
            context.as_of,
            quote_date,
            bumped_at_as_of,
        )?;
    validate_spread_pvs(base_npv, bumped_npv)?;
    Ok((base_npv, bumped_npv))
}

fn base_z_spread(context: &MetricContext) -> finstack_quant_core::Result<f64> {
    let base_spread = context
        .computed
        .get(&MetricId::ZSpread)
        .copied()
        .ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "metric:ZSpread".to_string(),
            })
        })?;
    if !base_spread.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bond spread risk requires a finite quote-reproducing Z-spread; got {base_spread}"
        )));
    }
    Ok(base_spread)
}

fn validate_spread_pvs(base_npv: f64, bumped_npv: f64) -> finstack_quant_core::Result<()> {
    if !base_npv.is_finite() || !bumped_npv.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bond spread risk requires finite quote-reproducing PVs; \
             base PV={base_npv}, bumped PV={bumped_npv}"
        )));
    }
    Ok(())
}

fn oas_bumped_pvs(context: &MetricContext) -> finstack_quant_core::Result<(f64, f64)> {
    let bond: &Bond = context.instrument_as()?;
    let has_price_driver = bond
        .instrument_pricing_overrides
        .market_quotes
        .has_price_driver();
    let base_oas = match context.computed.get(&MetricId::Oas).copied() {
        Some(oas) => oas,
        None if !has_price_driver => 0.0,
        None => return Err(crate::metrics::metric_not_found(MetricId::Oas)),
    };
    if !base_oas.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bond option spread risk requires a finite OAS; got {base_oas}"
        )));
    }
    let mut base_bond = bond.clone();
    crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides(
        &mut base_bond,
    );
    base_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_oas = Some(base_oas);
    let base_npv =
        context.reprice_instrument_raw(&base_bond, context.curves.as_ref(), context.as_of)?;

    let mut bumped_bond = base_bond.clone();
    bumped_bond
        .instrument_pricing_overrides
        .market_quotes
        .quoted_oas = Some(base_oas + ONE_BASIS_POINT);
    let bumped_npv =
        context.reprice_instrument_raw(&bumped_bond, context.curves.as_ref(), context.as_of)?;

    if !base_npv.is_finite() || !bumped_npv.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "bond option spread risk requires finite model PVs; base PV={base_npv}, bumped PV={bumped_npv}"
        )));
    }

    Ok((base_npv, bumped_npv))
}

/// Bond parallel CS01 with z-spread fallback.
///
/// Delegates to `GenericParallelCs01` when the active model consumes a credit
/// curve; otherwise computes CS01 by bumping the selected model's OAS for an
/// option-bearing bond or z-spread for a vanilla bond by 1 bp.
/// The result is keyed by credit curve ID or instrument ID.
pub(crate) struct BondCs01Calculator;

impl MetricCalculator for BondCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let uses_credit = {
            let bond: &Bond = context.instrument_as()?;
            active_model_consumes_credit(context, bond)
        };

        if uses_credit {
            return with_bond_risk_view(context, |ctx| {
                crate::metrics::GenericParallelCs01::<Bond>::default().calculate(ctx)
            });
        }

        let (inst_id, has_options) = {
            let bond = context.instrument_as::<Bond>()?;
            (bond.id().to_string(), bond.has_exercise_rights())
        };
        let (base_npv, bumped_npv) = if has_options {
            oas_bumped_pvs(context)?
        } else {
            model_z_spread_bumped_pvs(context)?
        };
        let cs01 = bumped_npv - base_npv;
        if !cs01.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond spread CS01 must be finite; got {cs01}"
            )));
        }

        context
            .computed
            .insert(MetricId::custom(format!("cs01::{}", inst_id)), cs01);

        Ok(cs01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::ZSpread]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Ok(bond) = context.instrument_as::<Bond>() else {
            return Cow::Borrowed(self.dependencies());
        };
        if active_model_consumes_credit(context, bond) {
            Cow::Borrowed(&[])
        } else if bond.has_exercise_rights() {
            if bond
                .instrument_pricing_overrides
                .market_quotes
                .has_price_driver()
            {
                Cow::Borrowed(&[MetricId::Oas])
            } else {
                Cow::Borrowed(&[])
            }
        } else {
            Cow::Borrowed(self.dependencies())
        }
    }
}

/// Bond bucketed CS01 with z-spread fallback.
///
/// Delegates to `GenericBucketedCs01` when the active model consumes a credit
/// curve; otherwise returns the parallel OAS or z-spread CS01 keyed by
/// instrument ID.
pub(crate) struct BondBucketedCs01Calculator;

impl MetricCalculator for BondBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let uses_credit = {
            let bond: &Bond = context.instrument_as()?;
            active_model_consumes_credit(context, bond)
        };

        if uses_credit {
            return with_bond_risk_view(context, |ctx| {
                crate::metrics::GenericBucketedCs01::<Bond>::default().calculate(ctx)
            });
        }

        let cs01 = context
            .computed
            .get(&MetricId::Cs01)
            .copied()
            .ok_or_else(|| {
                finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                    id: "metric:Cs01".to_string(),
                })
            })?;

        Ok(cs01)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Cs01]
    }

    fn dynamic_dependencies<'a>(&'a self, context: &MetricContext) -> Cow<'a, [MetricId]> {
        let Ok(bond) = context.instrument_as::<Bond>() else {
            return Cow::Borrowed(self.dependencies());
        };
        if active_model_consumes_credit(context, bond) {
            Cow::Borrowed(&[])
        } else {
            Cow::Borrowed(self.dependencies())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
    use crate::instruments::PricingOptions;
    use crate::metrics::MetricRegistry;
    use crate::pricer::{
        expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
    };
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use time::macros::date;

    struct OasSensitiveTreePricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for OasSensitiveTreePricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Tree)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            let oas = bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_oas
                .unwrap_or(0.0);
            Ok(ValuationResult::stamped(
                bond.id(),
                as_of,
                Money::new(
                    1_000.0 - 10_000.0 * oas - 1_000_000.0 * oas * oas,
                    Currency::USD,
                ),
            ))
        }
    }

    struct ZSpreadSensitiveDiscountingPricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for ZSpreadSensitiveDiscountingPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::Discounting)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
            self.calls.fetch_add(1, Ordering::SeqCst);
            let z_spread = bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_z_spread
                .unwrap_or(0.0);
            Ok(ValuationResult::stamped(
                bond.id(),
                as_of,
                Money::new(
                    1_000.0 - 10_000.0 * z_spread - 1_000_000.0 * z_spread * z_spread,
                    Currency::USD,
                ),
            ))
        }
    }

    struct FixedSpreadCalculator {
        calls: Arc<AtomicUsize>,
    }

    impl MetricCalculator for FixedSpreadCalculator {
        fn calculate(&self, _context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(0.002)
        }
    }

    #[test]
    fn option_spread_cs01_reprices_through_selected_custom_registry() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "CS01-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            as_of,
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.settlement_convention = None;
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2028 - 01 - 15),
                end_date: date!(2028 - 01 - 15),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(98.0);

        let pricer_calls = Arc::new(AtomicUsize::new(0));
        let mut pricers = PricerRegistry::new();
        pricers
            .register(OasSensitiveTreePricer {
                calls: Arc::clone(&pricer_calls),
            })
            .expect("unique custom pricer");
        let oas_calls = Arc::new(AtomicUsize::new(0));
        let mut metrics = MetricRegistry::new();
        metrics
            .register_metric(
                MetricId::Oas,
                Arc::new(FixedSpreadCalculator {
                    calls: Arc::clone(&oas_calls),
                }),
                &[InstrumentType::Bond],
            )
            .expect("unique custom OAS metric");
        metrics
            .register_metric(
                MetricId::Cs01,
                Arc::new(BondCs01Calculator),
                &[InstrumentType::Bond],
            )
            .expect("unique custom CS01 metric");

        let result = bond
            .price_with_metrics(
                &MarketContext::new(),
                as_of,
                &[MetricId::Cs01],
                PricingOptions::default()
                    .with_model(ModelKey::Tree)
                    .with_registry(Arc::new(pricers))
                    .with_metric_registry(Arc::new(metrics)),
            )
            .expect("selected-model option spread CS01");
        assert!((result.measures["cs01"] + 1.41).abs() < 1.0e-12);
        assert_eq!(oas_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            pricer_calls.load(Ordering::SeqCst),
            3,
            "base PV plus OAS base and bumped OAS must use the selected pricer"
        );
    }

    #[test]
    fn vanilla_spread_cs01_reprices_through_selected_custom_registry() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "CS01-Z-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            as_of,
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.settlement_convention = None;

        let pricer_calls = Arc::new(AtomicUsize::new(0));
        let mut pricers = PricerRegistry::new();
        pricers
            .register(ZSpreadSensitiveDiscountingPricer {
                calls: Arc::clone(&pricer_calls),
            })
            .expect("unique custom pricer");
        let z_spread_calls = Arc::new(AtomicUsize::new(0));
        let mut metrics = MetricRegistry::new();
        metrics
            .register_metric(
                MetricId::ZSpread,
                Arc::new(FixedSpreadCalculator {
                    calls: Arc::clone(&z_spread_calls),
                }),
                &[InstrumentType::Bond],
            )
            .expect("unique custom Z-spread metric");
        metrics
            .register_metric(
                MetricId::Cs01,
                Arc::new(BondCs01Calculator),
                &[InstrumentType::Bond],
            )
            .expect("unique custom CS01 metric");

        let result = bond
            .price_with_metrics(
                &MarketContext::new(),
                as_of,
                &[MetricId::Cs01],
                PricingOptions::default()
                    .with_model(ModelKey::Discounting)
                    .with_registry(Arc::new(pricers))
                    .with_metric_registry(Arc::new(metrics)),
            )
            .expect("selected-model vanilla spread CS01");
        assert!((result.measures["cs01"] + 1.41).abs() < 1.0e-12);
        assert_eq!(z_spread_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            pricer_calls.load(Ordering::SeqCst),
            3,
            "base PV plus Z-spread base and bump must use the selected pricer"
        );
    }
}
