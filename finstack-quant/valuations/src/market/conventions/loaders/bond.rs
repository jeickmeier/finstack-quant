//! Loader for bond conventions embedded in JSON registries.

use crate::market::conventions::defs::BondConventions;
use crate::market::conventions::ids::BondConventionId;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

pub(crate) fn load_registry() -> Result<HashMap<BondConventionId, BondConventions>, Error> {
    let json = include_str!("../../../../data/conventions/bond_conventions.json");
    super::json::parse_and_rekey(
        json,
        "bond",
        BondConventionId::new,
        |rec: &BondConventions| Ok(rec.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::BondConvention;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn usd_ust_bond_conventions_are_available() {
        let registry = load_registry().expect("bond registry");
        let conv = registry
            .get(&BondConventionId::new("USD-UST"))
            .expect("USD-UST conventions");

        assert_eq!(conv.currency, Currency::USD);
        assert_eq!(conv.market_convention, BondConvention::USTreasury);
        assert_eq!(conv.default_discount_curve_id, "USD-TREASURY");
    }

    #[test]
    fn all_standard_bond_conventions_are_available() {
        let registry = load_registry().expect("bond registry");

        let agency = registry
            .get(&BondConventionId::new("USD-AGENCY"))
            .expect("USD-AGENCY");
        assert_eq!(agency.currency, Currency::USD);
        assert_eq!(agency.market_convention, BondConvention::USAgency);

        let bund = registry
            .get(&BondConventionId::new("EUR-BUND"))
            .expect("EUR-BUND");
        assert_eq!(bund.currency, Currency::EUR);
        assert_eq!(bund.market_convention, BondConvention::GermanBund);

        let gilt = registry
            .get(&BondConventionId::new("GBP-GILT"))
            .expect("GBP-GILT");
        assert_eq!(gilt.currency, Currency::GBP);
        assert_eq!(gilt.market_convention, BondConvention::UKGilt);

        let oat = registry
            .get(&BondConventionId::new("EUR-OAT"))
            .expect("EUR-OAT");
        assert_eq!(oat.currency, Currency::EUR);
        assert_eq!(oat.market_convention, BondConvention::FrenchOAT);

        let jgb = registry
            .get(&BondConventionId::new("JPY-JGB"))
            .expect("JPY-JGB");
        assert_eq!(jgb.currency, Currency::JPY);
        assert_eq!(jgb.market_convention, BondConvention::JGB);
    }
}
