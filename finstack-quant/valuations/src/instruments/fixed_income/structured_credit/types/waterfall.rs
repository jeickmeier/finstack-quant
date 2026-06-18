//! Waterfall type definitions for structured credit instruments.
//!
//! This module contains all data structures for waterfall distribution:
//! - Payment recipients and calculation methods
//! - Tier structures and allocation modes
//! - Coverage triggers for OC/IC tests
//! - Result types for waterfall execution
//!
//! Execution logic is in `crate::instruments::fixed_income::structured_credit::pricing::waterfall`.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::ExplanationTrace;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// CORE TYPES
// ============================================================================

/// Recipient of waterfall payments
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum RecipientType {
    /// Service provider (trustee, admin, rating agency, etc.)
    ServiceProvider(String),
    /// Manager fee (type indicates senior/subordinated/incentive)
    ManagerFee(ManagementFeeType),
    /// Tranche payment
    Tranche(String),
    /// Equity/residual distribution
    Equity,
    /// Reserve account (credit enhancement replenishment)
    ReserveAccount(String),
}

/// Type of management fee
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum ManagementFeeType {
    /// Senior variant.
    Senior,
    /// Subordinated variant.
    Subordinated,
    /// Incentive variant.
    Incentive,
}

/// Rounding convention for payments
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum RoundingConvention {
    /// Round to nearest precision
    #[default]
    Nearest,
    /// Round down (floor)
    Floor,
    /// Round up (ceiling)
    Ceiling,
}

/// How to calculate payment amount
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum PaymentCalculation {
    /// Fixed amount
    FixedAmount {
        /// Amount.
        amount: Money,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Percentage of collateral balance
    PercentageOfCollateral {
        /// Rate.
        rate: f64,
        /// Annualized.
        annualized: bool,
        /// Day count convention for annualization.
        day_count: Option<finstack_quant_core::dates::DayCount>,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Interest due on tranche
    TrancheInterest {
        /// Tranche id.
        tranche_id: String,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Principal payment to tranche
    TranchePrincipal {
        /// Tranche id.
        tranche_id: String,
        /// Target balance.
        target_balance: Option<Money>,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// All remaining cash
    ResidualCash,
    /// Reserve account replenishment (up to target balance).
    ///
    /// The current reserve balance is passed dynamically via `WaterfallContext`
    /// at execution time, not stored here, because the balance changes each period.
    ReserveReplenishment {
        /// Target reserve balance the account should be replenished to.
        target_balance: Money,
    },
    /// Tranche interest with the coupon rate capped (available-funds / net-WAC cap).
    ///
    /// Identical to [`PaymentCalculation::TrancheInterest`] except the effective
    /// annualized coupon is capped at `cap_rate`, modelling an available-funds
    /// cap where a tranche cannot be paid more interest than the collateral's
    /// net weighted-average coupon supports. The capped shortfall is unpaid
    /// (no carryforward in this variant).
    CappedTrancheInterest {
        /// Tranche id.
        tranche_id: String,
        /// Cap on the annualized coupon rate (decimal, e.g. `0.03` = 3%).
        cap_rate: f64,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
}

/// Declarative, additively-applied waterfall rules layered onto a deal's base
/// waterfall.
///
/// Each sub-spec is optional; when none are present the resolved waterfall is
/// identical to the base waterfall. Applied by
/// [`crate::instruments::fixed_income::structured_credit::resolve_waterfall`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WaterfallRules {
    /// Available-funds / net-WAC cap on named tranches' interest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afc: Option<AfcSpec>,
    /// Excess-spread (spread-account) capture and draw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excess_spread: Option<ExcessSpreadSpec>,
    /// Step-down: switch principal allocation to pro-rata after a date when the
    /// step-down trigger passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_down: Option<StepDownSpec>,
    /// Shifting interest: senior receives a scheduled (declining) share of
    /// principal. Mutually exclusive with `step_down` (both govern principal
    /// allocation); `shifting_interest` takes precedence when both are set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifting_interest: Option<ShiftingInterestSpec>,
    /// Early amortization: end a revolving period early on a performance breach
    /// (master-trust style), switching the deal into amortization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_amortization: Option<EarlyAmortizationSpec>,
}

/// Available-funds cap (net-WAC cap) specification.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AfcSpec {
    /// Ids of tranches whose interest coupon is capped at the collateral's
    /// weighted-average coupon.
    pub capped_tranches: Vec<String>,
}

/// Excess-spread / spread-account specification.
///
/// Each period the account captures residual interest (that would otherwise be
/// distributed to equity) up to `target_balance`, and draws down to cover debt
/// tranche interest shortfalls — providing credit enhancement from excess
/// spread. Any balance unused at deal end is released back to equity, unless a
/// cumulative-loss trap trigger is breached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExcessSpreadSpec {
    /// Target funded balance of the spread account (currency units).
    pub target_balance: Money,
    /// Optional cumulative-loss fraction (decimal, e.g. `0.05` = 5% of the
    /// original pool) at or above which any remaining spread-account balance is
    /// *retained* in the deal at maturity rather than released to equity. When
    /// `None`, the unused balance is always released to equity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trap_loss_pct: Option<f64>,
}

/// Step-down specification for senior/subordinate principal allocation.
///
/// Principal is paid sequentially (senior first) until the deal seasons past
/// `step_down_date`; from then on, *if* cumulative losses remain below
/// `max_cumulative_loss_pct` (the step-down trigger), principal switches to
/// pro-rata across the debt tranches, releasing subordination to the juniors.
/// While the loss trigger is breached the deal reverts to sequential, so the
/// switch is re-evaluated every period (non-sticky).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StepDownSpec {
    /// Earliest date principal may switch to pro-rata.
    #[schemars(with = "String")]
    pub step_down_date: Date,
    /// Cumulative-loss fraction (decimal, of the original pool balance) at or
    /// above which the step-down trigger fails and principal stays sequential.
    pub max_cumulative_loss_pct: f64,
}

/// One step of a shifting-interest schedule: the senior's share of principal
/// from `months_from_closing` onward (until the next step).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShiftingInterestStep {
    /// Months from closing at which this senior share takes effect.
    pub months_from_closing: u32,
    /// Senior share of principal (decimal, `1.0` = 100% lockout) from this step.
    pub senior_pct: f64,
}

