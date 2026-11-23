//! Attach post-fit validation outcomes to [`CalibrationReport`](crate::CalibrationReport).

use super::ValidationMode;
use crate::{CalibrationReport, config::CalibrationConfig};
use finstack_quant_core::Result;

pub(crate) fn attach_validation_result(
    report: CalibrationReport,
    validation: Result<()>,
    global_config: &CalibrationConfig,
) -> CalibrationReport {
    match validation {
        // A passing gate keeps any failure the target already recorded.
        Ok(()) => report,
        Err(err) => match global_config.validation_mode {
            ValidationMode::Error => report.with_validation_result(false, Some(err.to_string())),
            ValidationMode::Warn => {
                let mut report = report;
                report.update_metadata("validation_warning", err.to_string());
                report
            }
        },
    }
}
