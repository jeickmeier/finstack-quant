//! Equity waterfall engine for private markets fund distribution calculations.
//!
//! This module implements the core waterfall allocation logic used in private
//! markets funds, including return of capital, preferred IRR hurdles, catch-up
//! provisions, promote splits, and clawback mechanisms.

use crate::instruments::common_impl::validation;
use finstack_core::config::{results_meta, FinstackConfig, ResultsMeta};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::math::solver::BrentSolver;
use finstack_core::money::Money;

use finstack_core::HashMap;
use indexmap::IndexMap;
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::sync::Arc;
use time::Duration;

use serde::{Deserialize, Serialize};

/// Waterfall allocation style.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WaterfallStyle {
    /// European style: aggregate all events at fund level
    #[default]
    European,
    /// American style: allocate per deal, then aggregate
    American,
}

/// Catch-up mode for GP profit sharing.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CatchUpMode {
    /// Full catch-up: GP gets 100% until target split is reached
    #[default]
    Full,
    /// Partial catch-up: GP gets configured percentage
    Partial,
}

/// Hurdle types for waterfall tiers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Hurdle {
    /// IRR-based hurdle (annual rate)
    Irr {
        /// Rate.
        rate: f64,
    },
    // Future: Moic { multiple: F } - can be added without breaking serde
}

/// Individual tranche in the waterfall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tranche {
    /// Return LP capital contributions before any profit sharing
    ReturnOfCapital,
    /// Preferred return to LPs at specified IRR
    PreferredIrr {
        /// Irr.
        irr: f64,
    },
    /// Catch-up allocation to GP
    CatchUp {
        /// Gp share.
        gp_share: f64,
    },
    /// Promote tier with hurdle and LP/GP split
    PromoteTier {
        /// Hurdle.
        hurdle: Hurdle,
        /// Lp share.
        lp_share: f64,
        /// Gp share.
        gp_share: f64,
    },
}

/// Clawback settlement trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClawbackSettle {
    /// Settle at fund termination
    FundEnd,
    /// Settle periodically (quarterly/annually)
    Periodic,
}

/// Clawback specification for GP carry reconciliation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClawbackSpec {
    /// Whether clawback is enabled
    pub enable: bool,
    /// Optional percentage of GP carry held back until settlement
    pub holdback_pct: Option<f64>,
    /// When to settle clawback
    pub settle_on: ClawbackSettle,
}

impl Default for ClawbackSpec {
    fn default() -> Self {
        Self {
            enable: false,
            holdback_pct: None,
            settle_on: ClawbackSettle::FundEnd,
        }
    }
}

/// Complete waterfall specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaterfallSpec {
    /// Allocation style (European vs American)
    pub style: WaterfallStyle,
    /// Ordered sequence of waterfall tranches
    #[schemars(with = "Vec<Tranche>")]
    pub tranches: SmallVec<[Tranche; 8]>,
    /// Optional clawback specification
    #[serde(default)]
    pub clawback: Option<ClawbackSpec>,
    /// Day count basis for IRR calculations
    #[serde(default = "default_irr_basis")]
    pub irr_basis: DayCount,
    /// Catch-up mode
    #[serde(default)]
    pub catchup_mode: CatchUpMode,
}

fn default_irr_basis() -> DayCount {
    DayCount::Act365F
}

impl WaterfallSpec {
    /// Create a new waterfall specification builder.
    #[must_use]
    pub fn builder() -> WaterfallSpecBuilder {
        WaterfallSpecBuilder::new()
    }

    /// Validate the waterfall specification.
    pub fn validate(&self) -> finstack_core::Result<()> {
        validation::require_or(
            !self.tranches.is_empty(),
            finstack_core::InputError::TooFewPoints,
        )?;

        // Validate promote tier splits sum to 1.0
        for tranche in &self.tranches {
            if let Tranche::PromoteTier {
                lp_share, gp_share, ..
            } = tranche
            {
                let sum = lp_share + gp_share;
                validation::require_or(
                    (sum - 1.0).abs() <= 1e-6,
                    finstack_core::InputError::Invalid,
                )?;
                validation::require_or(
                    lp_share.is_finite() && gp_share.is_finite(),
                    finstack_core::InputError::Invalid,
                )?;
                validation::require_or(
                    *lp_share >= 0.0 && *gp_share >= 0.0,
                    finstack_core::InputError::NegativeValue,
                )?;
            }
        }

        Ok(())
    }
}

/// Builder for waterfall specifications.
pub struct WaterfallSpecBuilder {
    style: WaterfallStyle,
    tranches: SmallVec<[Tranche; 8]>,
    clawback: Option<ClawbackSpec>,
    irr_basis: DayCount,
    catchup_mode: CatchUpMode,
}

impl Default for WaterfallSpecBuilder {
    fn default() -> Self {
        Self {
            style: WaterfallStyle::default(),
            tranches: SmallVec::new(),
            clawback: None,
            irr_basis: default_irr_basis(),
            catchup_mode: CatchUpMode::default(),
        }
    }
}

impl WaterfallSpecBuilder {
    /// new.
    pub fn new() -> Self {
        Self::default()
    }

    /// style.
    pub fn style(mut self, style: WaterfallStyle) -> Self {
        self.style = style;
        self
    }

    /// irr basis.
    pub fn irr_basis(mut self, basis: DayCount) -> Self {
        self.irr_basis = basis;
        self
    }

    /// catchup mode.
    pub fn catchup_mode(mut self, mode: CatchUpMode) -> Self {
        self.catchup_mode = mode;
        self
    }

    /// return of capital.
    pub fn return_of_capital(mut self) -> Self {
        self.tranches.push(Tranche::ReturnOfCapital);
        self
    }

    /// preferred irr.
    pub fn preferred_irr(mut self, irr: f64) -> Self {
        self.tranches.push(Tranche::PreferredIrr { irr });
        self
    }

    /// catchup.
    pub fn catchup(mut self, gp_share: f64) -> Self {
        self.tranches.push(Tranche::CatchUp { gp_share });
        self
    }

    /// promote tier.
    pub fn promote_tier(mut self, hurdle_irr: f64, lp_share: f64, gp_share: f64) -> Self {
        self.tranches.push(Tranche::PromoteTier {
            hurdle: Hurdle::Irr { rate: hurdle_irr },
            lp_share,
            gp_share,
        });
        self
    }

    /// clawback.
    pub fn clawback(mut self, spec: ClawbackSpec) -> Self {
        self.clawback = Some(spec);
        self
    }

    /// build.
    pub fn build(self) -> finstack_core::Result<WaterfallSpec> {
        let spec = WaterfallSpec {
            style: self.style,
            tranches: self.tranches,
            clawback: self.clawback,
            irr_basis: self.irr_basis,
            catchup_mode: self.catchup_mode,
        };
        spec.validate()?;
        Ok(spec)
    }
}

/// Type of fund event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundEvent {
    /// Date of the event
    #[schemars(with = "String")]
    pub date: Date,
    /// Amount (positive for all event types, sign determined by kind)
    pub amount: Money,
    /// Type of event
    pub kind: FundEventKind,
    /// Deal identifier for American-style waterfalls
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

    /// Get the signed amount for IRR calculations.
    /// Contributions are negative (outflows), distributions/proceeds are positive (inflows).
    pub fn signed_amount(&self) -> Money {
        match self.kind {
            FundEventKind::Contribution => {
                Money::new(-self.amount.amount(), self.amount.currency())
            }
            FundEventKind::Distribution | FundEventKind::Proceeds => self.amount,
        }
    }
}

/// Single row in the allocation ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AllocationRow {
    /// Date of allocation
    #[schemars(with = "String")]
    pub date: Date,
    /// Period key (for grouping)
    pub period_key: Option<Arc<str>>,
    /// Deal ID for American-style waterfalls
    pub deal_id: Option<Arc<str>>,
    /// Tranche name/description
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
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AllocationLedger {
    /// Allocation rows
    pub rows: Vec<AllocationRow>,
    /// Result metadata
    pub meta: ResultsMeta,
    /// LP contribution events stored as negative flows for cashflow reconstruction
    #[serde(default)]
    #[schemars(with = "Vec<(String, Money)>")]
    contributions: Vec<(Date, Money)>,
}

