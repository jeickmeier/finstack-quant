//! Waterfall type definitions for structured credit instruments.
//!
//! This module contains all data structures for waterfall distribution:
//! - Payment recipients and calculation methods
//! - Tier structures and allocation modes
//! - Coverage-test positions (`PaymentType::CoverageTest` tiers carrying
//!   `CoverageTestSpec`s) for OC/IC diversion
//! - Result types for waterfall execution
//!
//! Execution logic is in `crate::instruments::fixed_income::structured_credit::pricing::waterfall`.
//! The declarative period rules live in [`rules`], the coverage-test
//! declarations in [`coverage`] and construction in [`builder`].

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::explain::ExplanationTrace;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CreditRating;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Recipient of waterfall payments
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum ManagementFeeType {
    /// Senior variant.
    Senior,
    /// Subordinated variant.
    Subordinated,
    /// Incentive variant.
    Incentive,
}

/// Rounding convention for payments
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
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
    /// Percentage of the specially serviced collateral balance (CMBS special
    /// servicing fee): the balances of pool assets carrying a
    /// `special_servicing` spec.
    PercentageOfSpecialServiced {
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
        /// Balance the regular principal pass amortizes the tranche down to;
        /// `None` pays it in full. A coverage-test cure diversion at an earlier
        /// position pays toward zero and counts toward this target, so the
        /// regular pass only completes the remaining distance to it.
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
    /// net weighted-average coupon supports. The cap defines the *claim*, not
    /// just the allocation: the capped-off coupon is never owed, so it does not
    /// defer, does not PIK, and does not enter IC coverage or excess-spread
    /// sizing (no carryforward in this variant).
    CappedTrancheInterest {
        /// Tranche id.
        tranche_id: String,
        /// Cap on the annualized coupon rate (decimal, e.g. `0.03` = 3%).
        cap_rate: f64,
        /// Rounding convention.
        rounding: Option<RoundingConvention>,
    },
    /// Net-WAC carryover repayment: the tranche's carryover balance brought
    /// into the period (`amount`, set by the engine each period from the
    /// interest its available-funds cap withheld in earlier periods), paid
    /// from the cash reaching this recipient and recorded as interest on the
    /// tranche outside its capped claim.
    NetWacCarryover {
        /// Tranche id.
        tranche_id: String,
        /// Carryover balance outstanding at the period's opening.
        amount: Money,
    },
    /// Manager incentive fee: `share_pct` of the cash reaching this recipient
    /// that lies above the equity hurdle. The hurdle is tested on the
    /// [`EquityHistory`] in the `WaterfallContext` plus every equity
    /// distribution earlier in the same waterfall run plus this cash; the
    /// cash that lifts the equity IRR exactly to `hurdle_irr`
    /// ([`EquityHistory::hurdle_shortfall`]) passes to equity untouched and
    /// the manager shares only in the excess. Nothing is paid while the
    /// hurdle is unreachable, or when no equity history is supplied. The
    /// standard template places one recipient in the principal tier ahead of
    /// `equity_principal` and one ahead of the residual.
    IncentiveFee {
        /// Equity IRR hurdle as an annual decimal.
        hurdle_irr: f64,
        /// Share of the residual paid once the hurdle is met, in `[0, 1]`.
        share_pct: f64,
    },
}

/// Equity's cash history to date, the input of an incentive-fee IRR test.
#[derive(Debug, Clone, PartialEq)]
pub struct EquityHistory {
    /// Date the equity capital was invested (the deal closing date).
    pub invested_on: Date,
    /// Equity capital invested at closing (the equity notes' original par).
    pub invested: Money,
    /// Every distribution to equity so far, in date order.
    pub distributions: Vec<(Date, Money)>,
}

