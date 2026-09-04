//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::math::solver::{BrentSolver, Solver};
use std::cell::RefCell;

/// Resolve the bond's OAS from any supported price-driving quote.
///
/// Returns `None` when the bond has no price-driving quote. A quoted OAS is
/// returned directly; all other quote forms first normalize to settlement
/// dirty price through the shared quote-conversion path and then solve on the
/// bond's effective option model.
pub(crate) fn oas_decimal_from_quote_overrides(
    bond: &Bond,
    context: &MetricContext,
) -> finstack_quant_core::Result<Option<f64>> {
    bond.instrument_pricing_overrides.market_quotes.validate()?;
    if let Some(oas) = bond.instrument_pricing_overrides.market_quotes.quoted_oas {
        return Ok(Some(oas));
    }

    let model = context
        .pricing_model()
        .unwrap_or_else(|| bond.default_pricing_model());
    let pricing_dispatch = context.clone_pricer_dispatch();
    let Some(dirty_at_quote) = crate::instruments::fixed_income::bond::pricing::quote_conversions::settlement_dirty_from_quote_overrides(
        bond,
        context.curves.as_ref(),
        context.as_of,
        model,
        Some(&pricing_dispatch),
    )? else {
        return Ok(None);
    };
    let config =
        crate::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config(bond)?;
    let quote_context =
        crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
            bond,
            context.curves.as_ref(),
            context.as_of,
        )?;
    let dirty_target_at_quote = match config.oas_price_basis {
        crate::instruments::pricing_overrides::OasPriceBasis::SettlementDirty => dirty_at_quote,
        crate::instruments::pricing_overrides::OasPriceBasis::ForwardAccruedClean => {
            let schedule = bond.full_cashflow_schedule(context.curves.as_ref())?;
            let accrued_at_as_of = crate::cashflow::accrual::accrued_interest_amount(
                &schedule,
                context.as_of,
                &bond.accrual_config(),
            )?;
            dirty_at_quote - accrued_at_as_of
        }
    };
    let mut trial_template = bond.clone();
    crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides(
        &mut trial_template,
    );
    // A precision target belongs to the final estimate. Enforcing it on every
    // noisy root trial can reject a valid OAS before the solver reaches it.
    let final_ci_target = trial_template
        .instrument_pricing_overrides
        .model_config
        .mc_target_ci_half_width
        .take();
    let pricing_error: RefCell<Option<finstack_quant_core::Error>> = RefCell::new(None);
    let objective = |oas_bp: f64| -> f64 {
        let mut trial = trial_template.clone();
        trial.instrument_pricing_overrides.market_quotes.quoted_oas = Some(oas_bp / 10_000.0);
        let priced = if context.pricing_model().is_some() {
            context.reprice_instrument_raw(
                &trial,
                context.curves.as_ref(),
                quote_context.quote_date,
            )
        } else {
            trial.price_for_model_raw(model, context.curves.as_ref(), quote_context.quote_date)
        };
        match priced {
            Ok(value) => value - dirty_target_at_quote,
            Err(error) => {
                let mut slot = pricing_error.borrow_mut();
                if slot.is_none() {
                    *slot = Some(error);
                }
                1.0e12
            }
        }
    };
    let oas_bp = BrentSolver::new()
        .tolerance(config.tolerance)
        .max_iterations(config.max_iterations)
        .initial_bracket_size(config.initial_bracket_size_bp)
        .solve(objective, 0.0)
        .map_err(|error| match pricing_error.borrow_mut().take() {
            Some(pricing_error) => finstack_quant_core::Error::Validation(format!(
                "OAS solve failed: {error}; first underlying pricing error: {pricing_error}"
            )),
            None => error,
        })?;
    let oas_decimal = oas_bp / 10_000.0;

    if let Some(target) = final_ci_target {
        let mut final_trial = trial_template;
        final_trial
            .instrument_pricing_overrides
            .model_config
            .mc_target_ci_half_width = Some(target);
        final_trial
            .instrument_pricing_overrides
            .market_quotes
            .quoted_oas = Some(oas_decimal);
        if context.pricing_model().is_some() {
            context.reprice_instrument_raw(
                &final_trial,
                context.curves.as_ref(),
                quote_context.quote_date,
            )?;
        } else {
            final_trial.price_for_model_raw(
                model,
                context.curves.as_ref(),
                quote_context.quote_date,
            )?;
        }
    }

    Ok(Some(oas_decimal))
}

/// Calculates Option-Adjusted Spread for bonds with embedded options.
///
/// Uses the model selected by the metric context: scalar discounting or
/// hazard-rate pricing for non-callable bonds, a short-rate tree for `tree`,
/// and the joint rates-credit model for `rates_credit`. The option models value
/// call, put, and return-floor rights and solve for the spread (in
/// **decimal units**, e.g. `0.01 = 100bp`) that makes the model price equal to
/// the market price.
///
/// OAS accounts for embedded exercise rights through model-consistent rollback.
///
/// # Dependencies
///
/// Uses `quoted_oas` directly when supplied. Otherwise, any supported
/// price-driving bond quote is normalized to settlement clean price through
/// the shared quote-conversion path before solving OAS.
pub(crate) struct OasCalculator;

