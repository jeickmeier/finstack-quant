//! Fund cash-flow events and waterfall allocation ledger accounting types.

use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Type of fund event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FundEventKind {
    /// Capital contribution from LP
    Contribution,
    /// Distribution to LP
    Distribution,
    /// Sale proceeds from investment
    Proceeds,
}

/// Single fund cash flow event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FundEvent {
    /// Date of the event
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Amount (positive for all event types, sign determined by kind)
    pub amount: Money,
    /// Type of event
    pub kind: FundEventKind,
    /// Deal identifier. Required for every event in an American-style
    /// deal-by-deal waterfall.
    pub deal_id: Option<String>,
}

impl FundEvent {
    /// Create a capital contribution event.
    pub fn contribution(date: Date, amount: Money) -> Self {
        Self {
            date,
            amount,
            kind: FundEventKind::Contribution,
            deal_id: None,
        }
    }

    /// Create a distribution event.
    pub fn distribution(date: Date, amount: Money) -> Self {
        Self {
            date,
            amount,
            kind: FundEventKind::Distribution,
            deal_id: None,
        }
    }

    /// Create a proceeds event with deal ID.
    pub fn proceeds(date: Date, amount: Money, deal_id: impl Into<String>) -> Self {
        Self {
            date,
            amount,
            kind: FundEventKind::Proceeds,
            deal_id: Some(deal_id.into()),
        }
    }

    /// Set the deal ID for American-style waterfalls.
    pub fn with_deal_id(mut self, deal_id: impl Into<String>) -> Self {
        self.deal_id = Some(deal_id.into());
        self
    }
}

/// Single row in the allocation ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AllocationRow {
    /// Date of allocation
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Deal ID for American-style waterfalls
    pub deal_id: Option<Arc<str>>,
    /// PeFundWaterfallTranche name/description
    pub tranche: Arc<str>,
    /// Amount allocated to LP
    pub to_lp: Money,
    /// Amount allocated to GP
    pub to_gp: Money,
    /// LP unreturned capital balance after allocation
    pub lp_unreturned: Money,
    /// GP cumulative carry after allocation
    pub gp_carry_cum: Money,
    /// LP IRR to date (if calculable)
    pub lp_irr_to_date: Option<f64>,
    /// Optional note/description
    pub note: Option<Arc<str>>,
}

/// Complete allocation ledger with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AllocationLedger {
    /// Allocation rows
    pub rows: Vec<AllocationRow>,
    /// Result metadata
    pub meta: ResultsMeta,
    /// LP contribution events stored as negative flows for cashflow reconstruction
    #[serde(default)]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub(super) contributions: Vec<(Date, Money)>,
}

impl AllocationLedger {
    /// Extract LP-only cashflows for NPV calculation.
    pub fn lp_cashflows(&self) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut by_date: IndexMap<Date, f64> = IndexMap::new();
        let mut currency: Option<Currency> = None;

        for (date, amount) in &self.contributions {
            currency = currency.or(Some(amount.currency()));
            *by_date.entry(*date).or_default() += amount.amount();
        }

        for row in &self.rows {
            let amt = row.to_lp.amount();
            if amt.abs() > 1e-6 {
                currency = currency.or(Some(row.to_lp.currency()));
                *by_date.entry(row.date).or_default() += amt;
            }
        }

        let Some(ccy) = currency else {
            return Ok(Vec::new());
        };

        let mut flows: Vec<(Date, Money)> = by_date
            .into_iter()
            .filter(|(_, amt)| amt.abs() > 1e-6)
            .map(|(date, amt)| Money::from_model_value(amt, ccy).map(|money| (date, money)))
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;

        flows.sort_by_key(|(date, _)| *date);
        Ok(flows)
    }
}
