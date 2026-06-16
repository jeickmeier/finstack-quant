//! Loader for FX conventions embedded in JSON registries.

use crate::market::conventions::defs::FxConventions;
use crate::market::conventions::ids::FxConventionId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FxConventionRecord {
    base_currency: Currency,
    quote_currency: Currency,
    spot_lag_days: i32,
    business_day_convention: BusinessDayConvention,
    base_calendar_id: String,
    quote_calendar_id: String,
}

impl FxConventionRecord {
    fn into_conventions(self) -> Result<FxConventions, Error> {
        if self.base_currency == self.quote_currency {
            return Err(Error::Validation(
                "FX conventions must specify different base and quote currencies".to_string(),
            ));
        }
        if self.spot_lag_days < 0 || self.spot_lag_days > 7 {
            return Err(Error::Validation(format!(
                "FX spot_lag_days exceeds reasonable bound: {}",
                self.spot_lag_days
            )));
        }

        Ok(FxConventions {
            base_currency: self.base_currency,
            quote_currency: self.quote_currency,
            spot_lag_days: self.spot_lag_days,
            business_day_convention: self.business_day_convention,
            base_calendar_id: self.base_calendar_id,
            quote_calendar_id: self.quote_calendar_id,
        })
    }
}

pub(crate) fn load_registry() -> Result<HashMap<FxConventionId, FxConventions>, Error> {
    let json = include_str!("../../../../data/conventions/fx_conventions.json");
    super::json::parse_and_rekey(
        json,
        "FX",
        FxConventionId::new,
        |rec: &FxConventionRecord| rec.clone().into_conventions(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eurusd_conventions_are_available() {
        let registry = load_registry().expect("fx registry");
        let conv = registry
            .get(&FxConventionId::new("EUR/USD"))
            .expect("EUR/USD conventions");

        assert_eq!(conv.base_currency, Currency::EUR);
        assert_eq!(conv.quote_currency, Currency::USD);
        assert_eq!(conv.spot_lag_days, 2);
        assert_eq!(conv.base_calendar_id, "target2");
        assert_eq!(conv.quote_calendar_id, "usny");
    }
}