impl AllocationLedger {
    /// Extract LP-only cashflows for NPV calculation.
    pub fn lp_cashflows(&self) -> Vec<(Date, Money)> {
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
            return Vec::new();
        };

        let mut flows: Vec<(Date, Money)> = by_date
            .into_iter()
            .filter_map(|(date, amt)| {
                if amt.abs() > 1e-6 {
                    Some((date, Money::new(amt, ccy)))
                } else {
                    None
                }
            })
            .collect();

        flows.sort_by_key(|(date, _)| *date);
        flows
    }

    /// Export allocation ledger as structured data for DataFrame creation.
    /// Returns column names and data vectors suitable for external DataFrame libraries.
    pub fn to_tabular_data(&self) -> (Vec<&'static str>, Vec<Vec<String>>) {
        let column_names = vec![
            "date",
            "period_key",
            "deal_id",
            "tranche",
            "to_lp",
            "to_gp",
            "lp_unreturned",
            "gp_carry_cum",
            "lp_irr_to_date",
            "note",
        ];

        let mut rows = Vec::new();
        for row in &self.rows {
            let mut row_data = Vec::new();
            row_data.push(row.date.to_string());
            row_data.push(row.period_key.as_deref().unwrap_or("").to_string());
            row_data.push(row.deal_id.as_deref().unwrap_or("").to_string());
            row_data.push(row.tranche.to_string());
            row_data.push(row.to_lp.amount().to_string());
            row_data.push(row.to_gp.amount().to_string());
            row_data.push(row.lp_unreturned.amount().to_string());
            row_data.push(row.gp_carry_cum.amount().to_string());
            row_data.push(
                row.lp_irr_to_date
                    .map_or("".to_string(), |irr| format!("{:.6}", irr)),
            );
            row_data.push(row.note.as_deref().unwrap_or("").to_string());
            rows.push(row_data);
        }

        (column_names, rows)
    }

    /// Export as JSON string for external analysis.
    pub fn to_json(&self) -> finstack_core::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|_| finstack_core::Error::internal("failed to serialize PE fund waterfall"))
    }
}

#[allow(clippy::too_many_arguments)]
struct AllocationParams<'e> {
    total_amount: Money,
    initial_lp_unreturned: f64,
    initial_gp_carry: f64,
    lp_distributed_cum_before: f64,
    all_events: &'e [FundEvent],
    /// Ledger rows persisted before this distribution call. LP IRR and
    /// preferred-return solves are computed from contributions plus these
    /// rows' `to_lp` amounts — the LP's actual net receipts — not from gross
    /// fund events .
    prior_rows: &'e [AllocationRow],
    allocation_date: Date,
    currency: Currency,
}

/// Equity waterfall calculation engine.
pub struct EquityWaterfallEngine<'a> {
    spec: &'a WaterfallSpec,
    periods: Option<Vec<finstack_core::dates::Period>>,
}

impl<'a> EquityWaterfallEngine<'a> {
    /// Create a new waterfall engine.
    pub fn new(spec: &'a WaterfallSpec) -> Self {
        Self {
            spec,
            periods: None,
        }
    }

    /// Add period support for tagging allocation rows with period keys.
    pub fn with_periods(mut self, periods: Vec<finstack_core::dates::Period>) -> Self {
        self.periods = Some(periods);
        self
    }

    /// Add period support using a range expression like "2024Q4..2025Q2".
    pub fn with_period_range(
        mut self,
        range: &str,
        actuals_until: Option<&str>,
    ) -> finstack_core::Result<Self> {
        let plan = finstack_core::dates::build_periods(range, actuals_until)?;
        self.periods = Some(plan.periods);
        Ok(self)
    }

    /// Map a date to its period key if periods are configured.
    fn period_key_for(&self, date: Date) -> Option<String> {
        let periods = self.periods.as_ref()?;
        for period in periods {
            if date >= period.start && date < period.end {
                return Some(period.id.to_string());
            }
        }
        None
    }

    /// Run the waterfall allocation on the given events.
    pub fn run(&self, events: &[FundEvent]) -> finstack_core::Result<AllocationLedger> {
        // Validate and sort events
        let mut sorted_events = events.to_vec();
        sorted_events.sort_by(|a, b| a.date.cmp(&b.date));

        // Currency-safety: the waterfall performs raw f64 arithmetic across event
        // amounts, so every event must share a single currency. Reject mixed
        // currencies up front rather than silently summing across them. (Empty
        // input is left to the per-style handlers, which return `TooFewPoints`.)
        if let Some(first) = sorted_events.first() {
            let expected = first.amount.currency();
            if let Some(bad) = sorted_events
                .iter()
                .find(|e| e.amount.currency() != expected)
            {
                return Err(finstack_core::Error::CurrencyMismatch {
                    expected,
                    actual: bad.amount.currency(),
                });
            }
        }

        let contributions: Vec<(Date, Money)> = sorted_events
            .iter()
            .filter(|e| e.kind == FundEventKind::Contribution)
            .map(|e| (e.date, Money::new(-e.amount.amount(), e.amount.currency())))
            .collect();

        let mut ledger_rows = Vec::new();

        match self.spec.style {
            WaterfallStyle::European => {
                self.run_european(&sorted_events, &mut ledger_rows)?;
            }
            WaterfallStyle::American => {
                self.run_american(&sorted_events, &mut ledger_rows)?;
            }
        }

        // Apply clawback if specified
        if let Some(clawback_spec) = &self.spec.clawback {
            if clawback_spec.enable {
                self.apply_clawback(&sorted_events, &mut ledger_rows, clawback_spec)?;
            }
        }

        let config = FinstackConfig::default();
        let meta = results_meta(&config);

        Ok(AllocationLedger {
            rows: ledger_rows,
            meta,
            contributions,
        })
    }

    /// Run European-style waterfall (fund-level aggregation).
    fn run_european(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
    ) -> finstack_core::Result<()> {
        // Aggregate all events at fund level
        let mut lp_unreturned = 0.0;
        let mut gp_carry_cum = 0.0;
        let currency = events
            .first()
            .ok_or(finstack_core::InputError::TooFewPoints)?
            .amount
            .currency();

        for event in events {
            if event.kind == FundEventKind::Distribution || event.kind == FundEventKind::Proceeds {
                // Run waterfall allocation
                let lp_distributed_so_far: f64 = ledger_rows.iter().map(|r| r.to_lp.amount()).sum();
                let allocations = self.allocate_distribution(AllocationParams {
                    total_amount: event.amount,
                    initial_lp_unreturned: lp_unreturned,
                    initial_gp_carry: gp_carry_cum,
                    lp_distributed_cum_before: lp_distributed_so_far,
                    all_events: events,
                    prior_rows: ledger_rows,
                    allocation_date: event.date,
                    currency,
                })?;

                for alloc in allocations {
                    lp_unreturned = alloc.lp_unreturned.amount();
                    gp_carry_cum = alloc.gp_carry_cum.amount();
                    ledger_rows.push(alloc);
                }
            } else if event.kind == FundEventKind::Contribution {
                // Track LP capital
                lp_unreturned += event.amount.amount();
            }
        }

        Ok(())
    }

