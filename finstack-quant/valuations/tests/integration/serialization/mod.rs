//! Serialization Integration Tests
//!
//! Round-trip serialization tests ensuring data integrity:
//!
//! - [`instrument_roundtrip`]: JSON serialization for all instrument types
//! - [`result_roundtrip`]: ValuationResult serialization

pub mod instrument_roundtrip;
pub mod result_roundtrip;
