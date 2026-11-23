//! Replay fit-acceptance check for quote-space recalibration.

use crate::CalibrationConfig;

pub(super) fn ensure_replay_fit_accepted(
    curve_id: &str,
    config: &CalibrationConfig,
    report: &crate::CalibrationReport,
) -> finstack_quant_core::Result<()> {
    if !config.fail_on_bad_fit || report.success {
        return Ok(());
    }

    let tolerance = report
        .success_tolerance
        .map(|value| format!("{value:.3e}"))
        .unwrap_or_else(|| "unspecified".to_string());
    let worst = match (&report.worst_quote_id, report.worst_quote_residual) {
        (Some(id), Some(residual)) => {
            format!(", worst quote '{id}' residual {residual:.3e}")
        }
        _ => String::new(),
    };
    Err(finstack_quant_core::Error::Calibration {
        message: format!(
            "calibration replay for '{curve_id}' failed fit acceptance: max residual {:.3e} \
             with tolerance {tolerance}{worst}; {}",
            report.max_residual,
            report
                .validation_error
                .as_deref()
                .unwrap_or(&report.convergence_reason)
        ),
        category: "replay_bad_fit".to_string(),
    })
}
