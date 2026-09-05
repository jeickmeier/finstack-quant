//! Model-parameter replacement for P&L attribution.

use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;

/// Create a modified instrument with different model parameters.
///
/// Clones the instrument and replaces its model parameters with those from
/// the snapshot. Used for isolating model parameter P&L in attribution.
///
/// # Arguments
///
/// * `instrument` - Original instrument
/// * `params` - Model parameters to apply
///
/// # Returns
///
/// New instrument with modified parameters, or original if no params to modify.
///
/// # Errors
///
/// Returns error if instrument type doesn't match snapshot type.
///
pub(crate) fn with_model_params(
    instrument: &Arc<dyn Instrument>,
    params: &ModelParamsSnapshot,
) -> Result<Arc<dyn Instrument>> {
    if matches!(params, ModelParamsSnapshot::None) {
        return Ok(Arc::clone(instrument));
    }

    instrument.with_model_params(params).map(Arc::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
    use finstack_quant_valuations::instruments::fixed_income::convertible::{
        AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
    };
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::StructuredCredit;
    use time::Month;

    fn plain_bond() -> Arc<dyn Instrument> {
        Arc::new(
            Bond::fixed(
                "PLAIN-BOND-002",
                Money::from((1_000_000_i64, Currency::USD)),
                finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
                create_date(2024, Month::January, 1).expect("valid issue date"),
                create_date(2029, Month::January, 1).expect("valid maturity date"),
                finstack_quant_core::dates::StubKind::ShortFront,
                "USD-OIS",
            )
            .expect("valid fixed bond"),
        )
    }

    #[test]
    fn test_no_model_params_reuses_original_instrument() {
        let instrument = plain_bond();
        let unchanged = with_model_params(&instrument, &ModelParamsSnapshot::None)
            .expect("None model params should be a no-op");

        assert!(Arc::ptr_eq(&instrument, &unchanged));
    }

    #[test]
    fn test_mismatched_model_params_report_expected_instrument_type() {
        let instrument: Arc<dyn Instrument> = Arc::new(StructuredCredit::example());
        let params = ModelParamsSnapshot::Convertible {
            conversion_spec: ConversionSpec {
                ratio: Some(10.0),
                price: None,
                policy: ConversionPolicy::Voluntary,
                anti_dilution: AntiDilutionPolicy::None,
                dividend_adjustment: DividendAdjustment::None,
                dilution_events: Vec::new(),
            },
        };

        let Err(error) = with_model_params(&instrument, &params) else {
            panic!("mismatched model parameter snapshot should fail")
        };

        assert!(error.to_string().contains("StructuredCredit"));
    }
}