/// Shifting-interest principal allocation (non-agency senior/sub RMBS).
///
/// The senior tranche receives `senior_pct` of principal (the rest split across
/// the remaining debt tranches), where `senior_pct` follows a declining
/// schedule by deal age. A `1.0` lockout early routes all principal to the
/// senior; later steps release principal to the subordinates. Modelled as a
/// per-period weighted pro-rata of *all* principal (a first-order treatment;
/// agency-style scheduled-vs-prepayment splitting is a refinement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShiftingInterestSpec {
    /// Id of the senior tranche that receives the scheduled principal share.
    pub senior_id: String,
    /// Declining senior-share schedule, ascending by `months_from_closing`.
    pub schedule: Vec<ShiftingInterestStep>,
}

/// Early-amortization specification for revolving (master-trust) deals.
///
/// While a deal's reinvestment/revolving period is active, principal is recycled
/// and the investor (tranche) balances are held flat. If cumulative losses reach
/// `max_cumulative_loss_pct`, an early-amortization event is triggered: the
/// revolving period ends immediately and the deal begins paying principal down
/// (amortizing) even before its scheduled revolving-period end.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EarlyAmortizationSpec {
    /// Cumulative-loss fraction (decimal, of the original pool balance) at or
    /// above which the revolving period ends early and amortization begins.
    pub max_cumulative_loss_pct: f64,
}

/// Allocation mode within a tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum AllocationMode {
    /// Pay recipients sequentially in order until tier allocation exhausted
    Sequential,
    /// Distribute proportionally by weight or equally if no weights
    ProRata,
}

/// Payment type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum PaymentType {
    /// Fee payment
    Fee,
    /// Interest payment
    Interest,
    /// Principal payment
    Principal,
    /// Residual/equity distribution
    Residual,
}

