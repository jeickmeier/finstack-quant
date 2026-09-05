//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::engine::tree::bond_tree_config;
use crate::instruments::fixed_income::bond::pricing::quote_conversions::clear_price_driving_overrides;
use crate::instruments::Bond;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use crate::pricer::ModelKey;

#[derive(Debug, Clone, Default)]
pub(crate) struct BondVegaCalculator;

fn has_embedded_options(bond: &Bond) -> bool {
    bond.return_floor.is_some() || bond.call_put.as_ref().is_some_and(|cp| cp.has_options())
}

fn resolve_oas_decimal(bond: &Bond, context: &MetricContext) -> finstack_quant_core::Result<f64> {
    if let Some(oas) = context.computed.get(&MetricId::Oas) {
        return Ok(*oas);
    }
    Ok(super::oas::oas_decimal_from_quote_overrides(bond, context)?.unwrap_or(0.0))
}

/// Whether the selected model reads two-factor short-rate volatility.
///
/// That path reads its short-rate volatility from `model_config.hw1f_sigma`
/// and ignores (in fact rejects) `market_quotes.implied_volatility`, so a vega
/// bump has to move the field the model actually consumes. Bumping the wrong
/// channel would report a silently zero vega on every credit-risky callable.
fn uses_hw_sigma(bond: &Bond, model: ModelKey) -> bool {
    model == ModelKey::RatesCredit
        || (model == ModelKey::Tree
            && !matches!(
                bond.instrument_pricing_overrides.model_config.vol_model,
                Some(crate::instruments::common_impl::parameters::VolatilityModel::Black)
            )
            && bond
                .instrument_pricing_overrides
                .model_config
                .hw1f_sigma
                .is_some())
}

/// Base short-rate volatility read by the selected model.
fn base_rate_volatility(bond: &Bond, model: ModelKey) -> finstack_quant_core::Result<f64> {
    if uses_hw_sigma(bond, model) {
        Ok(bond
            .instrument_pricing_overrides
            .model_config
            .hw1f_sigma
            .unwrap_or(0.0))
    } else {
        Ok(bond_tree_config(bond)?.volatility)
    }
}

fn holder_option_value_at_vol(
    bond: &Bond,
    context: &MetricContext,
    model: ModelKey,
    oas_decimal: f64,
    volatility: f64,
) -> finstack_quant_core::Result<f64> {
    let mut bumped = bond.clone();
    if uses_hw_sigma(bond, model) {
        bumped.instrument_pricing_overrides.model_config.hw1f_sigma = Some(volatility);
    } else {
        bumped
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(volatility);
    }
    clear_price_driving_overrides(&mut bumped);
    bumped.instrument_pricing_overrides.market_quotes.quoted_oas = Some(oas_decimal);
    let price_with_options = if context.pricing_model().is_some() {
        context.reprice_instrument_raw(&bumped, context.curves.as_ref(), context.as_of)?
    } else {
        bumped.price_for_model_raw(model, context.curves.as_ref(), context.as_of)?
    };
    // Remove only the exercise rights. Keep the bumped rate volatility and
    // every credit-model input so this is the straight-bond value under the
    // same stochastic rates-credit process. In particular, stamping the
    // legacy `implied_volatility` field here makes the rates-credit path reject
    // an otherwise valid `hw1f_sigma` configuration.
    bumped.call_put = None;
    bumped.return_floor = None;
    let price_straight = if context.pricing_model().is_some() {
        context.reprice_instrument_raw(&bumped, context.curves.as_ref(), context.as_of)?
    } else {
        bumped.price_for_model_raw(model, context.curves.as_ref(), context.as_of)?
    };
    Ok(price_with_options - price_straight)
}