    /// Run American-style waterfall (deal-by-deal).
    fn run_american(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
    ) -> finstack_core::Result<()> {
        // Group events by deal_id
        let mut deals: HashMap<String, Vec<&FundEvent>> = HashMap::default();
        let mut fund_contributions = Vec::new();

        for event in events {
            match (&event.deal_id, event.kind) {
                (Some(deal_id), FundEventKind::Proceeds) => {
                    deals.entry(deal_id.to_owned()).or_default().push(event);
                }
                (_, FundEventKind::Contribution) => {
                    fund_contributions.push(event);
                }
                _ => {
                    // Fund-level distributions go to fund level
                    fund_contributions.push(event);
                }
            }
        }

        // Process each deal separately
        let mut total_lp_unreturned = fund_contributions
            .iter()
            .filter(|e| e.kind == FundEventKind::Contribution)
            .map(|e| e.amount.amount())
            .sum::<f64>();

        let mut total_gp_carry = 0.0;
        let currency = events
            .first()
            .ok_or(finstack_core::InputError::TooFewPoints)?
            .amount
            .currency();

        // Iterate deals in a deterministic, economically meaningful order: by each
        // deal's earliest event date, then `deal_id` as a stable tie-break. HashMap
        // iteration order is hash-arbitrary; because LP/GP balances are threaded
        // across deals sequentially, the processing order changes the allocations,
        // so we must not depend on it.
        let mut ordered_deals: Vec<(&String, &Vec<&FundEvent>)> = deals.iter().collect();
        ordered_deals.sort_by(|a, b| {
            let a_first = a.1.first().map(|e| e.date);
            let b_first = b.1.first().map(|e| e.date);
            a_first.cmp(&b_first).then_with(|| a.0.cmp(b.0))
        });

        for (deal_id, deal_events) in ordered_deals {
            // For each deal, allocate proceeds through the waterfall
            for event in deal_events {
                let lp_distributed_so_far: f64 = ledger_rows.iter().map(|r| r.to_lp.amount()).sum();
                let allocations = self.allocate_distribution(AllocationParams {
                    total_amount: event.amount,
                    initial_lp_unreturned: total_lp_unreturned,
                    initial_gp_carry: total_gp_carry,
                    lp_distributed_cum_before: lp_distributed_so_far,
                    all_events: events,
                    prior_rows: ledger_rows,
                    allocation_date: event.date,
                    currency,
                })?;

                for mut alloc in allocations {
                    alloc.deal_id = Some(Arc::from(deal_id.clone()));
                    total_lp_unreturned = alloc.lp_unreturned.amount();
                    total_gp_carry = alloc.gp_carry_cum.amount();
                    ledger_rows.push(alloc);
                }
            }
        }

        Ok(())
    }

    /// Allocate a single distribution through the waterfall.
    fn allocate_distribution(
        &self,
        params: AllocationParams,
    ) -> finstack_core::Result<Vec<AllocationRow>> {
        let mut remaining_amount = params.total_amount.amount();
        let mut lp_unreturned = params.initial_lp_unreturned;
        let mut gp_carry_cum = params.initial_gp_carry;
        let mut allocations = Vec::new();

        // Track LP allocated within this distribution call (prior tranches)
        let mut lp_allocated_in_call_so_far: f64 = 0.0;

        // LP-net cashflow history (contributions + ledger to_lp rows) used by
        // every IRR-based solve in this call . Same-date
        // allocations from earlier tranches in this call are handled via
        // `lp_allocated_in_call_so_far`.
        let lp_history =
            self.lp_net_history(params.all_events, params.prior_rows, params.allocation_date);

        // Precompute holdback percent (0.0 if none or clawback disabled)
        let holdback_pct: f64 = (match &self.spec.clawback {
            Some(c) if c.enable => c.holdback_pct.unwrap_or(0.0),
            _ => 0.0,
        })
        .clamp(0.0, 1.0);

        for (idx, tranche) in self.spec.tranches.iter().enumerate() {
            if remaining_amount <= 1e-6 {
                break;
            }

            let (to_lp, to_gp_paid, tranche_name, gp_carry_cum_after) = match tranche {
                Tranche::ReturnOfCapital => {
                    let allocation = remaining_amount.min(lp_unreturned);
                    lp_unreturned -= allocation;
                    remaining_amount -= allocation;
                    lp_allocated_in_call_so_far += allocation;
                    (
                        allocation,
                        0.0,
                        Arc::from("Return of Capital"),
                        gp_carry_cum,
                    )
                }

                Tranche::PreferredIrr { irr } => {
                    // `calculate_preferred_amount` returns the total LP amount needed at
                    // `allocation_date` to reach `target_irr`, based on the LP-net
                    // ledger history.  Earlier tranches within this same distribution
                    // (captured in `lp_allocated_in_call_so_far`) are invisible to the
                    // IRR solve because they haven't been persisted to the ledger yet.
                    // We subtract that same-date offset so the preferred tier only awards
                    // the incremental cash still needed on top of what the LP already
                    // received from earlier tranches in this distribution call.
                    let gross_required =
                        self.calculate_preferred_amount(*irr, &lp_history, params.allocation_date)?;
                    let required = (gross_required - lp_allocated_in_call_so_far).max(0.0);
                    let allocation = remaining_amount.min(required);
                    remaining_amount -= allocation;
                    lp_allocated_in_call_so_far += allocation;
                    (
                        allocation,
                        0.0,
                        format!("Preferred Return {:.1}%", irr * 100.0).into(),
                        gp_carry_cum,
                    )
                }

                Tranche::CatchUp { gp_share } => {
                    // Determine target cumulative GP share from the next promote tier if available
                    let mut target_gp_share: f64 = *gp_share; // fallback
                    for next in self.spec.tranches.iter().skip(idx + 1) {
                        if let Tranche::PromoteTier { gp_share, .. } = next {
                            target_gp_share = *gp_share;
                            break;
                        }
                    }

                    // Compute contributions up to current date
                    let total_contributions_to_date: f64 = params
                        .all_events
                        .iter()
                        .filter(|e| {
                            e.kind == FundEventKind::Contribution
                                && e.date <= params.allocation_date
                        })
                        .map(|e| e.amount.amount())
                        .sum();

                    // Profit to date before this catch-up tranche (gross basis)
                    let mut profit_excl = (params.lp_distributed_cum_before
                        + lp_allocated_in_call_so_far
                        + gp_carry_cum)
                        - total_contributions_to_date;
                    if !profit_excl.is_finite() || profit_excl.is_sign_negative() {
                        profit_excl = 0.0;
                    }

                    let needed_gp_gross = if target_gp_share >= 1.0 - 1e-12 {
                        remaining_amount // degenerate; give remaining in full-mode
                    } else {
                        ((target_gp_share * profit_excl) - gp_carry_cum) / (1.0 - target_gp_share)
                    };

                    let needed_gp_gross = needed_gp_gross.max(0.0);
                    let to_gp_gross = match self.spec.catchup_mode {
                        CatchUpMode::Full => needed_gp_gross.min(remaining_amount),
                        CatchUpMode::Partial => (remaining_amount * gp_share).min(needed_gp_gross),
                    };

                    // Holdback flow: `gp_carry_cum` and `remaining_amount`
                    // both move by the GROSS carry, but only `to_gp_paid`
                    // (post-holdback) is recorded on the ledger row. The
                    // held-back portion (`to_gp_gross - to_gp_paid`) is not
                    // an on-balance line item here; it is implicitly released
                    // to GP at periodic clawback settlement, where
                    // `apply_clawback` reconciles via
                    //   delta_gp = allowed_gp_total(as_of) − Σ ledger.to_gp
                    // (allowed = from-scratch no-holdback waterfall replay)
                    // and pays the difference (positive δ → GP receives the
                    // accumulated holdback; negative δ → clawback from GP).
                    let to_gp_paid = to_gp_gross * (1.0 - holdback_pct);
                    gp_carry_cum += to_gp_gross;
                    remaining_amount -= to_gp_gross;
                    (0.0, to_gp_paid, Arc::from("Catch-Up"), gp_carry_cum)
                }

                Tranche::PromoteTier {
                    lp_share,
                    gp_share,
                    hurdle,
                } => {
                    // Gate the promote split on the LP actually reaching the
                    // tier's hurdle IRR : pay the LP at
                    // 100% until the hurdle is met, then split at
                    // lp_share/gp_share, cascading to the next tier once the
                    // next hurdle is reached.
                    let Hurdle::Irr { rate: hurdle_rate } = hurdle;

                    // 1) 100% to LP until this tier's hurdle IRR is met.
                    let gross_required = self.calculate_preferred_amount(
                        *hurdle_rate,
                        &lp_history,
                        params.allocation_date,
                    )?;
                    let pre_split = (gross_required - lp_allocated_in_call_so_far)
                        .max(0.0)
                        .min(remaining_amount);
                    remaining_amount -= pre_split;
                    lp_allocated_in_call_so_far += pre_split;

                    // 2) Split at lp/gp shares, capped so the LP does not get
                    //    pushed past the *next* promote tier's hurdle — any
                    //    excess cascades to that tier.
                    let next_hurdle_rate =
                        self.spec
                            .tranches
                            .iter()
                            .skip(idx + 1)
                            .find_map(|t| match t {
                                Tranche::PromoteTier {
                                    hurdle: Hurdle::Irr { rate },
                                    ..
                                } => Some(*rate),
                                _ => None,
                            });
                    let split_amount = match next_hurdle_rate {
                        Some(next_rate) if *lp_share > 1e-12 => {
                            let next_required = self.calculate_preferred_amount(
                                next_rate,
                                &lp_history,
                                params.allocation_date,
                            )?;
                            let lp_needed = (next_required - lp_allocated_in_call_so_far).max(0.0);
                            (lp_needed / lp_share).min(remaining_amount)
                        }
                        _ => remaining_amount,
                    };

                    let to_lp_split = split_amount * lp_share;
                    let to_gp_gross = split_amount * gp_share;
                    let to_gp_paid = to_gp_gross * (1.0 - holdback_pct);
                    gp_carry_cum += to_gp_gross;
                    remaining_amount -= split_amount;
                    lp_allocated_in_call_so_far += to_lp_split;

                    let tranche_name: Arc<str> = format!(
                        "Promote {:.1}%+ ({}%/{}%)",
                        hurdle_rate * 100.0,
                        lp_share * 100.0,
                        gp_share * 100.0
                    )
                    .into();

                    (
                        pre_split + to_lp_split,
                        to_gp_paid,
                        tranche_name,
                        gp_carry_cum,
                    )
                }
            };

            // Calculate current LP IRR from the LP-net ledger history plus
            // what this call has allocated to the LP so far .
            let lp_irr_to_date = {
                let mut flows = lp_history.clone();
                if lp_allocated_in_call_so_far > 1e-9 {
                    flows.push((
                        params.allocation_date,
                        Money::new(lp_allocated_in_call_so_far, params.currency),
                    ));
                }
                self.calculate_lp_irr_to_date(&flows)
            };

            allocations.push(AllocationRow {
                date: params.allocation_date,
                period_key: self.period_key_for(params.allocation_date).map(Arc::from),
                deal_id: None, // Set by caller for American style
                tranche: tranche_name,
                to_lp: Money::new(to_lp, params.currency),
                to_gp: Money::new(to_gp_paid, params.currency),
                lp_unreturned: Money::new(lp_unreturned, params.currency),
                gp_carry_cum: Money::new(gp_carry_cum_after, params.currency),
                lp_irr_to_date,
                note: None,
            });
        }

        Ok(allocations)
    }

