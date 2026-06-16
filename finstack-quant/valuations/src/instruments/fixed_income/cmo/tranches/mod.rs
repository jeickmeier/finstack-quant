//! CMO tranche implementations.
//!
//! This module provides specific logic for different tranche types.

pub mod io_po;
pub mod pac_support;
pub mod sequential;

pub use io_po::{IoStripCharacteristics, PoStripCharacteristics};
pub use pac_support::PacSchedule;
pub use sequential::SequentialOrder;
