//! SA-CCR replacement cost computation.
//!
//! RC differs for margined vs. unmargined netting sets.

use super::types::{SaCcrNettingSetConfig, SaCcrTrade};

/// Compute replacement cost for a netting set (BCBS 279 paragraph 135).
///
/// In this codebase `config.collateral` represents the variation margin
/// (VM) posted, and `config.nica` is the net independent collateral
/// amount held separately. The Basel formula uses total net collateral
/// `C = VM + NICA`.
///
/// Unmargined:
///   `RC = max(V - C, 0)` with `C = VM + NICA`
///
/// Margined:
///   `RC = max(V - C, TH + MTA - NICA, 0)` with `C = VM + NICA`
///   The second term captures the minimum possible exposure given the
///   margin agreement mechanics, and it subtracts NICA (not the full C)
///   because NICA is already offsetting the threshold.
///
/// # Arguments
///
/// * `config` - Netting-set collateral and margin-agreement terms.
/// * `trades` - Derivative trades whose current MtM values form `V`.
///
/// # Returns
///
/// Non-negative replacement cost for the netting set.
///
/// # References
///
/// - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`
pub fn replacement_cost(config: &SaCcrNettingSetConfig, trades: &[SaCcrTrade]) -> f64 {
    let v: f64 = trades.iter().map(|t| t.mtm).sum();
    let c = config.collateral + config.nica; // total net collateral

    if config.is_margined {
        let margin_term = config.threshold + config.mta - config.nica;
        f64::max(v - c, f64::max(margin_term, 0.0))
    } else {
        f64::max(v - c, 0.0)
    }
}
