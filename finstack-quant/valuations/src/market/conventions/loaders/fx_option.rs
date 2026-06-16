//! Loader for FX option conventions embedded in JSON registries.

use crate::market::conventions::defs::FxOptionConventions;
use crate::market::conventions::ids::FxOptionConventionId;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

pub(crate) fn load_registry() -> Result<HashMap<FxOptionConventionId, FxOptionConventions>, Error> {
    let json = include_str!("../../../../data/conventions/fx_option_conventions.json");
    super::json::parse_and_rekey(
        json,
        "FX option",
        FxOptionConventionId::new,
        |rec: &FxOptionConventions| Ok(rec.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{ExerciseStyle, SettlementType};
    use crate::market::conventions::ids::FxConventionId;
    use finstack_quant_core::dates::DayCount;

    #[test]
    fn eurusd_vanilla_fx_option_conventions_are_available() {
        let registry = load_registry().expect("fx option registry");
        let conv = registry
            .get(&FxOptionConventionId::new("EUR/USD-VANILLA"))
            .expect("EUR/USD-VANILLA conventions");

        assert_eq!(conv.fx_convention_id, FxConventionId::new("EUR/USD"));
        assert_eq!(conv.exercise_style, ExerciseStyle::European);
        assert_eq!(conv.settlement, SettlementType::Cash);
        assert_eq!(conv.day_count, DayCount::Act365F);
    }
}
