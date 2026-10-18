//! Lender draws on a note after closing: scheduled draws and re-advances up
//! to a borrowing base (warehouse facilities).

use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

/// A scheduled increase of one note's balance: the lender advances `amount`
/// into the deal on the first payment date at or after `date`; the cash
/// joins principal proceeds (recycled while the deal revolves).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheDraw {
    /// Id of the note drawn.
    pub tranche_id: String,
    /// Earliest draw date; applied on the first payment date at or after it.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Amount advanced, in the deal currency.
    pub amount: Money,
}

/// Re-advance one note each revolving period up to its commitment and the
/// borrowing base (`coverage_rules.borrowing_base` on the live collateral):
/// the draw is `max(0, min(commitment, borrowing base) − balance)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TrancheReadvance {
    /// Id of the note re-advanced.
    pub tranche_id: String,
    /// Commitment the note's balance may be drawn up to, in the deal currency.
    pub commitment: Money,
}
