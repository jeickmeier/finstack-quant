use finstack_quant_core::types::IssuerId;
use finstack_quant_core::{Error, Result};

use super::config::{
    BetaShrinkage, BucketWeighting, CovarianceStrategy, CreditCalibrationConfig, VolModelChoice,
};
use super::inputs::CreditCalibrationInputs;
use crate::credit::hierarchy::dimension_key;
use crate::credit::peel::dts_spread_duration;
use crate::credit::units::validate_decimal_spread;

pub(super) fn validation_err(msg: impl Into<String>) -> Error {
    Error::Validation(msg.into())
}

fn validate_finite(label: impl std::fmt::Display, value: f64) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(validation_err(format!(
            "{label} must be finite, got {value}"
        )))
    }
}

fn validate_non_negative_finite(label: impl std::fmt::Display, value: f64) -> Result<()> {
    validate_finite(&label, value)?;
    if value >= 0.0 {
        Ok(())
    } else {
        Err(validation_err(format!(
            "{label} must be non-negative, got {value}"
        )))
    }
}

pub(super) fn validate_calibration_config(config: &CreditCalibrationConfig) -> Result<()> {
    if let BetaShrinkage::TowardOne { alpha } = config.beta_shrinkage {
        validate_finite("CreditCalibrator: beta_shrinkage alpha", alpha)?;
        if !(0.0..=1.0).contains(&alpha) {
            return Err(validation_err(format!(
                "CreditCalibrator: beta_shrinkage alpha must be in [0, 1], got {alpha}"
            )));
        }
    }

    if let CovarianceStrategy::Ridge { alpha } = config.covariance_strategy {
        validate_non_negative_finite("CreditCalibrator: ridge alpha", alpha)?;
    }

    if let VolModelChoice::Ewma { lambda } = config.vol_model {
        validate_finite("CreditCalibrator: ewma lambda", lambda)?;
        if !(lambda > 0.0 && lambda < 1.0) {
            return Err(validation_err(format!(
                "CreditCalibrator: ewma lambda must be in the open interval (0, 1), got {lambda}"
            )));
        }
    }

    // Custom dimension keys join into dotted dimension paths inside factor
    // IDs; a '.' inside the key would mis-segment those paths the same way a
    // dotted tag value would.
    for dim in &config.hierarchy.levels {
        let key = dimension_key(dim);
        if key.contains('.') {
            return Err(validation_err(format!(
                "CreditCalibrator: hierarchy dimension key {key:?} contains '.', \
                 which is reserved as the path separator"
            )));
        }
    }

    Ok(())
}