/// Individual payment recipient within a tier
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Recipient {
    /// Unique identifier
    pub id: String,
    /// Recipient type
    pub recipient_type: RecipientType,
    /// How to calculate payment amount
    pub calculation: PaymentCalculation,
    /// Weight for pro-rata distribution (None = equal weight)
    pub weight: Option<f64>,
}

impl Recipient {
    /// Create a new recipient
    pub fn new(
        id: impl Into<String>,
        recipient_type: RecipientType,
        calculation: PaymentCalculation,
    ) -> Self {
        Self {
            id: id.into(),
            recipient_type,
            calculation,
            weight: None,
        }
    }

    /// Set weight for pro-rata allocation
    #[must_use]
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Create a fixed fee recipient
    #[must_use]
    pub fn fixed_fee(id: impl Into<String>, provider: impl Into<String>, amount: Money) -> Self {
        Self::new(
            id,
            RecipientType::ServiceProvider(provider.into()),
            PaymentCalculation::FixedAmount {
                amount,
                rounding: None,
            },
        )
    }

    /// Create a tranche interest recipient
    #[must_use]
    pub fn tranche_interest(id: impl Into<String>, tranche_id: impl Into<String>) -> Self {
        let tranche_id_str = tranche_id.into();
        Self::new(
            id,
            RecipientType::Tranche(tranche_id_str.clone()),
            PaymentCalculation::TrancheInterest {
                tranche_id: tranche_id_str,
                rounding: None,
            },
        )
    }

    /// Create a tranche principal recipient
    #[must_use]
    pub fn tranche_principal(
        id: impl Into<String>,
        tranche_id: impl Into<String>,
        target_balance: Option<Money>,
    ) -> Self {
        let tranche_id_str = tranche_id.into();
        Self::new(
            id,
            RecipientType::Tranche(tranche_id_str.clone()),
            PaymentCalculation::TranchePrincipal {
                tranche_id: tranche_id_str,
                target_balance,
                rounding: None,
            },
        )
    }
}

/// Waterfall tier with multiple recipients
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WaterfallTier {
    /// Unique tier identifier
    pub id: String,
    /// Priority order (lower = higher priority)
    pub priority: usize,
    /// Recipients in this tier
    pub recipients: Vec<Recipient>,
    /// Payment type classification
    pub payment_type: PaymentType,
    /// How to allocate within tier
    pub allocation_mode: AllocationMode,
    /// Whether this tier can be diverted if coverage tests fail
    pub divertible: bool,
}

impl WaterfallTier {
    /// Create a new waterfall tier
    #[must_use]
    pub fn new(id: impl Into<String>, priority: usize, payment_type: PaymentType) -> Self {
        Self {
            id: id.into(),
            priority,
            recipients: Vec::new(),
            payment_type,
            allocation_mode: AllocationMode::Sequential,
            divertible: false,
        }
    }

    /// Add a recipient to this tier
    #[must_use]
    pub fn add_recipient(mut self, recipient: Recipient) -> Self {
        self.recipients.push(recipient);
        self
    }

    /// Set allocation mode
    #[must_use]
    pub fn allocation_mode(mut self, mode: AllocationMode) -> Self {
        self.allocation_mode = mode;
        self
    }

    /// Mark as divertible
    #[must_use]
    pub fn divertible(mut self, divertible: bool) -> Self {
        self.divertible = divertible;
        self
    }
}

// ============================================================================
// WATERFALL RESULT
// ============================================================================