impl EquityHistory {
    /// Annual IRR of the equity investment against its distributions to date
    /// plus `candidate` paid on `payment_date`, or `None` when no IRR exists
    /// (no capital invested, or no sign change yet).
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Date the candidate distribution would be paid.
    /// * `candidate` - Cash that would reach equity on `payment_date`.
    #[must_use]
    pub fn irr_with(&self, payment_date: Date, candidate: Money) -> Option<f64> {
        if self.invested.amount() <= 0.0 {
            return None;
        }
        let mut flows: Vec<(Date, f64)> = Vec::with_capacity(self.distributions.len() + 2);
        flows.push((self.invested_on, -self.invested.amount()));
        flows.extend(
            self.distributions
                .iter()
                .map(|(date, amount)| (*date, amount.amount())),
        );
        flows.push((payment_date, candidate.amount()));
        finstack_quant_core::cashflow::xirr(&flows, None)
            .ok()
            .filter(|irr| irr.is_finite())
    }

    /// Cash on `payment_date` that lifts the equity IRR to `hurdle_irr`,
    /// capped at `candidate`: zero when the hurdle is already earned, all of
    /// `candidate` when even that much leaves the IRR below the hurdle (or no
    /// IRR exists). The manager's incentive share applies to
    /// `candidate − shortfall`.
    ///
    /// # Arguments
    ///
    /// * `payment_date` - Date the candidate distribution would be paid.
    /// * `candidate` - Total cash that could reach equity on `payment_date`.
    /// * `hurdle_irr` - Equity IRR hurdle as an annual decimal.
    #[must_use]
    pub fn hurdle_shortfall(&self, payment_date: Date, candidate: Money, hurdle_irr: f64) -> Money {
        let currency = candidate.currency();
        let total = candidate.amount().max(0.0);
        let irr_at = |cash: f64| {
            Money::new(cash, currency)
                .ok()
                .and_then(|money| self.irr_with(payment_date, money))
        };
        if irr_at(0.0).is_some_and(|irr| irr >= hurdle_irr) {
            return Money::from((0_i64, currency));
        }
        if !irr_at(total).is_some_and(|irr| irr >= hurdle_irr) {
            return candidate;
        }
        // IRR is monotone in the candidate cash: bisect for the crossing.
        let (mut low, mut high) = (0.0_f64, total);
        for _ in 0..64 {
            let mid = 0.5 * (low + high);
            if irr_at(mid).is_some_and(|irr| irr >= hurdle_irr) {
                high = mid;
            } else {
                low = mid;
            }
            if high - low <= 0.005 {
                break;
            }
        }
        Money::new(high, currency).unwrap_or(candidate)
    }
}

mod builder;
mod coverage;
mod rules;

pub use builder::WaterfallBuilder;
pub use coverage::{
    CccBucketRule, CoveragePlacement, CoverageRules, CoverageTestAction, CoverageTestSpec,
    CoverageTestType, DefaultedValuation, DiscountObligationRule,
};
pub use rules::{
    AfcSpec, ControlledAccumulationSpec, EarlyAmortizationSpec, ExcessSpreadSpec,
    ReserveAccountSpec, ReserveTarget, ShiftMode, ShiftingInterestSpec, ShiftingInterestStep,
    StepDownSpec, StepDownTrigger, TargetOcSpec, WaterfallRules,
};

/// Allocation mode within a tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum AllocationMode {
    /// Pay recipients sequentially in order until tier allocation exhausted
    Sequential,
    /// Distribute proportionally by weight or equally if no weights
    ProRata,
}

/// Payment type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    /// Fee payment funded from interest collections.
    Fee,
    /// Coupon payment funded from interest collections.
    Interest,
    /// Capital repayment funded from principal collections.
    Principal,
    /// Excess-interest distribution; does not retire equity principal.
    Residual,
    /// Coverage-test position: the tier pays nobody itself. While any of its
    /// [`WaterfallTier::tests`] fails, the interest still undistributed at
    /// this point in the waterfall is diverted (up to the cure amount) per the
    /// test's [`CoverageTestAction`], so only tiers ranked below the test can
    /// lose cash to it.
    CoverageTest,
}

