//! Successful-fit gate shared by calibration benchmark targets.
use finstack_quant_calibration::api::{
    engine,
    schema::{CalibrationEnvelope, CalibrationResultEnvelope},
};

pub(super) fn checked_calibrate(envelope: &CalibrationEnvelope) -> CalibrationResultEnvelope {
    let output = engine::calibrate(envelope).expect("calibration must execute");
    assert!(
        output.result.report.success,
        "calibration must fit: {:?}",
        output.result.report
    );
    assert!(
        output
            .result
            .step_reports
            .values()
            .all(|report| report.success)
    );
    output
}