    /// Build the LP-net cashflow history from contributions and persisted
    /// ledger `to_lp` rows .
    ///
    /// Gross fund events overstate the LP's receipts by the GP carry, which
    /// inflates the LP IRR and deflates preferred-return entitlements.
    /// Contributions strictly before `allocation_date` enter as negative
    /// flows; persisted `to_lp` allocations up to and including
    /// `allocation_date` enter as positive flows. Allocations from earlier
    /// tranches within the *current* distribution call are not yet persisted
    /// and are handled by the caller via `lp_allocated_in_call_so_far`.
    fn lp_net_history(
        &self,
        all_events: &[FundEvent],
        prior_rows: &[AllocationRow],
        allocation_date: Date,
    ) -> Vec<(Date, Money)> {
        let mut flows: Vec<(Date, Money)> = Vec::new();
        for event in all_events {
            if event.kind == FundEventKind::Contribution && event.date < allocation_date {
                flows.push((
                    event.date,
                    Money::new(-event.amount.amount(), event.amount.currency()),
                ));
            }
        }
        for row in prior_rows {
            if row.date <= allocation_date && row.to_lp.amount().abs() > 1e-9 {
                flows.push((row.date, row.to_lp));
            }
        }
        flows.sort_by_key(|(d, _)| *d);
        flows
    }

    /// Calculate the amount needed for preferred return using robust root finding.
    ///
    /// Returns the total LP amount required at `current_date` to achieve `target_irr`
    /// given the LP-net cashflow history `lp_flows` (see
    /// [`lp_net_history`](Self::lp_net_history)), **not yet accounting for**
    /// same-date allocations from earlier tranches in the current distribution
    /// call.  The call site subtracts that offset before capping against
    /// `remaining_amount`, so the tier only allocates the incremental cash
    /// still needed above what earlier tranches have already paid to the LP.
    fn calculate_preferred_amount(
        &self,
        target_irr: f64,
        lp_flows: &[(Date, Money)],
        current_date: Date,
    ) -> finstack_core::Result<f64> {
        if lp_flows.is_empty() {
            return Ok(0.0); // Need at least one prior flow (contribution)
        }

        // Current IRR without additional preferred return. An IRR-to-date can
        // be legitimately *undefined* early in a fund's life — a single
        // contribution (too few flows) or an all-negative history (no NPV
        // sign change) — which is economically "below any hurdle", so the
        // tier proceeds to solve for the required amount. Genuine input
        // errors (e.g. day-count failures) propagate instead of being
        // silently treated as a 0% IRR.
        let base_date = lp_flows[0].0;
        let current_irr = match self.calculate_irr(lp_flows, base_date) {
            Ok(irr) => irr,
            Err(finstack_core::Error::Input(finstack_core::InputError::TooFewPoints))
            | Err(finstack_core::Error::Validation(_)) => f64::NEG_INFINITY,
            Err(e) => return Err(e),
        };

        if current_irr >= target_irr {
            return Ok(0.0); // Already at or above target IRR
        }

        // Large finite penalty for invalid candidate amounts inside the Brent
        // objective. `f64::INFINITY` poisons Brent's secant/inverse-quadratic
        // interpolation steps (inf - inf = NaN); a large finite value keeps
        // the bracket logic working while still repelling the solver.
        const IRR_OBJECTIVE_PENALTY: f64 = 1.0e6;

        // Use root finding to determine required additional distribution amount
        let target_function = |additional_amount: f64| -> f64 {
            if additional_amount < 0.0 {
                return IRR_OBJECTIVE_PENALTY;
            }

            let mut flows_with_additional = lp_flows.to_vec();
            flows_with_additional.push((
                current_date,
                Money::new(additional_amount, lp_flows[0].1.currency()),
            ));

            match self.calculate_irr(&flows_with_additional, base_date) {
                Ok(irr) => irr - target_irr,
                Err(_) => IRR_OBJECTIVE_PENALTY, // Invalid IRR
            }
        };

        let total_contributions: f64 = lp_flows
            .iter()
            .filter(|(_, amount)| amount.amount() < 0.0)
            .map(|(_, amount)| amount.amount().abs())
            .sum();

        // f(0) = current_irr - target_irr < 0 (checked above) and f is
        // increasing in the additional amount, so expand the upper bound
        // until the target IRR is bracketed and solve in-bracket. (A plain
        // guess-based solve fails for large funds: the guess can exceed the
        // solver's default bracket-search bounds.)
        let mut hi = total_contributions.max(1.0);
        let mut f_hi = target_function(hi);
        for _ in 0..60 {
            if !f_hi.is_finite() || f_hi >= 0.0 {
                break;
            }
            hi *= 2.0;
            f_hi = target_function(hi);
        }

        let solver = BrentSolver::new().tolerance(1e-6);
        let bracketed = if f_hi.is_finite() && f_hi >= 0.0 {
            solver.solve_in_bracket(target_function, 0.0, hi)
        } else {
            Err(finstack_core::InputError::Invalid.into())
        };
        match bracketed {
            Ok(amount) => Ok(amount.max(0.0)),
            Err(_) => {
                // If root finding fails, try to estimate analytically
                // For a simple case: if we have one contribution and want target IRR over time t,
                // then: target_amount = contribution * (1 + target_irr)^t
                if lp_flows.len() == 1 {
                    let contrib_amount = lp_flows[0].1.amount().abs();
                    // Day-count failures are real input errors; propagate
                    // instead of silently assuming a 1-year horizon.
                    let years = self.spec.irr_basis.year_fraction(
                        base_date,
                        current_date,
                        finstack_core::dates::DayCountContext::default(),
                    )?;
                    let required_total = contrib_amount * (1.0 + target_irr).powf(years);
                    let already_received = total_contributions - contrib_amount; // Net distributions so far
                    Ok((required_total - already_received).max(0.0))
                } else {
                    Err(finstack_core::Error::Calibration {
                        message: "Failed to solve for preferred return amount".into(),
                        category: "Waterfall".into(),
                    })
                }
            }
        }
    }