/// Result of waterfall distribution
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WaterfallDistribution {
    /// Payment date
    #[schemars(with = "String")]
    pub payment_date: Date,
    /// Total available cash at start
    pub total_available: Money,

    /// Tier-level allocations
    pub tier_allocations: Vec<(String, Money)>,

    /// Distributions by recipient
    pub distributions: HashMap<RecipientType, Money>,
    /// Detailed payment records
    pub payment_records: Vec<PaymentRecord>,

    /// Coverage test results (test_name, value, passed)
    pub coverage_tests: Vec<(String, f64, bool)>,

    /// Total diverted cash
    pub diverted_cash: Money,
    /// Remaining undistributed cash
    pub remaining_cash: Money,
    /// Whether any diversions occurred
    pub had_diversions: bool,
    /// Diversion reason if applicable
    pub diversion_reason: Option<String>,
    /// Detailed diverted payment records.
    #[serde(default)]
    pub diverted_amounts: Vec<DiversionRecord>,

    /// Recovery proceeds included in this period's available cash.
    /// Tracked separately from principal collections for trustee report reconciliation.
    #[serde(default = "WaterfallDistribution::zero_usd")]
    pub recovery_proceeds: Money,

    /// Optional explanation trace (enabled via ExplainOpts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplanationTrace>,
}

impl WaterfallDistribution {
    fn zero_usd() -> Money {
        Money::new(0.0, finstack_quant_core::currency::Currency::USD)
    }
}

/// Record of a diverted payment from one tier to another recipient.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DiversionRecord {
    /// Source tier where cash originated.
    pub source_tier: String,
    /// Recipient identifier that received the diverted cash.
    pub target_tranche: String,
    /// Diverted amount.
    pub amount: Money,
    /// Human-readable reason for diversion.
    pub reason: String,
}

/// Record of individual payment
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PaymentRecord {
    /// Tier id
    pub tier_id: String,
    /// Recipient id within tier
    pub recipient_id: String,
    /// Priority
    pub priority: usize,
    /// Recipient
    pub recipient: RecipientType,
    /// Requested amount
    pub requested_amount: Money,
    /// Paid amount
    pub paid_amount: Money,
    /// Shortfall
    pub shortfall: Money,
    /// Diverted
    pub diverted: bool,
}

// ============================================================================
// COVERAGE TRIGGERS
// ============================================================================

/// Simple OC/IC trigger for diversion
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CoverageTestRules {
    /// Haircuts applied by collateral rating
    pub haircuts: HashMap<CreditRating, f64>,
    /// Optional par-value threshold ratio (collateral / liabilities)
    pub par_value_threshold: Option<f64>,
}

impl CoverageTestRules {
    /// Create new coverage rules.
    pub fn new(haircuts: HashMap<CreditRating, f64>, par_value_threshold: Option<f64>) -> Self {
        Self {
            haircuts,
            par_value_threshold,
        }
    }

    /// Empty/default rules (no haircuts, no threshold).
    pub fn empty() -> Self {
        Self {
            haircuts: HashMap::default(),
            par_value_threshold: None,
        }
    }

    /// Check whether no rules are configured.
    pub fn is_empty(&self) -> bool {
        self.haircuts.is_empty() && self.par_value_threshold.is_none()
    }
}

impl From<&super::setup::CoverageTestConfig> for CoverageTestRules {
    fn from(config: &super::setup::CoverageTestConfig) -> Self {
        Self {
            haircuts: config.haircuts.clone(),
            par_value_threshold: config.par_value_threshold,
        }
    }
}

/// Coverage trigger definition used for diversion logic (OC/IC thresholds).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CoverageTrigger {
    /// Tranche where test applies
    pub tranche_id: String,
    /// OC trigger level (e.g., 1.15 = 115%)
    pub oc_trigger: Option<f64>,
    /// IC trigger level (e.g., 1.10 = 110%)
    pub ic_trigger: Option<f64>,
}

/// Type of coverage test (simplified to OC/IC only)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum CoverageTestType {
    /// Overcollateralization test
    OC,
    /// Interest coverage test
    IC,
}

// ============================================================================
// WATERFALL WORKSPACE (Pre-allocated Buffers)
// ============================================================================