impl MetricCalculator for BondVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        if !has_embedded_options(bond) {
            return Ok(0.0);
        }

        let model = context
            .pricing_model()
            .unwrap_or_else(|| bond.default_pricing_model());
        if !matches!(model, ModelKey::Tree | ModelKey::RatesCredit) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond vega requires model 'tree' or 'rates_credit'; selected '{}'",
                model.as_str()
            )));
        }

        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;
        let bump = defaults.vol_bump_pct;
        let base_vol = base_rate_volatility(bond, model)?;
        // On the rates-credit path the canonical field is itself the source
        // quote, so there is no quote-to-model conversion to undo.
        let source_vol = if uses_hw_sigma(bond, model) {
            None
        } else {
            bond.instrument_pricing_overrides
                .market_quotes
                .implied_volatility
                .filter(|source_vol| source_vol.is_finite() && *source_vol > 0.0)
        };
        let model_bump = source_vol.map_or(bump, |source_vol| bump * base_vol / source_vol);
        let down_vol = (base_vol - model_bump).max(1e-8);
        let up_vol = base_vol + model_bump;
        let oas_decimal = resolve_oas_decimal(bond, context)?;

        let up = holder_option_value_at_vol(bond, context, model, oas_decimal, up_vol)?;
        let down = holder_option_value_at_vol(bond, context, model, oas_decimal, down_vol)?;
        let effective_model_bump = up_vol - down_vol;
        if effective_model_bump.abs() < f64::EPSILON {
            return Ok(0.0);
        }

        // Bloomberg OAS screens display bond vega in price points for a 1 vol point
        // move in the source volatility quote. When a source implied vol is
        // provided alongside a converted tree vol, `model_bump` maps that quote
        // bump into the tree's volatility units.
        let source_width = source_vol
            .filter(|_| base_vol.is_finite() && base_vol.abs() > f64::EPSILON)
            .map_or(effective_model_bump, |source_vol| {
                effective_model_bump * source_vol / base_vol
            });
        if source_width.abs() < f64::EPSILON {
            return Ok(0.0);
        }

        Ok((up - down) / bond.notional.amount() * 100.0 / source_width * 0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::bond::{
        CallPut, CallPutSchedule, ProtectionWindow, ReturnFloorSpec,
    };
    use crate::instruments::InstrumentPricingOverrides;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn rates_credit_vega_preserves_model_configuration_on_straight_leg() {
        let as_of = date!(2025 - 01 - 01);
        let mut bond = Bond::fixed(
            "RATES-CREDIT-VEGA",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
            as_of,
            date!(2030 - 01 - 01),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond should build");
        bond.settlement_convention = None;
        bond.credit_curve_id = Some(CurveId::new("USD-CREDIT"));
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2028 - 01 - 01),
                end_date: date!(2028 - 01 - 01),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
            .with_quoted_oas(0.0)
            .with_hw1f_sigma(0.01)
            .with_hazard_volatility(0.005)
            .with_tree_steps(16)
            .with_mc_paths(128);

        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 0.82)])
                    .build()
                    .expect("discount curve should build"),
            )
            .insert(
                HazardCurve::builder("USD-CREDIT")
                    .base_date(as_of)
                    .recovery_rate(0.4)
                    .knots([(0.0, 0.02), (5.0, 0.02)])
                    .build()
                    .expect("hazard curve should build"),
            );
        let mut floor_only = bond.clone();
        floor_only.call_put = None;
        floor_only.return_floor = Some(ReturnFloorSpec::moic(1.0).window(
            ProtectionWindow::Between {
                start: date!(2028 - 01 - 01),
                end: date!(2028 - 01 - 02),
            },
        ));

        for (label, instrument) in [("call", bond), ("return floor", floor_only)] {
            let mut context = MetricContext::new(
                Arc::new(instrument),
                Arc::new(market.clone()),
                as_of,
                Money::from((1_000_i64, Currency::USD)),
                MetricContext::default_config(),
            );
            let vega = BondVegaCalculator
                .calculate(&mut context)
                .unwrap_or_else(|error| panic!("{label} vega failed: {error}"));
            assert!(
                vega.is_finite(),
                "rates-credit {label} vega must be finite, got {vega}"
            );
            if label == "return floor" {
                assert!(
                    vega.abs() > 1.0e-8,
                    "floor-only bond must retain optionality in vega, got {vega}"
                );
            }
        }
    }

    #[test]
    fn standalone_vega_normalizes_equivalent_clean_and_dirty_quotes() {
        let as_of = date!(2025 - 01 - 01);
        let mut clean_bond = Bond::fixed(
            "QUOTE-NORMALIZED-VEGA",
            Money::from((1_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.06).expect("valid rate fixture"),
            date!(2024 - 01 - 15),
            date!(2030 - 01 - 15),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond should build");
        clean_bond.settlement_convention = None;
        clean_bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: date!(2028 - 01 - 15),
                end_date: date!(2028 - 01 - 15),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: vec![],
        });
        clean_bond.instrument_pricing_overrides = InstrumentPricingOverrides::default()
            .with_quoted_clean_price(100.0)
            .with_implied_vol(0.01)
            .with_tree_steps(16);
        let market = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (5.0, 0.82), (10.0, 0.65)])
                .build()
                .expect("discount curve should build"),
        );
        let quote_context =
            crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext::new(
                &clean_bond,
                &market,
                as_of,
            )
            .expect("quote context");
        let dirty_amount = quote_context.dirty_from_clean_pct(100.0, clean_bond.notional.amount());
        let mut dirty_bond = clean_bond.clone();
        dirty_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price = None;
        dirty_bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_dirty_price_currency = Some(dirty_amount);

        let calculate = |bond: Bond| {
            let mut context = MetricContext::new(
                Arc::new(bond),
                Arc::new(market.clone()),
                as_of,
                Money::from((1_000_i64, Currency::USD)),
                MetricContext::default_config(),
            );
            BondVegaCalculator
                .calculate(&mut context)
                .expect("standalone vega")
        };
        let from_clean = calculate(clean_bond);
        let from_dirty = calculate(dirty_bond);
        assert!(
            (from_clean - from_dirty).abs() < 1.0e-8,
            "equivalent settlement clean and dirty quotes must resolve the same OAS: clean={from_clean}, dirty={from_dirty}"
        );
    }
}