pub(super) fn validate_calibration_inputs(
    inputs: &CreditCalibrationInputs,
    config: &CreditCalibrationConfig,
) -> Result<()> {
    // Date grid must be strictly increasing (sorted, no duplicates): every
    // downstream step (differencing, as_of lookup, history alignment) assumes
    // it, and a shuffled or duplicated grid would silently corrupt returns.
    for pair in inputs.history_panel.dates.windows(2) {
        if pair[0] >= pair[1] {
            return Err(validation_err(format!(
                "CreditCalibrator: history_panel.dates must be strictly increasing; \
                 found {:?} followed by {:?}",
                pair[0], pair[1]
            )));
        }
    }

    validate_regular_grid(&inputs.history_panel.dates, config.panel_frequency)?;

    // The anchor must be the panel end. An earlier as_of would let
    // post-as_of history leak into betas, vols, and correlations
    // (look-ahead), silently invalidating any backtest built on the
    // artifact. Callers wanting an earlier anchor must truncate the panel.
    if let Some(last) = inputs.history_panel.dates.last() {
        if inputs.as_of != *last {
            return Err(validation_err(format!(
                "CreditCalibrator: as_of {:?} must equal the last panel date {:?}; \
                 calibrating with history after as_of is look-ahead — truncate \
                 history_panel (and generic_factor.values) at as_of instead",
                inputs.as_of, last
            )));
        }
    }

    // The anchor cross-section must cover exactly the calibrated universe.
    // A history issuer missing from `as_of_spreads` would silently receive
    // `adder_at_anchor = 0.0` and shift every bucket peer's anchor mean; an
    // asof-only issuer would silently enter anchor bucket means with unit
    // betas while receiving no artifact row. Both directions are data gaps
    // the caller must resolve explicitly.
    let history_only: Vec<&str> = inputs
        .history_panel
        .spreads
        .keys()
        .filter(|id| !inputs.as_of_spreads.contains_key(*id))
        .map(IssuerId::as_str)
        .collect();
    if !history_only.is_empty() {
        return Err(validation_err(format!(
            "CreditCalibrator: as_of_spreads is missing {} issuer(s) present in \
             history_panel.spreads (first few: {:?}); supply an as_of spread for \
             every calibrated issuer",
            history_only.len(),
            &history_only[..history_only.len().min(5)]
        )));
    }
    let asof_only: Vec<&str> = inputs
        .as_of_spreads
        .keys()
        .filter(|id| !inputs.history_panel.spreads.contains_key(*id))
        .map(IssuerId::as_str)
        .collect();
    if !asof_only.is_empty() {
        return Err(validation_err(format!(
            "CreditCalibrator: as_of_spreads contains {} issuer(s) absent from \
             history_panel.spreads (first few: {:?}); anchor-only issuers would \
             distort bucket anchors without receiving an artifact row",
            asof_only.len(),
            &asof_only[..asof_only.len().min(5)]
        )));
    }

    for (idx, value) in inputs.generic_factor.values.iter().copied().enumerate() {
        validate_decimal_spread(
            format!("CreditCalibrator: generic_factor.values[{idx}]"),
            value,
        )?;
    }

    for (issuer, series) in &inputs.history_panel.spreads {
        for (idx, value) in series.iter().copied().enumerate() {
            let Some(spread) = value else {
                return Err(validation_err(format!(
                    "CreditCalibrator: spread series for issuer {:?} is missing an \
                     observation at index {idx} (date {:?}); the panel must be \
                     fully aligned with no None entries",
                    issuer.as_str(),
                    inputs.history_panel.dates.get(idx)
                )));
            };
            validate_decimal_spread(
                format!(
                    "CreditCalibrator: spread series for issuer {:?} at index {idx}",
                    issuer.as_str()
                ),
                spread,
            )?;
        }
    }

    for (issuer, spread) in &inputs.as_of_spreads {
        validate_decimal_spread(
            format!(
                "CreditCalibrator: as_of_spreads for issuer {:?}",
                issuer.as_str()
            ),
            *spread,
        )?;
    }

    for (issuer, vol) in &inputs.idiosyncratic_overrides {
        validate_non_negative_finite(
            format!(
                "CreditCalibrator: idiosyncratic override for issuer {:?}",
                issuer.as_str()
            ),
            *vol,
        )?;
    }

    match config.bucket_weighting {
        BucketWeighting::Equal => {}
        BucketWeighting::Dts => {
            for issuer in inputs.history_panel.spreads.keys() {
                dts_spread_duration(issuer, &inputs.spread_durations)
                    .map_err(|message| validation_err(format!("CreditCalibrator: {message}")))?;
            }
            let extra: Vec<&str> = inputs
                .spread_durations
                .keys()
                .filter(|id| !inputs.history_panel.spreads.contains_key(*id))
                .map(IssuerId::as_str)
                .collect();
            if !extra.is_empty() {
                return Err(validation_err(format!(
                    "CreditCalibrator: spread_durations contains {} issuer(s) absent \
                     from history_panel.spreads (first few: {:?})",
                    extra.len(),
                    &extra[..extra.len().min(5)]
                )));
            }
        }
    }

    Ok(())
}

fn validate_regular_grid(
    dates: &[finstack_quant_core::dates::Date],
    frequency: super::config::PanelFrequency,
) -> Result<()> {
    use finstack_quant_core::dates::DateExt;

    if dates.is_empty() {
        return Ok(());
    }
    // Daily panels are business-day series: exact-grid matching would reject
    // every real market panel at the first holiday, so the check is weekday
    // membership plus a bounded gap. The 7-calendar-day bound tolerates
    // weekends and holiday closures up to a full week while rejecting grids
    // that are obviously not daily (fortnightly, monthly). Annualization for
    // Daily is the market-standard 252 (see `PanelFrequency::Daily`).
    if frequency == super::config::PanelFrequency::Daily {
        const MAX_DAILY_GAP_DAYS: i64 = 7;
        for (idx, date) in dates.iter().copied().enumerate() {
            if date.is_weekend() {
                return Err(validation_err(format!(
                    "CreditCalibrator: history_panel.dates is not a business-day \
                     (Daily) grid; date {date:?} at index {idx} falls on a weekend"
                )));
            }
        }
        for (idx, pair) in dates.windows(2).enumerate() {
            let gap = (pair[1] - pair[0]).whole_days();
            if gap > MAX_DAILY_GAP_DAYS {
                return Err(validation_err(format!(
                    "CreditCalibrator: history_panel.dates is not a Daily grid; \
                     gap of {gap} calendar days between {:?} and {:?} (indices \
                     {idx} and {}) exceeds the {MAX_DAILY_GAP_DAYS}-day holiday \
                     tolerance",
                    pair[0],
                    pair[1],
                    idx + 1
                )));
            }
        }
        return Ok(());
    }
    let origin = dates[0];
    for (idx, actual) in dates.iter().copied().enumerate() {
        let steps = i32::try_from(idx).map_err(|_| {
            validation_err("CreditCalibrator: history_panel.dates is longer than i32::MAX")
        })?;
        let expected = frequency.date_after(origin, steps)?;
        if actual != expected {
            return Err(validation_err(format!(
                "CreditCalibrator: history_panel.dates is not a regular {frequency:?} \
                 grid; at index {idx} expected {expected:?}, got {actual:?}"
            )));
        }
    }
    Ok(())
}
