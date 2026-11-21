//! Common functionality shared across multiple instruments.
//!
//! This module contains utilities and types that are used
//! by multiple instrument implementations, including:
//! - Core instrument traits (Instrument)
//! - Common helper functions
//! - Shared data structures and enums

// Core instrument traits and metadata
pub(crate) mod traits;

// Unified dependency representation
pub(crate) mod dependencies;

// Shared utilities and helper functions
pub mod helpers;
// Listed future / future-option terms (public at `instruments::*`)
pub(crate) mod listed;
pub mod margin;
// Shared volatility override/surface resolution.
pub mod vol_resolution;

// Common parameter types shared across instruments
pub mod parameters;
