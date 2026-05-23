use crate::instruments::common_impl::dependencies::MarketDependencies;
use finstack_core::factor_model::{CurveType, MarketDependency};

/// Flattens aggregated market dependencies into individually matchable entries.
#[must_use]
pub fn decompose(deps: &MarketDependencies) -> Vec<MarketDependency> {
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

    for id in &deps.spot_ids {
        result.push(MarketDependency::Spot { id: id.clone() });
    }

    for id in &deps.vol_surface_ids {
        result.push(MarketDependency::VolSurface { id: id.clone() });
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

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::dependencies::MarketDependencies;
    use finstack_core::currency::Currency;
    use finstack_core::types::CurveId;

    #[test]
    fn test_decompose_empty_dependencies() {
        let deps = MarketDependencies::new();
        let result = decompose(&deps);
        assert!(result.is_empty());
    }

    #[test]
    fn test_decompose_discount_curves() {
        let mut deps = MarketDependencies::new();
        deps.curves.discount_curves.push(CurveId::new("USD-OIS"));

        let result = decompose(&deps);
        assert_eq!(result.len(), 1);
        match &result[0] {
            MarketDependency::Curve { id, curve_type } => {
                assert_eq!(id.as_ref(), "USD-OIS");
                assert_eq!(*curve_type, CurveType::Discount);
            }
            _ => unreachable!("expected curve variant"),
        }
    }

    #[test]
    fn test_decompose_credit_curves() {
        let mut deps = MarketDependencies::new();
        deps.curves.credit_curves.push(CurveId::new("ACME-HAZARD"));

        let result = decompose(&deps);
        assert_eq!(result.len(), 1);
        match &result[0] {
            MarketDependency::CreditCurve { id } => {
                assert_eq!(id.as_ref(), "ACME-HAZARD");
            }
            _ => unreachable!("expected credit curve variant"),
        }
    }

    #[test]
    fn test_decompose_mixed() {
        let mut deps = MarketDependencies::new();
        deps.curves.discount_curves.push(CurveId::new("USD-OIS"));
        deps.curves.credit_curves.push(CurveId::new("ACME-HAZARD"));
        deps.spot_ids.push("AAPL".into());
        deps.vol_surface_ids.push("AAPL-VOL".into());
        deps.fx_pairs
            .push(crate::instruments::common_impl::dependencies::FxPair::new(
                Currency::USD,
                Currency::EUR,
            ));

        let result = decompose(&deps);
        assert_eq!(result.len(), 5);
    }
}
