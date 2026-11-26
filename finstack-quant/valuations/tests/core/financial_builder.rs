//! The `FinancialBuilder` derive reports every required field that was not
//! set, on a struct declared for the test so the field order, the defaulted
//! field and the optional field are all under its control.

use finstack_quant_core::Error;
use finstack_quant_valuations_macros::FinancialBuilder;

/// Accepts every assembled value: these tests are about fields that are not set.
fn accept(_sample: &Sample) -> finstack_quant_core::Result<()> {
    Ok(())
}

/// Three required fields around one defaulted and one optional field.
#[derive(Clone, Debug, PartialEq, FinancialBuilder)]
#[builder(validate = accept)]
pub struct Sample {
    /// Required; first in declaration order.
    pub first: String,
    /// Required with a default, so never reported as missing.
    #[builder(default = 2)]
    pub defaulted: u32,
    /// Required; second in declaration order.
    pub second: f64,
    /// Optional, so never reported as missing.
    pub optional: Option<String>,
    /// Required; last in declaration order.
    pub third: bool,
}

/// The validation message `build()` fails with.
fn missing(builder: SampleBuilder) -> String {
    match builder.build() {
        Err(Error::Validation(message)) => message,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn one_missing_field_keeps_the_established_message() {
    let builder = Sample::builder().first("a".to_string()).second(1.0);
    assert_eq!(
        missing(builder),
        "SampleBuilder: missing required field 'third'"
    );
}

#[test]
fn every_missing_field_is_named_in_declaration_order() {
    assert_eq!(
        missing(Sample::builder()),
        "SampleBuilder: missing required field 'first' (also missing: 'second', 'third')"
    );
    // Setter order does not matter: the report follows the declaration.
    assert_eq!(
        missing(Sample::builder().second(1.0)),
        "SampleBuilder: missing required field 'first' (also missing: 'third')"
    );
}

#[test]
fn defaulted_and_optional_fields_are_not_required() {
    let sample = Sample::builder()
        .first("a".to_string())
        .second(1.0)
        .third(true)
        .build()
        .expect("every required field is set");
    assert_eq!(sample.defaulted, 2);
    assert_eq!(sample.optional, None);
}
