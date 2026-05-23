//! Re-export shim for the relocated Higham (2002) nearest-correlation matrix
//! projection.
//!
//! The implementation now lives in
//! [`finstack_analytics::correlation::nearest_correlation`]; this module
//! preserves the old `finstack_valuations::correlation::nearest_correlation::*`
//! paths via re-export.

pub use finstack_analytics::correlation::nearest_correlation::{
    nearest_correlation_matrix, NearestCorrelationOpts,
};
