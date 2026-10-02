//! Portfolio adapter from instrument dependencies to factor-model dependencies.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::factor::{CurveType, MarketDependency};
use finstack_quant_valuations::instruments::MarketDependencies;

pub(super) fn flatten(
    deps: &MarketDependencies,
    market: &MarketContext,
) -> finstack_quant_core::Result<Vec<MarketDependency>> {
    let mut result = Vec::new();

    for id in &deps.curves.discount_curves {
        result.push(MarketDependency::Curve {
            id: id.clone(),
            curve_type: CurveType::Discount,
        });
    }
    for id in &deps.curves.forward_curves {
        result.push(MarketDependency::Curve {
            id: id.clone(),
            curve_type: CurveType::Forward,
        });
    }
    for id in &deps.curves.credit_curves {
        result.push(MarketDependency::CreditCurve { id: id.clone() });
    }
    for id in &deps.credit_index_ids {
        for key in crate::dependencies::credit_index_keys(market, id)? {
            let dependency = match key {
                crate::MarketFactorKey::Curve { id, .. } => MarketDependency::CreditCurve { id },
                crate::MarketFactorKey::BaseCorrelation(id) => MarketDependency::Curve {
                    id,
                    curve_type: CurveType::BaseCorrelation,
                },
                _ => continue,
            };
            if !result.contains(&dependency) {
                result.push(dependency);
            }
        }
    }
    for id in &deps.curves.inflation_curves {
        result.push(MarketDependency::Curve {
            id: id.clone(),
            curve_type: CurveType::Inflation,
        });
    }
    for id in &deps.market_scalar_ids {
        result.push(MarketDependency::Spot { id: id.clone() });
    }
    for id in deps.unique_vol_surface_ids() {
        result.push(MarketDependency::VolSurface {
            id: id.as_str().to_string(),
        });
    }
    for pair in &deps.fx_pairs {
        result.push(MarketDependency::FxPair {
            base: pair.base,
            quote: pair.quote,
        });
    }
    for id in &deps.series_ids {
        result.push(MarketDependency::Series { id: id.clone() });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credit_index_fails_assignment_resolution() {
        let mut deps = MarketDependencies::new();
        deps.add_credit_index("CDX.NA.IG.42");

        assert!(flatten(&deps, &MarketContext::new()).is_err());
    }
}
