//! Loader for interest rate future conventions embedded in JSON registries.

use crate::market::conventions::defs::IrFutureConventions;
use crate::market::conventions::ids::IrFutureContractId;
use finstack_core::types::IndexId;
use finstack_core::Error;
use finstack_core::HashMap;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct IrFutureConventionsRecord {
    index_id: String,
    calendar_id: String,
    settlement_days: i32,
    delivery_months: u8,
    face_value: f64,
    tick_size: f64,
    tick_value: f64,
    #[serde(default)]
    convexity_adjustment: Option<f64>,
}

impl IrFutureConventionsRecord {
    fn into_conventions(self) -> Result<IrFutureConventions, Error> {
        if self.delivery_months == 0 {
            return Err(Error::Validation(
                "IR future conventions delivery_months must be > 0".to_string(),
            ));
        }
        if !self.face_value.is_finite() || self.face_value <= 0.0 {
            return Err(Error::Validation(
                "IR future conventions face_value must be positive".to_string(),
            ));
        }
        if !self.tick_size.is_finite() || self.tick_size <= 0.0 {
            return Err(Error::Validation(
                "IR future conventions tick_size must be positive".to_string(),
            ));
        }
        if !self.tick_value.is_finite() || self.tick_value <= 0.0 {
            return Err(Error::Validation(
                "IR future conventions tick_value must be positive".to_string(),
            ));
        }
        if self.settlement_days < 0 {
            return Err(Error::Validation(
                "IR future conventions settlement_days must be non-negative".to_string(),
            ));
        }

        Ok(IrFutureConventions {
            index_id: IndexId::new(self.index_id),
            calendar_id: self.calendar_id,
            settlement_days: self.settlement_days,
            delivery_months: self.delivery_months,
            face_value: self.face_value,
            tick_size: self.tick_size,
            tick_value: self.tick_value,
            convexity_adjustment: self.convexity_adjustment,
        })
    }
}

/// Load the IR futures conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<HashMap<IrFutureContractId, IrFutureConventions>, Error> {
    let json = include_str!("../../../../data/conventions/ir_future_conventions.json");
    super::json::parse_and_rekey(
        json,
        "IR future",
        IrFutureContractId::new,
        |rec: &IrFutureConventionsRecord| rec.clone().into_conventions(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euribor_three_month_future_contract_is_available() {
        let registry = load_registry().expect("ir future registry");
        let euribor = registry
            .get(&IrFutureContractId::new("ICE:ER"))
            .expect("ICE:ER conventions");

        assert_eq!(euribor.index_id, IndexId::new("EUR-EURIBOR-3M"));
        assert_eq!(euribor.calendar_id, "target2");
        assert_eq!(euribor.settlement_days, 2);
        assert_eq!(euribor.delivery_months, 3);
        assert_eq!(euribor.face_value, 1_000_000.0);
        assert_eq!(euribor.tick_size, 0.005);
        assert_eq!(euribor.tick_value, 12.50);
    }
}
