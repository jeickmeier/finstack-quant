//! Calibration Schema.
//!
//! Defines the JSON contract for plan-driven calibration.

// `EnvelopeError` is intentionally large (carries available-IDs lists and
// strict-load diagnostics) because the cross-binding consumers want all the
// diagnostic context in one shot. Boxing the error would harm ergonomics on a
// cold error path. Mirrors the same allowance in `api::engine` and
// `api::validate`.
#![allow(clippy::result_large_err)]

mod envelope;
mod steps;
mod surface_steps;
#[cfg(test)]
mod tests;

pub use envelope::{
    CALIBRATION_CONTRACT, CALIBRATION_SCHEMA, CalibrationEnvelope, CalibrationPlan,
    CalibrationResult, CalibrationResultEnvelope, CalibrationSchema, CalibrationStep,
    StepWithQuotes,
};
pub use steps::{
    DiscountCurveParams, ForwardCurveParams, HazardCurveParams, InflationCurveParams,
    ParametricCurveParams, StepParams, XccyBasisParams,
};
pub use surface_steps::{
    BaseCorrelationParams, CapFloorHullWhiteStepParams, HullWhiteStepParams,
    HullWhiteVolatilityMode, StudentTParams, SurfaceExtrapolationPolicy, SviSurfaceParams,
    SwaptionVolConvention, SwaptionVolParams, VolSurfaceParams,
};
