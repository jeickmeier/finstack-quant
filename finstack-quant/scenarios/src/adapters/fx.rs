//! Foreign exchange shock adapter.

use crate::engine::ExecutionContext;
use crate::engine::ScenarioEffect;
use crate::error::Result;
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::MarketBump;

fn post_shock_triangulation_warnings(
    market: &finstack_quant_core::market_data::context::MarketContext,
    base: finstack_quant_core::currency::Currency,
    quote: finstack_quant_core::currency::Currency,
    pct: f64,
    as_of: finstack_quant_core::dates::Date,
) -> Result<Vec<Warning>> {
    let Some(fx) = market.fx() else {
        return Ok(Vec::new());
    };
    let state = fx.get_serializable_state()?;
    let Some(pivot) = state.config.pivot_currency else {
        return Ok(Vec::new());
    };
    let bumped = fx.with_bumped_rate(base, quote, pct / 100.0, as_of)?;
    let mut warnings = Vec::new();

    for &(from, to, _) in &state.quotes {
        if from == to || from == pivot || to == pivot {
            continue;
        }
        if from != base && from != quote && to != base && to != quote {
            continue;
        }

        let direct = bumped
            .rate(finstack_quant_core::money::fx::FxQuery::new(
                from,
                to,
                as_of,
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ))
            .map(|r| r.rate);
        let via_pivot = bumped
            .rate(finstack_quant_core::money::fx::FxQuery::new(
                from,
                pivot,
                as_of,
                finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            ))
            .and_then(|lhs| {
                bumped
                    .rate(finstack_quant_core::money::fx::FxQuery::new(
                        pivot,
                        to,
                        as_of,
                        finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
                    ))
                    .map(|rhs| lhs.rate * rhs.rate)
            });

        let (Ok(direct), Ok(implied)) = (direct, via_pivot) else {
            continue;
        };

        let tolerance = 1.0e-8 * direct.abs().max(implied.abs()).max(1.0);
        if (direct - implied).abs() > tolerance {
            warnings.push(Warning::FxTriangulationInconsistent {
                detail: format!(
                    "FX shock on {base}/{quote} leaves direct quote {from}/{to} inconsistent with {pivot}-triangulated cross (direct={direct:.10}, implied={implied:.10})"
                ),
            });
        }
    }

    Ok(warnings)
}

/// Generate effects for an FX percent shock.
pub(crate) fn fx_pct_effects(
    base: finstack_quant_core::currency::Currency,
    quote: finstack_quant_core::currency::Currency,
    pct: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let bump = MarketBump::FxPct {
        base,
        quote,
        pct,
        as_of: ctx.as_of,
    };
    let mut effects = vec![ScenarioEffect::MarketBump(bump)];
    effects.extend(
        post_shock_triangulation_warnings(ctx.market, base, quote, pct, ctx.as_of)?
            .into_iter()
            .map(ScenarioEffect::Warning),
    );
    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::fx::{FxConfig, FxMatrix, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn post_shock_triangulation_check_warns_on_inconsistent_cross() {
        let as_of = date!(2025 - 01 - 01);
        let fx = FxMatrix::try_with_config(
            Arc::new(SimpleFxProvider::new()),
            FxConfig {
                pivot_currency: Some(Currency::USD),
                cache_capacity: 32,
            },
        )
        .expect("fx config should be valid");
        fx.set_quotes(&[
            (Currency::EUR, Currency::USD, 1.20),
            (Currency::USD, Currency::JPY, 150.0),
            (Currency::EUR, Currency::JPY, 180.0),
        ])
        .expect("quotes should seed");
        let market = MarketContext::new().insert_fx(fx);

        let warnings =
            post_shock_triangulation_warnings(&market, Currency::EUR, Currency::USD, 10.0, as_of)
                .expect("warning generation should succeed");

        assert!(
            warnings.iter().any(|warning| {
                let s = warning.to_string();
                s.contains("EUR/JPY") && s.contains("implied=198.0000000000")
            }),
            "expected direct cross inconsistency warning, got {warnings:?}"
        );
    }

    #[test]
    fn post_shock_triangulation_check_accepts_valid_negative_percent_shock() {
        let as_of = date!(2025 - 01 - 01);
        let fx = FxMatrix::try_with_config(
            Arc::new(SimpleFxProvider::new()),
            FxConfig {
                pivot_currency: Some(Currency::USD),
                cache_capacity: 32,
            },
        )
        .expect("fx config should be valid");
        fx.set_quotes(&[
            (Currency::EUR, Currency::USD, 1.20),
            (Currency::USD, Currency::JPY, 150.0),
            (Currency::EUR, Currency::JPY, 180.0),
        ])
        .expect("quotes should seed");
        let market = MarketContext::new().insert_fx(fx);

        let warnings =
            post_shock_triangulation_warnings(&market, Currency::EUR, Currency::USD, -5.0, as_of)
                .expect("valid -5% shock should not fail preview");

        assert!(
            warnings.iter().any(|warning| {
                let s = warning.to_string();
                s.contains("EUR/JPY") && s.contains("implied=171.0000000000")
            }),
            "expected post-shock state from a -5% bump, got {warnings:?}"
        );
    }
}