/// Individual payment recipient within a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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

/// Collection account(s) a waterfall tier draws on.
///
/// Interest and principal proceeds are separate accounts in the executor.
/// A tier normally draws on the account matching its [`PaymentType`]; a
/// `Fee` or `Interest` tier may instead be allowed to top up from principal
/// proceeds (the CLO principal-waterfall convention that senior fees and
/// senior note interest shortfalls are paid from principal before any note
/// is redeemed). Cash taken from principal this way is reported as
/// [`WaterfallDistribution::principal_used_for_interest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FundingSource {
    /// Interest proceeds only (the default for fee, interest and residual tiers).
    Interest,
    /// Principal proceeds only (the default for principal tiers).
    Principal,
    /// Interest proceeds first, then principal proceeds for any shortfall.
    InterestThenPrincipal,
}

impl FundingSource {
    /// Account a tier of `payment_type` draws on when it sets no explicit
    /// funding source.
    ///
    /// # Arguments
    ///
    /// * `payment_type` - Tier classification; `Principal` tiers draw on
    ///   principal proceeds, every other tier on interest proceeds.
    #[must_use]
    pub fn default_for(payment_type: PaymentType) -> Self {
        match payment_type {
            PaymentType::Principal => Self::Principal,
            _ => Self::Interest,
        }
    }
}

/// Waterfall tier: a payment step with recipients, or a coverage-test
/// position ([`PaymentType::CoverageTest`]) carrying `tests` and no recipients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallTier {
    /// Unique tier identifier
    pub id: String,
    /// Priority order (lower = higher priority)
    pub priority: usize,
    /// Recipients in this tier (empty for a coverage-test tier)
    pub recipients: Vec<Recipient>,
    /// Payment type classification
    pub payment_type: PaymentType,
    /// How to allocate within tier
    pub allocation_mode: AllocationMode,
    /// Coverage tests evaluated at this position (only for
    /// [`PaymentType::CoverageTest`] tiers; empty otherwise). Every test in
    /// one tier shares the same [`CoverageTestAction`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<CoverageTestSpec>,
    /// Collection account(s) this tier draws on; `None` uses
    /// [`FundingSource::default_for`] the tier's `payment_type`. See
    /// [`Self::effective_funding`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub funding: Option<FundingSource>,
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
            tests: Vec::new(),
            funding: None,
        }
    }

    /// Set the collection account(s) this tier draws on.
    ///
    /// # Arguments
    ///
    /// * `source` - Account(s) to pay this tier from; `InterestThenPrincipal`
    ///   lets a fee or interest tier top up from principal proceeds.
    #[must_use]
    pub fn funding(mut self, source: FundingSource) -> Self {
        self.funding = Some(source);
        self
    }

    /// Account(s) this tier draws on: the explicit `funding`, or the default
    /// for its `payment_type`.
    #[must_use]
    pub fn effective_funding(&self) -> FundingSource {
        self.funding
            .unwrap_or_else(|| FundingSource::default_for(self.payment_type))
    }

    /// Create a coverage-test tier at `priority` carrying `tests`.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique tier identifier.
    /// * `priority` - Position in the waterfall (lower runs first); `0` lets
    ///   [`WaterfallBuilder::add_tier`] assign the next priority.
    /// * `tests` - Coverage tests evaluated at this position; they must share
    ///   one [`CoverageTestAction`].
    #[must_use]
    pub fn coverage_tests(
        id: impl Into<String>,
        priority: usize,
        tests: Vec<CoverageTestSpec>,
    ) -> Self {
        Self {
            id: id.into(),
            priority,
            recipients: Vec::new(),
            payment_type: PaymentType::CoverageTest,
            allocation_mode: AllocationMode::Sequential,
            tests,
            funding: None,
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

    /// Tranche ids whose interest claim this tier pays.
    pub(crate) fn interest_tranche_ids(&self) -> impl Iterator<Item = &str> {
        self.recipients
            .iter()
            .filter_map(|recipient| match &recipient.calculation {
                PaymentCalculation::TrancheInterest { tranche_id, .. }
                | PaymentCalculation::CappedTrancheInterest { tranche_id, .. } => {
                    Some(tranche_id.as_str())
                }
                _ => None,
            })
    }
}

/// Result of waterfall distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallDistribution {
    /// Payment date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Total available cash at start
    pub total_available: Money,

    /// Tier-level allocations
    pub tier_allocations: Vec<(String, Money)>,

    /// Distributions by recipient
    pub distributions: BTreeMap<RecipientType, Money>,
    /// The PRINCIPAL portion of `distributions`, per recipient.
    ///
    /// `distributions` aggregates a tranche's interest and principal
    /// into one number, because `RecipientType::Tranche(id)` is the same key
    /// for both. Step 5 therefore had to RE-DERIVE the split by assuming
    /// interest is satisfied first — which silently reclassified a diverted
    /// OC-cure principal payment as interest whenever that tranche carried a
    /// shortfall, leaving the balance unretired so the cure could not reduce
    /// the very denominator it was sized to fix.
    ///
    /// The waterfall already knows the answer: `PaymentCalculation`
    /// distinguishes `TranchePrincipal` from `TrancheInterest`. Reporting it
    /// here makes the classification authoritative instead of reconstructed.
    pub principal_distributions: BTreeMap<RecipientType, Money>,
    /// Detailed payment records
    pub payment_records: Vec<PaymentRecord>,

    /// Coverage test results (test_name, value, passed)
    pub coverage_tests: Vec<(String, f64, bool)>,

    /// Total diverted cash
    pub diverted_cash: Money,
    /// Remaining undistributed cash
    pub remaining_cash: Money,
    /// Undistributed interest retained for subsequent interest-waterfall periods.
    pub remaining_interest: Money,
    /// Undistributed principal retained for reinvestment or debt repayment.
    pub remaining_principal: Money,
    /// Principal proceeds spent on fee or interest tiers whose
    /// [`FundingSource`] is `InterestThenPrincipal`. Already netted out of
    /// `remaining_principal`; reported so the principal account's use is
    /// visible.
    pub principal_used_for_interest: Money,
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
        Money::from((0_i64, finstack_quant_core::currency::Currency::USD))
    }
}