/// Pre-allocated workspace for waterfall execution to avoid hot-path allocations.
///
/// This struct holds reusable buffers that are cleared between periods rather than
/// reallocated. For Monte Carlo simulations with thousands of paths and hundreds
/// of periods, this significantly reduces allocation overhead.
#[derive(Debug, Clone)]
pub struct WaterfallWorkspace {
    /// Pre-allocated tier allocations buffer
    pub tier_allocations: Vec<(String, Money)>,
    /// Pre-allocated distributions map
    pub distributions: HashMap<RecipientType, Money>,
    /// Pre-allocated payment records buffer
    pub payment_records: Vec<PaymentRecord>,
    /// Pre-allocated coverage test results buffer
    pub coverage_tests: Vec<(String, f64, bool)>,
    /// Pre-allocated tranche index (built once per deal, reused across periods)
    pub tranche_index: HashMap<String, usize>,
}

impl WaterfallWorkspace {
    /// Create a new workspace with pre-allocated capacity.
    pub fn new(num_tiers: usize, num_recipients: usize, num_tranches: usize) -> Self {
        let mut distributions = HashMap::default();
        distributions.reserve(num_recipients);
        let mut tranche_index = HashMap::default();
        tranche_index.reserve(num_tranches);
        Self {
            tier_allocations: Vec::with_capacity(num_tiers),
            distributions,
            payment_records: Vec::with_capacity(num_recipients),
            coverage_tests: Vec::with_capacity(num_tranches * 2),
            tranche_index,
        }
    }

    /// Create workspace from a Waterfall and TrancheStructure.
    pub fn from_engine(engine: &Waterfall, tranches: &super::TrancheStructure) -> Self {
        let num_tiers = engine.tiers.len();
        let num_recipients: usize = engine.tiers.iter().map(|t| t.recipients.len()).sum();
        let num_tranches = tranches.tranches.len();

        let mut workspace = Self::new(num_tiers, num_recipients, num_tranches);

        for (i, t) in tranches.tranches.iter().enumerate() {
            workspace.tranche_index.insert(t.id.to_string(), i);
        }

        workspace
    }

    /// Clear all buffers for reuse in the next period.
    pub fn clear(&mut self) {
        self.tier_allocations.clear();
        self.distributions.clear();
        self.payment_records.clear();
        self.coverage_tests.clear();
    }
}

impl Default for WaterfallWorkspace {
    fn default() -> Self {
        Self::new(8, 32, 8)
    }
}

// ============================================================================
// WATERFALL ENGINE
// ============================================================================

/// Main waterfall engine with tier-based distribution
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Waterfall {
    /// Ordered payment tiers
    pub tiers: Vec<WaterfallTier>,
    /// Coverage triggers for OC/IC diversion
    pub coverage_triggers: Vec<CoverageTrigger>,
    /// Base currency
    pub base_currency: Currency,
    /// Optional coverage test rules (haircuts, par thresholds)
    pub coverage_rules: Option<CoverageTestRules>,
}

impl Waterfall {
    /// Create a [`WaterfallBuilder`] for constructing a new waterfall engine.
    ///
    /// This is the preferred entry point, consistent with other builder patterns.
    #[must_use]
    pub fn builder(base_currency: Currency) -> WaterfallBuilder {
        WaterfallBuilder::new(base_currency)
    }