    /// Calculate IRR for LP cashflows.
    ///
    /// Delegates to the standalone [`super::metrics::calculate_irr`] routine
    /// (same `(1 + r)^{-t}` discounting, day-count error propagation, and
    /// multiple-root guard) using the waterfall's `irr_basis`.
    ///
    /// `base_date` must be the date of the first flow — the shared routine
    /// anchors year fractions there.
    fn calculate_irr(
        &self,
        flows: &[(Date, Money)],
        base_date: Date,
    ) -> finstack_core::Result<f64> {
        debug_assert!(
            flows.first().is_none_or(|(d, _)| *d == base_date),
            "waterfall IRR base_date must be the first flow's date"
        );
        let _ = base_date;
        super::metrics::calculate_irr(flows, self.spec.irr_basis)
    }

    /// Calculate LP IRR to date from an LP-net cashflow history
    /// .
    fn calculate_lp_irr_to_date(&self, lp_flows: &[(Date, Money)]) -> Option<f64> {
        if lp_flows.len() < 2 {
            return None;
        }

        let base_date = lp_flows[0].0;
        self.calculate_irr(lp_flows, base_date).ok()
    }

    /// GP entitlement as of `as_of` under the fund's actual waterfall
    /// economics .
    ///
    /// Replays a from-scratch waterfall over the fund's *lifetime* economics
    /// to `as_of`: every contribution at its actual date (so IRR hurdles see
    /// the true capital timing) plus one lump-sum distribution at `as_of`
    /// equal to all distributions/proceeds to date. The replay runs with
    /// clawback/holdback disabled at fund level (clawback is a fund-level
    /// lifetime test, also for American deal-by-deal funds), and the gross GP
    /// allocations are summed. This respects
    /// [`CatchUpMode::Full`]/[`CatchUpMode::Partial`], the presence or
    /// absence of a [`Tranche::CatchUp`] tranche, and promote-tier hurdles —
    /// i.e. it equals "what a from-scratch waterfall would have paid the
    /// GP", rather than `profit_total × first_promote_tier.gp_share` (which
    /// over-releases to the GP under partial or missing catch-up).
    fn allowed_gp_total_as_of(
        &self,
        events: &[FundEvent],
        as_of: Date,
    ) -> finstack_core::Result<f64> {
        let mut synthetic: Vec<FundEvent> = events
            .iter()
            .filter(|e| e.kind == FundEventKind::Contribution && e.date <= as_of)
            .cloned()
            .collect();

        let total_distributions: f64 = events
            .iter()
            .filter(|e| {
                (e.kind == FundEventKind::Distribution || e.kind == FundEventKind::Proceeds)
                    && e.date <= as_of
            })
            .map(|e| e.amount.amount())
            .sum();
        if total_distributions <= 0.0 {
            return Ok(0.0);
        }

        let Some(currency) = events.first().map(|e| e.amount.currency()) else {
            return Ok(0.0);
        };
        synthetic.push(FundEvent::distribution(
            as_of,
            Money::new(total_distributions, currency),
        ));

        // Clawback disabled in the replay: no holdback (rows carry gross GP
        // amounts) and no recursive settlement rows.
        let mut replay_spec = self.spec.clone();
        replay_spec.clawback = None;
        let replay_engine = EquityWaterfallEngine {
            spec: &replay_spec,
            periods: None,
        };

        let mut replay_rows = Vec::new();
        replay_engine.run_european(&synthetic, &mut replay_rows)?;

        Ok(replay_rows.iter().map(|r| r.to_gp.amount()).sum())
    }

    /// Apply clawback reconciliation.
    fn apply_clawback(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
        clawback_spec: &ClawbackSpec,
    ) -> finstack_core::Result<()> {
        let Some(last_event_date) = events.iter().map(|e| e.date).max() else {
            return Ok(());
        };
        let paid_gp_as_of = |rows: &[AllocationRow], as_of: Date| -> f64 {
            rows.iter()
                .filter(|r| r.date <= as_of)
                .map(|r| r.to_gp.amount())
                .sum()
        };

        if matches!(clawback_spec.settle_on, ClawbackSettle::Periodic) {
            let periods = self
                .periods
                .as_ref()
                .ok_or(finstack_core::InputError::Invalid)?;

            for period in periods {
                let settlement_date = period.end - Duration::days(1);
                // Skip periods that start after the last event
                if period.start > last_event_date {
                    continue;
                }

                // Skip if no events have occurred by the settlement date
                if events.iter().all(|e| e.date > settlement_date) {
                    continue;
                }

                let allowed_gp_total = self.allowed_gp_total_as_of(events, settlement_date)?;
                let paid_gp_total = paid_gp_as_of(ledger_rows, settlement_date);

                let delta_gp: f64 = allowed_gp_total - paid_gp_total;
                if delta_gp.abs() <= 1e-9 {
                    continue;
                }

                let last_row = ledger_rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.date <= settlement_date)
                    .max_by(|(idx_a, a), (idx_b, b)| match a.date.cmp(&b.date) {
                        Ordering::Equal => idx_a.cmp(idx_b),
                        other => other,
                    })
                    .map(|(_, row)| row.clone());

                let Some(last_row) = last_row else {
                    continue;
                };

                let currency = last_row.to_gp.currency();
                let to_gp = Money::new(delta_gp, currency);
                let to_lp = Money::new((-delta_gp).max(0.0), currency);

                let settlement_row = AllocationRow {
                    date: settlement_date,
                    period_key: self.period_key_for(settlement_date).map(Arc::from),
                    deal_id: None,
                    tranche: Arc::from("Clawback Settlement (periodic)"),
                    to_lp,
                    to_gp,
                    lp_unreturned: last_row.lp_unreturned,
                    gp_carry_cum: Money::new(allowed_gp_total, currency),
                    lp_irr_to_date: {
                        let flows = self.lp_net_history(events, ledger_rows, settlement_date);
                        self.calculate_lp_irr_to_date(&flows)
                    },
                    note: Some(Arc::from("Clawback settlement and holdback release")),
                };

                ledger_rows.push(settlement_row);
            }

            return Ok(());
        }

        let last_row = match ledger_rows.last() {
            Some(r) => r.clone(),
            None => return Ok(()),
        };

        let settlement_date = last_event_date;
        let allowed_gp_total = self.allowed_gp_total_as_of(events, settlement_date)?;
        let paid_gp_total = paid_gp_as_of(ledger_rows, settlement_date);
        let delta_gp: f64 = allowed_gp_total - paid_gp_total;

        if delta_gp.abs() <= 1e-9 {
            return Ok(());
        }

        let currency = last_row.to_gp.currency();
        let to_gp = Money::new(delta_gp, currency);
        let to_lp = Money::new((-delta_gp).max(0.0), currency);

