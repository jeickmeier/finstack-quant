//! Duration cell tables giving a base-return curve for duration-matched
//! credit excess return attribution.
//!
//! There are two ways to build a [`DurationCellTable`]:
//!
//! 1. [`cell_returns_from_reference`] implements the bucketing method of
//!    Dynkin, Hyman & Vankudre (1998), Appendix B, "Attribution of Portfolio
//!    Performance Relative to an Index": reference instruments (typically
//!    on-the-run Treasuries) are bucketed into fixed-width duration cells; a
//!    cell's base return is the **simple average** of the total returns of
//!    every reference instrument that falls in it ("calculate the average
//!    return of all Treasuries in that cell"). Cells with no reference
//!    instrument are filled by linear interpolation between the nearest
//!    observed neighbours (interior gaps) or flat extrapolation from the
//!    nearest observed cell (leading/trailing gaps — matching Lehman's
//!    convention of extending the longest observed cell, e.g. `12.5+ =
//!    1.16%`).
//! 2. [`cell_returns_from_curves`] derives each cell's base return directly
//!    from two curve snapshots (start- and end-of-period discount curves)
//!    instead of averaging discrete reference returns: a hypothetical
//!    zero-coupon position bought at the cell midpoint and revalued off the
//!    end curve. This makes the base curve a caller choice (Treasury or
//!    swap/OIS) rather than a hardcoded assumption, and every cell is
//!    observed since a curve has a value at every queried point.
//!
//! Either table is the foundation for duration-matched credit excess
//! returns: a credit position's excess return over its duration-matched
//! Treasury (or swap) cell isolates the credit-specific component of
//! performance from the general level/shape move of the base curve.
//!
//! # References
//!
//! * Dynkin, L., Hyman, J., & Vankudre, P. (1998). "Attribution of Portfolio
//!   Performance Relative to an Index." Lehman Brothers Fixed Income
//!   Research, March 1998, Appendix B. `docs/REFERENCES.md#dynkin-hyman-vankudre-1998`
//!

mod engine;
#[cfg(test)]
mod tests;

pub use engine::{
    CellConfig, CellReturn, DurationCellTable, ExcessReturnPosition, ExcessReturnResult,
    PositionExcess, ReferenceReturn, cell_returns_from_curves, cell_returns_from_reference,
    excess_returns,
};
