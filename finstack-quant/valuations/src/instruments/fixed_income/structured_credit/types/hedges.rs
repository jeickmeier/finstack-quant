//! Hedge swaps settled through the deal waterfall.
//!
//! A [`HedgeSwap`] wraps an [`InterestRateSwap`] with the notional it tracks
//! and the priority its payments take. Every payment period the swap's net
//! flow from the deal's side (receive leg minus pay leg, over the flows dated
//! in the period) is bucketed by the simulation engine: a net receipt joins
//! interest proceeds, a net payment becomes a fixed-amount
//! `SwapCounterparty` fee recipient in the senior or junior fee tier. Swap
//! payments therefore rank with the deal's fees, reduce the cash available to
//! the notes, and enter the IC test through the senior-fee netting, instead of
//! being valued as an NPV overlay outside the waterfall.

use super::TrancheStructure;
use crate::instruments::rates::irs::InterestRateSwap;
use crate::instruments::Instrument;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::Result;
use serde::{Deserialize, Serialize};

/// Notional a hedge swap's flows are scaled to each period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SwapNotional {
    /// The swap's own contractual notional; flows are used as scheduled.
    Contractual,
    /// Balance-guaranteed swap on one note: each period's flows are rescaled
    /// by `note balance at period start / contractual notional`.
    TranchePar(String),
    /// Balance-guaranteed swap on the collateral: flows are rescaled by
    /// `performing pool balance at period start / contractual notional`.
    PoolPar,
}

/// Where a hedge swap's net payments rank in the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SwapPriority {
    /// With the senior fees, ahead of every note (the usual hedge ranking).
    SeniorFee,
    /// After note interest and principal, ahead of the residual.
    JuniorFee,
}

/// An interest rate swap the deal pays and receives through its waterfall.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HedgeSwap {
    /// The swap; its `side` is the deal's side (`Pay` pays fixed and
    /// receives floating), and its notional currency must be the deal's.
    pub swap: InterestRateSwap,
    /// Notional the flows track each period.
    pub notional: SwapNotional,
    /// Fee tier the net payments rank in.
    pub priority: SwapPriority,
}

impl HedgeSwap {
    /// Wrap `swap` on its contractual notional with senior-fee priority.
    ///
    /// # Arguments
    ///
    /// * `swap` - Interest rate swap from the deal's side; its notional
    ///   currency must match the deal currency.
    #[must_use]
    pub fn new(swap: InterestRateSwap) -> Self {
        Self {
            swap,
            notional: SwapNotional::Contractual,
            priority: SwapPriority::SeniorFee,
        }
    }

    /// Track the balance of the note `tranche_id` (balance-guaranteed swap).
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Id of a note in the deal's capital structure whose
    ///   period-start balance rescales the swap's flows.
    #[must_use]
    pub fn on_tranche_par(mut self, tranche_id: impl Into<String>) -> Self {
        self.notional = SwapNotional::TranchePar(tranche_id.into());
        self
    }

    /// Track the performing collateral balance.
    #[must_use]
    pub fn on_pool_par(mut self) -> Self {
        self.notional = SwapNotional::PoolPar;
        self
    }

    /// Rank the net payments after the notes, ahead of the residual.
    #[must_use]
    pub fn junior(mut self) -> Self {
        self.priority = SwapPriority::JuniorFee;
        self
    }

    /// Validate the swap for pricing inside a deal in `base_currency` with
    /// `tranches`: the swap itself must be priceable, its notional positive
    /// and in the deal currency, and a `TranchePar` reference must name a
    /// note of the deal.
    pub(crate) fn validate(
        &self,
        base_currency: Currency,
        tranches: &TrancheStructure,
    ) -> Result<()> {
        self.swap.validate_for_pricing()?;
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        if self.swap.notional.currency() != base_currency {
            return Err(invalid(format!(
                "hedge swap {} is in {} but the deal is in {base_currency}",
                self.swap.id,
                self.swap.notional.currency()
            )));
        }
        if self.swap.notional.amount() <= 0.0 || !self.swap.notional.amount().is_finite() {
            return Err(invalid(format!(
                "hedge swap {} needs a positive notional",
                self.swap.id
            )));
        }
        if let SwapNotional::TranchePar(id) = &self.notional {
            if !tranches.tranches.iter().any(|t| t.id.as_str() == id) {
                return Err(invalid(format!(
                    "hedge swap {} tracks tranche {id}, which is not in the deal",
                    self.swap.id
                )));
            }
        }
        Ok(())
    }
}