        let settlement_row = AllocationRow {
            date: settlement_date,
            period_key: self.period_key_for(settlement_date).map(Arc::from),
            deal_id: None,
            tranche: Arc::from("Clawback Settlement (fund_end)"),
            to_lp,
            to_gp,
            lp_unreturned: last_row.lp_unreturned,
            gp_carry_cum: Money::new(allowed_gp_total, currency),
            lp_irr_to_date: {
                let flows = self.lp_net_history(events, ledger_rows, settlement_date);
                self.calculate_lp_irr_to_date(&flows)
            },
            note: Some(Arc::from("Clawback settlement and holdback release")),
        };

        ledger_rows.push(settlement_row);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;
    use time::Month;

    fn test_currency() -> Currency {
        Currency::USD
    }

    fn test_date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("Valid date"), day)
            .expect("Valid date")
    }

    #[test]
    fn waterfall_spec_builder() {
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .catchup(1.0)
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("Operation succeeded");

        assert_eq!(spec.style, WaterfallStyle::European);
        assert_eq!(spec.tranches.len(), 4);
        assert_eq!(spec.irr_basis, DayCount::Act365F);
    }

    #[test]
    fn fund_event_creation() {
        let contrib = FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::new(1000000.0, test_currency()),
        );

        assert_eq!(contrib.kind, FundEventKind::Contribution);
        assert_eq!(contrib.signed_amount().amount(), -1000000.0);

        let distrib = FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::new(1500000.0, test_currency()),
        );

        assert_eq!(distrib.kind, FundEventKind::Distribution);
        assert_eq!(distrib.signed_amount().amount(), 1500000.0);
    }

    #[test]
    fn simple_waterfall_allocation() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2) // Simple 80/20 split
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::new(1500000.0, test_currency()),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Operation succeeded");

        assert!(!ledger.rows.is_empty());

        // Should have return of capital and promote allocation
        let roc_rows: Vec<_> = ledger
            .rows
            .iter()
            .filter(|r| r.tranche.contains("Return of Capital"))
            .collect();
        assert!(!roc_rows.is_empty());

        // LP should get back their $1M capital first
        let total_lp_roc: f64 = roc_rows.iter().map(|r| r.to_lp.amount()).sum();
        assert!((total_lp_roc - 1000000.0).abs() < 1e-6);
    }

    #[test]
    fn mixed_currency_events_rejected() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("spec builds");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(1_000_000.0, Currency::USD),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::new(1_500_000.0, Currency::EUR),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let err = engine
            .run(&events)
            .expect_err("mixed-currency events must be rejected");
        assert!(matches!(err, finstack_core::Error::CurrencyMismatch { .. }));
    }

    #[test]
    fn american_waterfall_processes_deals_chronologically() {
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::American)
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("spec builds");

        // The chronological order of the two deals is the REVERSE of their deal_id
        // order, so this only passes if deals are processed by earliest date rather
        // than by id or hash bucket.
        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(2_000_000.0, test_currency()),
            ),
            FundEvent::proceeds(
                test_date(2022, 1, 1),
                Money::new(1_000_000.0, test_currency()),
                "A_late",
            ),
            FundEvent::proceeds(
                test_date(2021, 1, 1),
                Money::new(1_000_000.0, test_currency()),
                "Z_early",
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("runs");

        let first_deal = ledger
            .rows
            .iter()
            .find_map(|r| r.deal_id.as_deref())
            .expect("at least one deal-tagged row");
        assert_eq!(first_deal, "Z_early");
    }

    #[test]
    fn american_waterfall_invariant_to_input_order() {
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::American)
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("spec builds");

        let mut events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(2_000_000.0, test_currency()),
            ),
            FundEvent::proceeds(
                test_date(2022, 1, 1),
                Money::new(1_500_000.0, test_currency()),
                "deal_a",
            ),
            FundEvent::proceeds(
                test_date(2021, 1, 1),
                Money::new(900_000.0, test_currency()),
                "deal_b",
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger_a = engine.run(&events).expect("runs");
        events.reverse();
        let ledger_b = engine.run(&events).expect("runs");
        assert_eq!(ledger_a.rows, ledger_b.rows);
    }

    #[test]
    fn ledger_to_tabular_conversion() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(
                test_date(2020, 1, 1),
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 1, 1),
                Money::new(1000000.0, test_currency()),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Operation succeeded");
        let (columns, _rows) = ledger.to_tabular_data();

        // Check tabular structure
        assert!(columns.contains(&"date"));
        assert!(columns.contains(&"tranche"));
        assert!(columns.contains(&"to_lp"));
        assert!(columns.contains(&"to_gp"));
    }

    #[test]
    fn validate_waterfall_spec() {
        // Valid spec
        let valid_spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build();
        assert!(valid_spec.is_ok());

        // Invalid spec - promote shares don't sum to 1.0
        let invalid_spec = WaterfallSpec {
            style: WaterfallStyle::European,
            tranches: smallvec![Tranche::PromoteTier {
                hurdle: Hurdle::Irr { rate: 0.0 },
                lp_share: 0.7,
                gp_share: 0.4, // 0.7 + 0.4 = 1.1 > 1.0
            }],
            clawback: None,
            irr_basis: DayCount::Act365F,
            catchup_mode: CatchUpMode::Full,
        };

        assert!(invalid_spec.validate().is_err());
    }

    #[test]
    fn catchup_precise_reaches_target_split() {
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .catchup(1.0)
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2024, 1, 1), Money::new(280.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Operation succeeded");

        let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
        let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
        let profit = (total_lp + total_gp) - 100.0;

        // Ensure profit positive and GP share ~20%
        assert!(profit > 0.0);
        let gp_share = if profit.abs() > 1e-9 {
            total_gp / profit
        } else {
            0.0
        };
        assert!((gp_share - 0.20).abs() < 1e-6, "gp_share={}", gp_share);

        // Catch-up row should exist with positive GP allocation (likely ~9)
        let catchup_rows: Vec<_> = ledger
            .rows
            .iter()
            .filter(|r| r.tranche.contains("Catch-Up") && r.to_gp.amount() > 0.0)
            .collect();
        assert!(!catchup_rows.is_empty());
    }

    /// a promote tier's hurdle must gate the GP split.
    /// Until the LP-net IRR reaches the hurdle, the tier pays the LP at 100%;
    /// only the excess is split at lp/gp shares.
    #[test]
    fn promote_tier_hurdle_gates_gp_split() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.20, 0.8, 0.2)
            .build()
            .expect("spec builds");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000.0, test_currency())),
            FundEvent::distribution(test_date(2021, 1, 1), Money::new(1400.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("runs");

        let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
        let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();

        // LP must first be brought to the 20% hurdle IRR at 100%:
        // entitlement = 1000 · 1.20^t with t = 366/365 (Act/365F, 2020 leap).
        let t = 366.0 / 365.0;
        let hurdle_entitlement = 1000.0 * 1.20_f64.powf(t);
        let expected_gp = 0.2 * (1400.0 - hurdle_entitlement);

        assert!(
            (total_gp - expected_gp).abs() < 0.1,
            "GP carry must only apply above the hurdle entitlement: \
             expected ~{expected_gp}, got {total_gp}"
        );
        assert!(
            total_gp < 0.2 * 400.0 - 1.0,
            "GP carry {total_gp} must be below the ungated 20% of profit (80)"
        );
        assert!((total_lp + total_gp - 1400.0).abs() < 1e-6, "conservation");
    }

    /// when the distribution cannot bring the LP to the
    /// hurdle IRR, the GP receives nothing from the promote tier.
    #[test]
    fn promote_tier_below_hurdle_pays_lp_only() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.20, 0.8, 0.2)
            .build()
            .expect("spec builds");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000.0, test_currency())),
            FundEvent::distribution(test_date(2021, 1, 1), Money::new(1100.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("runs");

        let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();
        let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
        assert!(
            total_gp.abs() < 1e-9,
            "GP must receive nothing below the hurdle, got {total_gp}"
        );
        assert!((total_lp - 1100.0).abs() < 1e-6, "LP receives everything");
    }

    /// multiple promote tiers cascade — each tier's split
    /// applies only between its own hurdle and the next tier's hurdle.
    #[test]
    fn promote_tiers_cascade_at_next_hurdle() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.08, 0.8, 0.2)
            .promote_tier(0.20, 0.7, 0.3)
            .build()
            .expect("spec builds");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000.0, test_currency())),
            FundEvent::distribution(test_date(2021, 1, 1), Money::new(2000.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("runs");

        let t = 366.0 / 365.0;
        let entitlement_8 = 1000.0 * 1.08_f64.powf(t);
        let entitlement_20 = 1000.0 * 1.20_f64.powf(t);

        // Tier 1 (80/20) covers only the band where the LP moves from the 8%
        // to the 20% entitlement: distribution band = (E20 − E8)/0.8.
        let tier1_band = (entitlement_20 - entitlement_8) / 0.8;
        let expected_gp_tier1 = 0.2 * tier1_band;
        // Tier 2 (70/30) takes everything above.
        let remaining_after = 2000.0 - 1000.0 - (entitlement_8 - 1000.0) - tier1_band;
        let expected_gp_tier2 = 0.3 * remaining_after;

        let gp_tier1: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.tranche.contains("Promote 8.0%"))
            .map(|r| r.to_gp.amount())
            .sum();
        let gp_tier2: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.tranche.contains("Promote 20.0%"))
            .map(|r| r.to_gp.amount())
            .sum();

        assert!(
            (gp_tier1 - expected_gp_tier1).abs() < 0.1,
            "tier-1 GP should be ~{expected_gp_tier1}, got {gp_tier1}"
        );
        assert!(
            (gp_tier2 - expected_gp_tier2).abs() < 0.1,
            "tier-2 GP should be ~{expected_gp_tier2}, got {gp_tier2}"
        );

        let total: f64 = ledger
            .rows
            .iter()
            .map(|r| r.to_lp.amount() + r.to_gp.amount())
            .sum();
        assert!((total - 2000.0).abs() < 1e-6, "conservation");
    }

    /// the ledger's `lp_irr_to_date` must be computed
    /// from the LP-net history (contributions + ledger `to_lp`), not from
    /// gross fund events that include GP carry.
    #[test]
    fn lp_irr_to_date_uses_ledger_net_flows() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("spec builds");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000.0, test_currency())),
            FundEvent::distribution(test_date(2021, 1, 1), Money::new(1500.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("runs");

        // LP receives 1000 (RoC) + 0.8·500 = 1400 net of GP carry.
        let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
        assert!((total_lp - 1400.0).abs() < 1e-6);

        let t = 366.0 / 365.0;
        let expected_net_irr = 1.4_f64.powf(1.0 / t) - 1.0;
        let gross_irr = 1.5_f64.powf(1.0 / t) - 1.0;

        let last_irr = ledger
            .rows
            .last()
            .and_then(|r| r.lp_irr_to_date)
            .expect("last row has an LP IRR");
        assert!(
            (last_irr - expected_net_irr).abs() < 1e-3,
            "lp_irr_to_date {last_irr} should match the LP-net IRR {expected_net_irr}"
        );
        assert!(
            last_irr < gross_irr - 0.05,
            "lp_irr_to_date {last_irr} must sit well below the gross-event IRR {gross_irr}"
        );
    }

    #[test]
    fn clawback_fund_end_overdistribution() {
        let claw = ClawbackSpec {
            enable: true,
            holdback_pct: None,
            settle_on: ClawbackSettle::FundEnd,
        };

        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .clawback(claw)
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2022, 1, 1), Money::new(150.0, test_currency())),
            FundEvent::contribution(test_date(2023, 1, 1), Money::new(90.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Operation succeeded");

        // Last row should be clawback settlement with negative GP amount
        let last = ledger.rows.last().expect("rows");
        assert!(last.tranche.contains("Clawback Settlement"));
        assert!(last.to_gp.amount() < 0.0);
        // In this scenario, first promote paid GP 10; final profit is 0 so GP should return ~10
        assert!((last.to_gp.amount() + 10.0).abs() < 1e-6);
    }

    #[test]
    fn clawback_no_catchup_does_not_over_release() {
        // No catch-up tranche: GP entitlement on lifetime profit is the
        // promote share of profit *above the preferred return*, not
        // profit × gp_share. The old formula released 10 (= 50 × 0.2); the
        // catch-up-aware replay releases only (50 − pref) × 0.2.
        let claw = ClawbackSpec {
            enable: true,
            holdback_pct: Some(1.0), // all carry held back until settlement
            settle_on: ClawbackSettle::FundEnd,
        };

        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .promote_tier(0.0, 0.8, 0.2)
            .clawback(claw)
            .build()
            .expect("spec");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2022, 1, 1), Money::new(150.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("run");

        let settlement = ledger
            .rows
            .iter()
            .find(|r| r.tranche.contains("Clawback Settlement"))
            .expect("settlement row");

        // Act365F tenor 2020-01-01 → 2022-01-01 is 731/365 years.
        let t = 731.0 / 365.0;
        let pref = 100.0 * (1.08_f64).powf(t) - 100.0;
        let expected_gp = (50.0 - pref) * 0.2;
        assert!(
            (settlement.to_gp.amount() - expected_gp).abs() < 1e-3,
            "settlement {} should equal promote share above pref {expected_gp}",
            settlement.to_gp.amount()
        );
        assert!(
            settlement.to_gp.amount() < 10.0 - 1e-6,
            "must not over-release to the GP at profit × gp_share (10): {}",
            settlement.to_gp.amount()
        );
    }

    #[test]
    fn clawback_partial_catchup_does_not_over_release() {
        // Partial catch-up caps the GP's catch-up take at gp_share of each
        // dollar, so lifetime GP entitlement is below the full-catch-up
        // profit × promote-share figure the old settlement formula used.
        let claw = ClawbackSpec {
            enable: true,
            holdback_pct: Some(1.0),
            settle_on: ClawbackSettle::FundEnd,
        };

        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .catchup(0.5)
            .promote_tier(0.0, 0.8, 0.2)
            .catchup_mode(CatchUpMode::Partial)
            .clawback(claw)
            .build()
            .expect("spec");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2022, 1, 1), Money::new(120.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("run");

        let settlement = ledger
            .rows
            .iter()
            .find(|r| r.tranche.contains("Clawback Settlement"))
            .expect("settlement row");

        // Replay economics: RoC 100 → pref P = 100·1.08^t − 100 → catch-up
        // takes min(0.5 × remaining, needed) → promote splits the rest 80/20.
        let t = 731.0 / 365.0;
        let pref = 100.0 * (1.08_f64).powf(t) - 100.0;
        let remaining = 20.0 - pref;
        let needed = 0.25 * pref; // (0.2·P)/(1 − 0.2)
        let catchup_gp = (0.5 * remaining).min(needed);
        let promote_gp = (remaining - catchup_gp) * 0.2;
        let expected_gp = catchup_gp + promote_gp;
        assert!(
            (settlement.to_gp.amount() - expected_gp).abs() < 1e-3,
            "settlement {} should equal partial-catch-up entitlement {expected_gp}",
            settlement.to_gp.amount()
        );
        // Old formula: profit × first promote gp_share = 20 × 0.2 = 4.0.
        assert!(
            settlement.to_gp.amount() < 4.0 - 1e-6,
            "must not over-release at full-catch-up economics (4.0): {}",
            settlement.to_gp.amount()
        );
    }

    #[test]
    fn period_support_quarterly() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(
                test_date(2024, 11, 1), // 2024Q4
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 2, 15), // 2025Q1
                Money::new(1200000.0, test_currency()),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec)
            .with_period_range("2024Q4..2025Q2", None)
            .expect("Operation succeeded");

        let ledger = engine.run(&events).expect("Operation succeeded");

        // All distribution allocation rows should have 2025Q1 period key
        let distribution_rows: Vec<_> = ledger
            .rows
            .iter()
            .filter(|r| r.date == test_date(2025, 2, 15))
            .collect();

        assert!(!distribution_rows.is_empty());
        for row in distribution_rows {
            assert_eq!(row.period_key, Some(Arc::from("2025Q1")));
        }
    }

    #[test]
    fn period_support_outside_range() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(
                test_date(2024, 1, 1), // Outside period range
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2026, 1, 1), // Outside period range
                Money::new(1000000.0, test_currency()),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec)
            .with_period_range("2025Q1..Q4", None)
            .expect("Operation succeeded");

        let ledger = engine.run(&events).expect("Operation succeeded");

        // All rows should have None period_key since dates are outside range
        for row in &ledger.rows {
            assert_eq!(row.period_key, None);
        }
    }

    #[test]
    fn period_support_clawback_settlement() {
        let claw = ClawbackSpec {
            enable: true,
            holdback_pct: Some(0.1),
            settle_on: ClawbackSettle::FundEnd,
        };

        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .clawback(claw)
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(test_date(2025, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(
                test_date(2025, 6, 15), // 2025Q2
                Money::new(150.0, test_currency()),
            ),
        ];

        let engine = EquityWaterfallEngine::new(&spec)
            .with_period_range("2025Q1..Q4", None)
            .expect("Operation succeeded");

        let ledger = engine.run(&events).expect("Operation succeeded");

        // Find clawback settlement row
        let clawback_row = ledger
            .rows
            .iter()
            .find(|r| r.tranche.contains("Clawback Settlement"))
            .expect("Should have clawback settlement");

        // Settlement date is the last event date (2025-06-15), which is in Q2
        assert_eq!(clawback_row.period_key, Some(Arc::from("2025Q2")));
    }

    #[test]
    fn periodic_clawback_overdistribution() {
        let claw = ClawbackSpec {
            enable: true,
            holdback_pct: None,
            settle_on: ClawbackSettle::Periodic,
        };

        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .promote_tier(0.0, 0.8, 0.2)
            .clawback(claw)
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(test_date(2024, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2024, 4, 15), Money::new(150.0, test_currency())),
            FundEvent::contribution(test_date(2024, 5, 15), Money::new(90.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec)
            .with_period_range("2024Q1..Q3", None)
            .expect("Operation succeeded");

        let ledger = engine.run(&events).expect("Operation succeeded");

        let clawback_row = ledger
            .rows
            .iter()
            .find(|r| r.tranche.contains("Clawback Settlement (periodic)"))
            .expect("Should have periodic clawback settlement");

        assert_eq!(clawback_row.period_key, Some(Arc::from("2024Q2")));
        assert!(clawback_row.to_gp.amount() < 0.0);
        assert!((clawback_row.to_gp.amount() + 10.0).abs() < 1e-6);
    }

    #[test]
    fn period_support_no_periods_configured() {
        let spec = WaterfallSpec::builder()
            .return_of_capital()
            .build()
            .expect("Operation succeeded");

        let events = vec![
            FundEvent::contribution(
                test_date(2025, 1, 1),
                Money::new(1000000.0, test_currency()),
            ),
            FundEvent::distribution(
                test_date(2025, 6, 15),
                Money::new(1000000.0, test_currency()),
            ),
        ];

        // Engine without periods
        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Operation succeeded");

        // All rows should have None period_key
        for row in &ledger.rows {
            assert_eq!(row.period_key, None);
        }
    }

    /// Regression test: C16 — PreferredIrr tranche must not double-count same-date LP cash
    /// already allocated by earlier tranches (e.g. ReturnOfCapital) within the same distribution.
    ///
    /// Setup: LP contributes $1 000 at t=0. At t=5y the fund returns $1 500.
    /// Tranches: ReturnOfCapital → PreferredIrr(8%) → PromoteTier(80/20).
    ///
    /// RoC tranche allocates $1 000 to LP first.  Remaining $500 goes to preferred.
    /// 8% IRR over 5y on $1 000 requires ~$1 469.33 total to LP.  The LP has already
    /// received $1 000 as RoC, so the preferred tier should award at most ~$469.33 to LP.
    ///
    /// The buggy code omits the $1 000 same-date RoC cash from the IRR solve, making it
    /// think the LP has received $0 so far on this date, so it computes ~$1 469 as the
    /// preferred requirement — far more than the $500 remaining — and allocates the full
    /// $500 to the preferred tier, leaving $0 for the promote tier.
    ///
    /// After the fix the preferred tier is correctly capped and the promote tier receives
    /// its share of the remaining profit.
    #[test]
    fn preferred_irr_does_not_double_count_same_date_roc() {
        let spec = WaterfallSpec::builder()
            .style(WaterfallStyle::European)
            .return_of_capital()
            .preferred_irr(0.08)
            .promote_tier(0.0, 0.8, 0.2)
            .build()
            .expect("Valid spec");

        // LP contributes $1 000; fund distributes $1 500 five years later.
        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000.0, test_currency())),
            FundEvent::distribution(test_date(2025, 1, 1), Money::new(1500.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let ledger = engine.run(&events).expect("Run succeeded");

        let total_lp: f64 = ledger.rows.iter().map(|r| r.to_lp.amount()).sum();
        let total_gp: f64 = ledger.rows.iter().map(|r| r.to_gp.amount()).sum();

        // (1) Cash conservation: LP + GP must equal the $1 500 distribution exactly.
        let total_distributed = total_lp + total_gp;
        assert!(
            (total_distributed - 1500.0).abs() < 1e-4,
            "Cash not conserved: LP={total_lp:.4}, GP={total_gp:.4}, total={total_distributed:.4}"
        );

        // (2) The promote tier must receive some allocation (i.e. the preferred tier must NOT
        //     consume the entire remaining amount after RoC just because it ignores same-date
        //     RoC cash in the IRR solve).  GP gets 20% of profit above hurdle via promote.
        //     Profit = $500.  At 8% IRR over 5y, preferred ~ $469.  Remaining for promote
        //     = ~$31.  GP share ≈ 20% of ~$31 ≈ $6.  We just assert GP > 0.
        assert!(
            total_gp > 1e-4,
            "GP received nothing — preferred tier consumed all remaining cash (double-count bug still present). GP={total_gp:.6}"
        );

        // (3) Preferred tranche allocation must be strictly less than what the buggy code
        //     would compute.  Without the fix the preferred tier gets the full $500 remaining
        //     (since it thinks LP needs ~$1469 but only $500 is left after RoC).
        //     With the fix it gets less (the correct incremental preferred amount above RoC).
        let preferred_lp: f64 = ledger
            .rows
            .iter()
            .filter(|r| r.tranche.contains("Preferred Return"))
            .map(|r| r.to_lp.amount())
            .sum();
        // Buggy value = 500.0 (full remaining).  Correct value < 500.0.
        assert!(
            preferred_lp < 499.99,
            "Preferred tranche consumed full remaining amount — double-count bug likely still present. preferred_lp={preferred_lp:.4}"
        );
    }

    #[test]
    fn solver_failure_propagation() {
        // Use NaN IRR to force solver failure (cannot solve for target)
        let spec = WaterfallSpec::builder()
            .preferred_irr(f64::NAN)
            .build()
            .expect("Valid spec");

        let events = vec![
            FundEvent::contribution(test_date(2020, 1, 1), Money::new(100.0, test_currency())),
            FundEvent::contribution(test_date(2020, 6, 1), Money::new(100.0, test_currency())),
            FundEvent::distribution(test_date(2021, 1, 1), Money::new(300.0, test_currency())),
        ];

        let engine = EquityWaterfallEngine::new(&spec);
        let result = engine.run(&events);

        assert!(result.is_err());
        match result {
            Err(finstack_core::Error::Calibration { message, .. }) => {
                assert_eq!(message, "Failed to solve for preferred return amount");
            }
            _ => panic!("Expected Calibration error, got {:?}", result),
        }
    }
}
