//! Loader for cross-currency swap conventions embedded in JSON registries.

use crate::market::conventions::defs::XccyConventions;
use crate::market::conventions::ids::XccyConventionId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_quant_core::types::IndexId;
use finstack_quant_core::Error;
use finstack_quant_core::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct XccyConventionRecord {
    base_currency: Currency,
    quote_currency: Currency,
    base_index_id: String,
    quote_index_id: String,
    spot_lag_days: i32,
    payment_frequency: String,
    day_count: DayCount,
    business_day_convention: BusinessDayConvention,
    base_calendar_id: String,
    quote_calendar_id: String,
    #[serde(default)]
    notional_exchange: crate::instruments::rates::xccy_swap::NotionalExchange,
}

impl XccyConventionRecord {
    fn into_conventions(self) -> Result<XccyConventions, Error> {
        if self.base_currency == self.quote_currency {
            return Err(Error::Validation(
                "XCCY conventions must specify different base and quote currencies".to_string(),
            ));
        }
        if self.spot_lag_days < 0 || self.spot_lag_days > 7 {
            return Err(Error::Validation(format!(
                "XCCY spot_lag_days exceeds reasonable bound: {}",
                self.spot_lag_days
            )));
        }

        let payment_frequency = Tenor::parse(&self.payment_frequency).map_err(|e| {
            Error::Validation(format!(
                "Invalid `payment_frequency` in XCCY conventions registry: '{}': {}",
                self.payment_frequency, e
            ))
        })?;

        Ok(XccyConventions {
            base_currency: self.base_currency,
            quote_currency: self.quote_currency,
            base_index_id: IndexId::new(self.base_index_id),
            quote_index_id: IndexId::new(self.quote_index_id),
            spot_lag_days: self.spot_lag_days,
            payment_frequency,
            day_count: self.day_count,
            business_day_convention: self.business_day_convention,
            base_calendar_id: self.base_calendar_id,
            quote_calendar_id: self.quote_calendar_id,
            notional_exchange: self.notional_exchange,
        })
    }
}

/// Load XCCY conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<HashMap<XccyConventionId, XccyConventions>, Error> {
    let json = include_str!("../../../../data/conventions/xccy_conventions.json");
    super::json::parse_and_rekey(
        json,
        "XCCY",
        XccyConventionId::new,
        |rec: &XccyConventionRecord| rec.clone().into_conventions(),
    )
}

#[cfg(test)]
mod mtm_reset_loader_tests {
    use super::*;
    use crate::instruments::rates::xccy_swap::{NotionalExchange, ResettingSide};

    #[test]
    fn eur_usd_xccy_convention_defaults_to_mtm_resetting_on_leg1() {
        let registry = load_registry().expect("xccy registry loads");
        let conv = registry
            .get(&XccyConventionId::new("EUR/USD-XCCY"))
            .expect("EUR/USD-XCCY convention present");
        assert_eq!(
            conv.notional_exchange,
            NotionalExchange::MtmResetting {
                resetting_side: ResettingSide::Leg1,
            },
        );
    }

    #[test]
    fn xccy_convention_falls_back_to_initial_and_final_when_field_absent() {
        // If a future JSON entry omits `notional_exchange`, the default must be
        // `InitialAndFinal` (matches today's behaviour). This is exercised by
        // the serde default; we just confirm the type-level default here.
        assert_eq!(
            NotionalExchange::default(),
            NotionalExchange::InitialAndFinal,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eurusd_xccy_conventions_are_available() {
        let registry = load_registry().expect("xccy registry");
        let conv = registry
            .get(&XccyConventionId::new("EUR/USD-XCCY"))
            .expect("EUR/USD-XCCY conventions");

        assert_eq!(conv.base_currency, Currency::EUR);
        assert_eq!(conv.quote_currency, Currency::USD);
        assert_eq!(conv.base_index_id, IndexId::new("EUR-ESTR-OIS"));
        assert_eq!(conv.quote_index_id, IndexId::new("USD-SOFR-OIS"));
        assert_eq!(conv.spot_lag_days, 2);
        assert_eq!(conv.base_calendar_id, "target2");
        assert_eq!(conv.quote_calendar_id, "usny");
    }
}