    /// Create new waterfall engine
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            tiers: Vec::new(),
            coverage_triggers: Vec::new(),
            base_currency,
            coverage_rules: None,
        }
    }

    /// Add a tier
    #[must_use]
    pub fn add_tier(mut self, tier: WaterfallTier) -> Self {
        self.tiers.push(tier);
        self.tiers.sort_by_key(|t| t.priority);
        self
    }

    /// Add coverage trigger for OC/IC diversion
    #[must_use]
    pub fn add_coverage_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.coverage_triggers.push(trigger);
        self
    }

    /// Attach coverage test rules (e.g., rating haircuts).
    #[must_use]
    pub fn with_coverage_rules(mut self, rules: CoverageTestRules) -> Self {
        self.coverage_rules = Some(rules);
        self
    }

    /// Create a standard sequential waterfall for a given tranche structure.
    ///
    /// This creates a typical CLO/ABS waterfall with:
    /// 1. Fees tier (sequential)
    /// 2. Interest tier (sequential, by priority)
    /// 3. Principal tier (sequential, by priority, divertible)
    /// 4. Equity tier (residual)
    pub fn standard_sequential(
        base_currency: Currency,
        tranches: &super::TrancheStructure,
        fee_recipients: Vec<Recipient>,
    ) -> Self {
        let mut engine = Self::new(base_currency);
        let mut priority = 1;

        // Add fees tier
        if !fee_recipients.is_empty() {
            let fees_tier = WaterfallTier::new("fees", priority, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential);
            let fees_tier = fee_recipients
                .into_iter()
                .fold(fees_tier, |tier, recipient| tier.add_recipient(recipient));
            engine.tiers.push(fees_tier);
            priority += 1;
        }

        // Add interest tier
        let mut sorted_tranches = tranches.tranches.clone();
        sorted_tranches.sort_by_key(|t| t.payment_priority);

        let mut interest_recipients = Vec::new();
        for tranche in &sorted_tranches {
            if tranche.seniority != super::TrancheSeniority::Equity {
                interest_recipients.push(Recipient::tranche_interest(
                    format!("{}_interest", tranche.id.as_str()),
                    tranche.id.as_str(),
                ));
            }
        }

        if !interest_recipients.is_empty() {
            let interest_tier = WaterfallTier::new("interest", priority, PaymentType::Interest)
                .allocation_mode(AllocationMode::Sequential);
            let interest_tier = interest_recipients
                .into_iter()
                .fold(interest_tier, |tier, recipient| {
                    tier.add_recipient(recipient)
                });
            engine.tiers.push(interest_tier);
            priority += 1;
        }

        // Add principal tier
        let mut principal_recipients = Vec::new();
        for tranche in &sorted_tranches {
            if tranche.seniority != super::TrancheSeniority::Equity {
                principal_recipients.push(Recipient::tranche_principal(
                    format!("{}_principal", tranche.id.as_str()),
                    tranche.id.as_str(),
                    None,
                ));
            }
        }

        if !principal_recipients.is_empty() {
            let principal_tier = WaterfallTier::new("principal", priority, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential);
            let principal_tier = principal_recipients
                .into_iter()
                .fold(principal_tier, |tier, recipient| {
                    tier.add_recipient(recipient)
                });
            engine.tiers.push(principal_tier);
            priority += 1;
        }

        // Equity tier is divertible: when OC/IC tests fail, residual cash that
        // would go to equity is redirected to the most senior principal tier
        // (cash trap / turbo paydown). This matches INTEX/Bloomberg CLO convention.
        let equity_tier = WaterfallTier::new("equity", priority, PaymentType::Residual)
            .allocation_mode(AllocationMode::Sequential)
            .divertible(true)
            .add_recipient(Recipient::new(
                "equity_distribution",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            ));
        engine.tiers.push(equity_tier);

        engine
    }
}

/// Builder for waterfall engine
pub struct WaterfallBuilder {
    engine: Waterfall,
    next_priority: usize,
}

impl WaterfallBuilder {
    /// Create new builder
    #[must_use]
    pub fn new(base_currency: Currency) -> Self {
        Self {
            engine: Waterfall::new(base_currency),
            next_priority: 1,
        }
    }

    /// Add a tier
    #[must_use]
    pub fn add_tier(mut self, mut tier: WaterfallTier) -> Self {
        if tier.priority == 0 {
            tier.priority = self.next_priority;
            self.next_priority += 1;
        }
        self.engine = self.engine.add_tier(tier);
        self
    }

    /// Add coverage trigger
    #[must_use]
    pub fn add_coverage_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.engine = self.engine.add_coverage_trigger(trigger);
        self
    }

    /// Attach coverage test rules (haircuts, par thresholds).
    #[must_use]
    pub fn coverage_rules(mut self, rules: CoverageTestRules) -> Self {
        self.engine = self.engine.with_coverage_rules(rules);
        self
    }

    /// Build the waterfall engine
    pub fn build(self) -> finstack_quant_core::Result<Waterfall> {
        Ok(self.engine)
    }
}
