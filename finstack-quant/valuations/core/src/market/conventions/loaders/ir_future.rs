//! Loader for interest rate future conventions embedded in JSON registries.

use crate::market::conventions::ids::IrFutureContractId;
use crate::market::conventions::{IrFutureConventions, IrFutureReferencePeriod};
use finstack_quant_core::Error;
use finstack_quant_core::FxHashMap;

fn validate_ir_future_conventions(conv: IrFutureConventions) -> Result<IrFutureConventions, Error> {
    if conv.delivery_months == 0 {
        return Err(Error::Validation(
            "IR future conventions delivery_months must be > 0".to_string(),
        ));
    }
    if !conv.face_value.is_finite() || conv.face_value <= 0.0 {
        return Err(Error::Validation(
            "IR future conventions face_value must be positive".to_string(),
        ));
    }
    if !conv.tick_size.is_finite() || conv.tick_size <= 0.0 {
        return Err(Error::Validation(
            "IR future conventions tick_size must be positive".to_string(),
        ));
    }
    if !conv.tick_value.is_finite() || conv.tick_value <= 0.0 {
        return Err(Error::Validation(
            "IR future conventions tick_value must be positive".to_string(),
        ));
    }
    if conv.settlement_days < 0 {
        return Err(Error::Validation(
            "IR future conventions settlement_days must be non-negative".to_string(),
        ));
    }
    if conv.reference_period != IrFutureReferencePeriod::ForwardStarting
        && conv.settlement_days != 0
    {
        return Err(Error::Validation(
            "IR future conventions settlement_days must be zero for in-arrears contracts"
                .to_string(),
        ));
    }
    if conv.reference_period == IrFutureReferencePeriod::ImmQuarterInArrears
        && conv.delivery_months != 3
    {
        return Err(Error::Validation(
            "IMM-quarter IR future conventions delivery_months must equal 3".to_string(),
        ));
    }
    if matches!(
        conv.reference_period,
        IrFutureReferencePeriod::CalendarMonthInArrears
            | IrFutureReferencePeriod::BusinessMonthInArrears
    ) && conv.delivery_months != 1
    {
        return Err(Error::Validation(
            "calendar-month IR future conventions delivery_months must equal 1".to_string(),
        ));
    }

    Ok(conv)
}

/// Load the IR futures conventions from the embedded JSON registry.
pub(crate) fn load_registry() -> Result<FxHashMap<IrFutureContractId, IrFutureConventions>, Error> {
    let json = include_str!("../../../../data/conventions/ir_future_conventions.json");
    super::json::parse_and_rekey(
        json,
        "IR future",
        IrFutureContractId::new,
        validate_ir_future_conventions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::types::IndexId;

    #[test]
    fn euribor_three_month_future_contract_is_available() {
        let registry = load_registry().expect("ir future registry");
        let euribor = registry
            .get(&IrFutureContractId::new("ICE:ER"))
            .expect("ICE:ER conventions");

        assert_eq!(euribor.index_id, IndexId::new("EUR-EURIBOR-3M"));
        assert_eq!(
            euribor.compounding,
            finstack_quant_cashflows::builder::FloatingLegCompounding::Simple
        );
        assert_eq!(
            euribor.reference_period,
            IrFutureReferencePeriod::ForwardStarting
        );
        assert_eq!(euribor.calendar_id, "target2");
        assert_eq!(euribor.settlement_days, 2);
        assert_eq!(euribor.delivery_months, 3);
        assert_eq!(euribor.face_value, 1_000_000.0);
        assert_eq!(euribor.tick_size, 0.005);
        assert_eq!(euribor.tick_value, 12.50);
    }
}
