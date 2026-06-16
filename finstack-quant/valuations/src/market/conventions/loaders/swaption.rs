//! Loader for swaption conventions embedded in JSON registries.

use crate::market::conventions::defs::SwaptionConventions;
use crate::market::conventions::ids::SwaptionConventionId;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SwaptionConventionRecord {
    calendar_id: String,
    settlement_days: i32,
    business_day_convention: BusinessDayConvention,
    fixed_leg_frequency: String,
    fixed_leg_day_count: DayCount,
    float_leg_index: String,
}

impl SwaptionConventionRecord {
    fn into_conventions(self) -> Result<SwaptionConventions, Error> {
        let fixed_leg_freq = Tenor::parse(&self.fixed_leg_frequency).map_err(|e| {
            Error::Validation(format!(
                "Invalid `fixed_leg_frequency` in Swaption conventions: '{}': {}",
                self.fixed_leg_frequency, e
            ))
        })?;
        Ok(SwaptionConventions {
            calendar_id: self.calendar_id,
            settlement_days: self.settlement_days,
            business_day_convention: self.business_day_convention,
            fixed_leg_frequency: fixed_leg_freq,
            fixed_leg_day_count: self.fixed_leg_day_count,
            float_leg_index: self.float_leg_index,
        })
    }
}

/// Load the Swaption conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<HashMap<SwaptionConventionId, SwaptionConventions>, Error> {
    let json = include_str!("../../../../data/conventions/swaption_conventions.json");
    super::json::parse_and_rekey(
        json,
        "Swaption",
        SwaptionConventionId::new,
        |rec: &SwaptionConventionRecord| rec.clone().into_conventions(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::{DayCount, Tenor};

    #[test]
    fn usd_swaption_convention_uses_sofr_ois_fixed_leg_defaults() {
        let registry = load_registry().expect("swaption registry");
        let usd = registry
            .get(&SwaptionConventionId::new("USD"))
            .expect("USD swaption conventions");

        assert_eq!(
            usd.fixed_leg_frequency,
            Tenor::parse("1Y").expect("valid tenor")
        );
        assert_eq!(usd.fixed_leg_day_count, DayCount::Act360);
        assert_eq!(usd.float_leg_index, "USD-SOFR-OIS");
    }
}