/// Record of a diverted payment from one tier to another recipient.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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

/// Main waterfall engine with tier-based distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Waterfall {
    /// Ordered payment tiers, including [`PaymentType::CoverageTest`] positions
    pub tiers: Vec<WaterfallTier>,
    /// Base currency
    pub base_currency: Currency,
    /// Collateral valuation rules for the OC tests; `None` values collateral
    /// at par with defaulted assets at recovery.
    pub coverage_rules: Option<CoverageRules>,
}

/// Fee recipients of the standard template, by rank.
#[derive(Debug, Clone, Default)]
pub struct TemplateFees {
    /// Senior fees paid ahead of every note (trustee, senior management,
    /// servicing); an empty list omits the fees tier.
    pub senior: Vec<Recipient>,
    /// Junior fees paid after every note coupon and ahead of principal (the
    /// subordinated management fee); an empty list omits the tier.
    pub junior: Vec<Recipient>,
    /// Manager incentive fee on the cash reaching equity above its IRR
    /// hurdle ([`PaymentCalculation::IncentiveFee`]): placed in the principal
    /// tier ahead of `equity_principal` and in a tier ahead of the residual.
    pub incentive: Option<Recipient>,
    /// Net-WAC carryover recipients ([`PaymentCalculation::NetWacCarryover`]),
    /// one per capped tranche, in a tier after principal and ahead of the
    /// incentive fee; empty omits the tier.
    pub carryover: Vec<Recipient>,
}