impl MetricCalculator for OasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        oas_decimal_from_quote_overrides(bond, context)?.ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "bond.instrument_pricing_overrides.market_quotes price driver".to_string(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::price_from_oas;
    use crate::instruments::fixed_income::bond::{ProtectionWindow, ReturnFloorSpec};
    use crate::instruments::InstrumentPricingOverrides;
    use crate::pricer::{
        expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingDispatch,
        PricingError,
    };
    use crate::results::ValuationResult;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use time::macros::date;

    struct LinearOasPricer {
        calls: Arc<AtomicUsize>,
    }

    impl LinearOasPricer {
        fn price(&self, instrument: &dyn Instrument) -> f64 {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)
                .expect("OAS test pricer expects a bond");
            let oas = bond
                .instrument_pricing_overrides
                .market_quotes
                .quoted_oas
                .expect("every OAS solver trial must carry quoted_oas");
            1_000.0 - 10_000.0 * oas
        }
    }

    impl Pricer for LinearOasPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::new(self.price(instrument), Currency::USD),
            ))
        }

        fn price_raw_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> std::result::Result<f64, PricingError> {
            Ok(self.price(instrument))
        }
    }

    #[test]
    fn deterministic_floor_only_clean_quote_oas_reproduces_price() {
        let as_of = date!(2025 - 03 - 20);
        let mut bond = Bond::fixed(
            "FLOOR-OAS-ROUNDTRIP",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.06),
            date!(2024 - 01 - 15),
            date!(2030 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond should build");
        bond.settlement_convention = None;
        bond.return_floor = Some(
            ReturnFloorSpec::moic(1.0).window(ProtectionWindow::Between {
                start: date!(2027 - 01 - 15),
                end: date!(2027 - 01 - 16),
            }),
        );
        bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
            .with_implied_vol(0.01)
            .with_tree_steps(16);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.82), (10.0, 0.65)])
                .build()
                .expect("discount curve should build"),
        );
        let expected_oas = 0.0025;
        let target_dirty = price_from_oas(&bond, &market, as_of, ModelKey::Tree, expected_oas)
            .expect("target option price");
        let quote_context =
            crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
                &bond, &market, as_of,
            )
            .expect("quote context");
        let target_clean_pct =
            (target_dirty - quote_context.accrued_at_quote_date) / bond.notional.amount() * 100.0;
        bond.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = Some(target_clean_pct);

        let context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market.clone()),
            as_of,
            Money::new(target_dirty, Currency::USD),
            MetricContext::default_config(),
        );
        let solved = oas_decimal_from_quote_overrides(&bond, &context)
            .expect("OAS solve")
            .expect("price-driving quote");
        let reproduced =
            price_from_oas(&bond, &market, as_of, ModelKey::Tree, solved).expect("repriced floor");
        assert!(
            (solved - expected_oas).abs() < 1.0e-7,
            "floor-only OAS must recover the generating spread: solved={solved}, expected={expected_oas}"
        );
        assert!(
            (reproduced - target_dirty).abs() < 1.0e-7,
            "floor-only OAS must reproduce quoted dirty value: reproduced={reproduced}, target={target_dirty}"
        );
    }

    #[test]
    fn oas_trials_preserve_selected_custom_registry_model() {
        let as_of = date!(2025 - 01 - 15);
        let mut bond = Bond::fixed(
            "OAS-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            as_of,
            date!(2030 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        bond.settlement_convention = None;
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(98.0);
        assert_eq!(bond.default_pricing_model(), ModelKey::Discounting);

        let market = MarketContext::new()
            .insert(DiscountCurve::flat("USD-OIS", as_of, 0.04).expect("valid discount curve"));
        let calls = Arc::new(AtomicUsize::new(0));
        let mut registry = PricerRegistry::new();
        registry
            .register(LinearOasPricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let mut context = MetricContext::new(
            Arc::new(bond.clone()),
            Arc::new(market),
            as_of,
            Money::new(980.0, Currency::USD),
            MetricContext::default_config(),
        );
        context.set_pricer_dispatch(PricingDispatch::registered(
            ModelKey::HazardRate,
            Arc::new(registry),
        ));

        let solved = oas_decimal_from_quote_overrides(&bond, &context)
            .expect("custom-model OAS solve")
            .expect("price-driving quote");
        assert!((solved - 0.002).abs() < 1.0e-10, "solved={solved}");
        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "every objective evaluation must use the selected registry pricer"
        );
    }
}
