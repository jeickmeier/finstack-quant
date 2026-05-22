//! Tranche structures for structured credit instruments.

// InterestSpec removed with loan; retain coupon for metadata only
use crate::instruments::common_impl::traits::Attributes;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::money::Money;
#[cfg(test)]
use finstack_core::types::CurveId;
use finstack_core::types::InstrumentId;
use rust_decimal::prelude::ToPrimitive;

use serde::{Deserialize, Serialize};

use super::enums::{TrancheSeniority, TriggerConsequence};
use finstack_core::types::CreditRating;

/// Tranche behavioral types (simplified to standard only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
pub enum TrancheBehaviorType {
    /// Standard bond (pays interest and principal)
    Standard,
}

fn default_behavior_type() -> TrancheBehaviorType {
    TrancheBehaviorType::Standard
}

/// Coverage test trigger specification
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CoverageTrigger {
    /// Trigger threshold level (e.g., 1.20 for 120% OC)
    pub trigger_level: f64,
    /// Higher level required to cure breach
    pub cure_level: Option<f64>,
    /// Date when breach occurred (if any)
    #[schemars(with = "Option<String>")]
    pub breach_date: Option<Date>,
    /// What happens when triggered
    pub consequence: TriggerConsequence,
}

impl CoverageTrigger {
    /// Create a new coverage trigger
    pub fn new(trigger_level: f64, consequence: TriggerConsequence) -> Self {
        Self {
            trigger_level,
            cure_level: None,
            breach_date: None,
            consequence,
        }
    }

    /// With cure level (typically higher than trigger)
    pub fn with_cure_level(mut self, cure_level: f64) -> Self {
        self.cure_level = Some(cure_level);
        self
    }

    /// Check if currently breached
    pub fn is_breached(&self, current_level: f64) -> bool {
        current_level < self.trigger_level
    }

    /// Check if breach is cured
    pub fn is_cured(&self, current_level: f64) -> bool {
        if let Some(cure) = self.cure_level {
            current_level >= cure
        } else {
            current_level >= self.trigger_level
        }
    }
}

/// Credit enhancement mechanisms for a tranche
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreditEnhancement {
    /// Subordination amount (sum of junior tranches)
    pub subordination: Money,
    /// Overcollateralization amount
    pub overcollateralization: Money,
    /// Reserve account balance
    pub reserve_account: Money,
    /// Available excess spread
    pub excess_spread: f64,
    /// Cash trap/turbo active
    pub cash_trap_active: bool,
}

impl Default for CreditEnhancement {
    fn default() -> Self {
        Self {
            subordination: Money::new(0.0, finstack_core::currency::Currency::USD),
            overcollateralization: Money::new(0.0, finstack_core::currency::Currency::USD),
            reserve_account: Money::new(0.0, finstack_core::currency::Currency::USD),
            excess_spread: 0.0,
            cash_trap_active: false,
        }
    }
}

/// Tranche coupon specification
///
/// Supports fixed and floating rate coupons used in standard structured credit instruments.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum TrancheCoupon {
    /// Fixed rate coupon (rate as decimal, e.g., 0.05 for 5%)
    Fixed {
        /// Fixed interest rate as decimal (e.g., 0.05 for 5%)
        rate: f64,
    },

    /// Floating rate coupon using canonical FloatingRateSpec.
    ///
    /// Uses the standard floating rate specification with all rates in basis points.
    Floating(crate::cashflow::builder::FloatingRateSpec),
}

impl TrancheCoupon {
    /// Get current rate for a given date (without index lookup)
    ///
    /// For Fixed: returns the fixed rate
    /// For Floating: returns just the spread component (use current_rate_with_index for full rate)
    pub fn current_rate(&self, _date: Date) -> f64 {
        match self {
            TrancheCoupon::Fixed { rate } => *rate,
            TrancheCoupon::Floating(spec) => spec.spread_bp.to_f64().unwrap_or_default() / 10_000.0,
        }
    }

    /// Compute current rate including index forward where applicable.
    ///
    /// For Fixed coupons, returns the fixed rate.
    /// For Floating coupons, uses centralized projection with floor/cap support.
    /// Uses proper calendar-based month addition for period end calculation.
    pub fn current_rate_with_index(
        &self,
        date: Date,
        context: &finstack_core::market_data::context::MarketContext,
    ) -> f64 {
        // Best-effort wrapper that preserves historical behavior:
        // - Missing market data falls back to spread-only
        // - Projection failures fall back to spread-only
        //
        // For correctness-first valuation (no silent fallbacks), prefer
        // `try_current_rate_with_index` and propagate the error.
        self.try_current_rate_with_index(date, context)
            .unwrap_or_else(|_| self.current_rate(date))
    }

    /// Compute current rate including index forward where applicable (fallible).
    ///
    /// This method returns an error if required market data is missing or if the
    /// rate projection fails. Prefer this in pricing/valuation code paths to avoid
    /// silent mispricing.
    pub fn try_current_rate_with_index(
        &self,
        date: Date,
        context: &finstack_core::market_data::context::MarketContext,
    ) -> finstack_core::Result<f64> {
        match self {
            TrancheCoupon::Fixed { rate } => Ok(*rate),
            TrancheCoupon::Floating(spec) => {
                let fwd = context.get_forward(spec.index_id.as_str())?;
                let tenor = fwd.tenor();
                let period_end = crate::instruments::fixed_income::structured_credit::utils::rate_helpers::
                    try_tenor_to_period_end(date, tenor, fwd.day_count())?;

                let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                crate::cashflow::builder::project_floating_rate(
                    date,
                    period_end,
                    fwd.as_ref(),
                    &params,
                )
            }
        }
    }
}

/// Structured credit tranche with attachment/detachment points
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tranche {
    /// Unique tranche identifier
    pub id: InstrumentId,

    /// Structural boundaries (as % of total capital structure)
    pub attachment_point: f64, // Lower bound (e.g., 0.0% for equity, 10% for mezz)
    /// Detachment point as percentage (upper loss bound for this tranche)
    pub detachment_point: f64, // Upper bound (e.g., 10% for equity, 15% for mezz)

    /// Behavioral classification for specialized handling
    #[serde(default = "default_behavior_type")]
    pub behavior_type: TrancheBehaviorType,

    /// Tranche characteristics
    pub seniority: TrancheSeniority,
    /// Credit rating (if rated by agencies)
    pub rating: Option<CreditRating>,

    /// Size and balances
    pub original_balance: Money,
    /// Current outstanding balance (after amortization and losses)
    pub current_balance: Money,
    /// Target balance for revolving period (optional, for revolving structures)
    pub target_balance: Option<Money>, // For revolving structures

    /// Interest specification
    pub coupon: TrancheCoupon,

    /// Coverage test triggers
    pub oc_trigger: Option<CoverageTrigger>,
    /// Interest coverage trigger specification
    pub ic_trigger: Option<CoverageTrigger>,

    /// Credit enhancement details
    pub credit_enhancement: CreditEnhancement,

    /// Payment characteristics
    pub frequency: Tenor,
    /// Day count convention for interest accrual
    pub day_count: DayCount,
    /// Accumulated deferred interest (if payment has been deferred)
    pub deferred_interest: Money,

    /// Whether interest shortfalls capitalize into tranche balance (PIK accretion).
    ///
    /// When `true`, unpaid interest is added to the outstanding tranche balance
    /// and accrues interest in subsequent periods (payment-in-kind).
    /// When `false` (default for debt tranches), shortfalls are tracked but do
    /// NOT increase the balance, matching standard CLO/ABS indenture treatment
    /// where shortfalls are paid from future interest collections.
    #[serde(default)]
    pub pik_enabled: bool,

    /// Structural features
    pub is_revolving: bool,
    /// Whether reinvestment of principal is permitted
    pub can_reinvest: bool,
    /// Legal final maturity date
    #[schemars(with = "String")]
    pub maturity: Date,
    /// Expected maturity date (may be earlier than legal maturity for CLOs)
    #[schemars(with = "Option<String>")]
    pub expected_maturity: Option<Date>,

    /// Payment priority (1 = most senior, paid first).
    ///
    /// On a standalone `Tranche` this is a *provisional* value derived from
    /// `seniority` (see `Tranche::new`). It is **overwritten deterministically**
    /// by [`TrancheStructure::new`] (and by deserialization of a
    /// `TrancheStructure`), which ranks every tranche by structural seniority so
    /// that multiple notes at one `TrancheSeniority` (e.g. Class A-1/A-2/A-3 all
    /// `Senior`) receive distinct, strictly-increasing priorities. Do not rely
    /// on this field outside of an assembled `TrancheStructure`.
    pub payment_priority: u32,

    /// Attributes for scenario selection
    pub attributes: Attributes,
}

impl Tranche {
    /// Create a new tranche with required fields
    pub fn new(
        id: impl Into<String>,
        attachment_point: f64,
        detachment_point: f64,
        seniority: TrancheSeniority,
        original_balance: Money,
        coupon: TrancheCoupon,
        maturity: Date,
    ) -> finstack_core::Result<Self> {
        // Validate attachment/detachment points
        if attachment_point < 0.0 || detachment_point <= attachment_point {
            return Err(finstack_core::InputError::Invalid.into());
        }
        if detachment_point > 100.0 {
            return Err(finstack_core::InputError::Invalid.into());
        }

        Ok(Self {
            id: InstrumentId::new(id.into()),
            attachment_point,
            detachment_point,
            behavior_type: TrancheBehaviorType::Standard,
            seniority,
            rating: None,
            original_balance,
            current_balance: original_balance,
            target_balance: None,
            coupon,
            oc_trigger: None,
            ic_trigger: None,
            credit_enhancement: CreditEnhancement::default(),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            deferred_interest: Money::new(0.0, original_balance.currency()),
            pik_enabled: false,
            is_revolving: false,
            can_reinvest: false,
            maturity,
            expected_maturity: None,
            // Provisional: overwritten by `TrancheStructure::new` /
            // `TrancheStructure` deserialization, which assigns a structurally
            // ranked, distinct priority per note. See the field doc comment.
            payment_priority: match seniority {
                TrancheSeniority::Senior => 1,
                TrancheSeniority::Mezzanine => 2,
                TrancheSeniority::Subordinated => 3,
                TrancheSeniority::Equity => 4,
            },
            attributes: Attributes::new(),
        })
    }

    /// Creates a new builder.
    pub fn builder() -> TrancheBuilder {
        TrancheBuilder::new()
    }

    /// Tranche thickness (detachment - attachment)
    pub fn thickness(&self) -> f64 {
        self.detachment_point - self.attachment_point
    }

    /// Check if tranche is first loss (attachment at 0%)
    pub fn is_first_loss(&self) -> bool {
        self.attachment_point == 0.0
    }

    /// Check if tranche is currently impaired by losses.
    ///
    /// A tranche is impaired when cumulative pool losses (as a fraction of
    /// performing pool balance) exceed the tranche's attachment point.
    ///
    /// # Arguments
    /// * `cumulative_loss_pct` - Cumulative net loss as a fraction of total
    ///   performing pool balance (0.0 to 1.0). E.g., 0.05 means 5% losses.
    pub fn is_impaired(&self, cumulative_loss_pct: f64) -> bool {
        cumulative_loss_pct > self.attachment_point
    }

    /// Calculate the dollar loss allocated to this tranche given cumulative pool losses.
    ///
    /// Uses attachment/detachment point convention (INTEX/Moody's Analytics):
    /// - Losses below attachment: zero allocation to this tranche
    /// - Losses between attachment and detachment: proportional allocation
    /// - Losses above detachment: tranche fully impaired (loss = original balance)
    ///
    /// # Arguments
    /// * `cumulative_loss_pct` - Cumulative net loss as fraction of performing pool balance.
    /// * `total_pool_balance` - Performing pool balance (denominator for loss pct).
    ///   Used to cross-check loss amounts, though the primary computation uses
    ///   the tranche's `original_balance` directly.
    pub fn loss_allocation(&self, cumulative_loss_pct: f64, total_pool_balance: Money) -> Money {
        let _ = total_pool_balance; // Reserved for future cross-check
        if cumulative_loss_pct <= self.attachment_point {
            // Losses have not reached this tranche's subordination
            Money::new(0.0, self.original_balance.currency())
        } else if cumulative_loss_pct >= self.detachment_point {
            // Tranche fully impaired — written down to zero
            self.original_balance
        } else {
            // Partial loss: only the portion between attachment and detachment
            let loss_within_tranche_pct = cumulative_loss_pct - self.attachment_point;
            let loss_rate = loss_within_tranche_pct / self.thickness();
            self.original_balance * loss_rate
        }
    }

    /// Current tranche balance after applying cumulative losses.
    ///
    /// Returns `max(0, current_balance - loss_allocation)`.
    pub fn current_balance_after_losses(
        &self,
        cumulative_loss_pct: f64,
        total_pool_balance: Money,
    ) -> Money {
        let loss_amount = self.loss_allocation(cumulative_loss_pct, total_pool_balance);
        Money::new(
            (self.current_balance.amount() - loss_amount.amount()).max(0.0),
            self.current_balance.currency(),
        )
    }

    /// Builder methods for fluent construction
    #[must_use]
    pub fn with_rating(mut self, rating: CreditRating) -> Self {
        self.rating = Some(rating);
        self
    }

    /// Add overcollateralization coverage trigger
    #[must_use]
    pub fn with_oc_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.oc_trigger = Some(trigger);
        self
    }

    /// Add interest coverage trigger
    #[must_use]
    pub fn with_ic_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.ic_trigger = Some(trigger);
        self
    }

    /// Mark tranche as revolving (enables reinvestment)
    #[must_use]
    pub fn revolving(mut self) -> Self {
        self.is_revolving = true;
        self.can_reinvest = true;
        self
    }

    /// Set expected maturity date (typically earlier than legal maturity)
    #[must_use]
    pub fn with_expected_maturity(mut self, date: Date) -> Self {
        self.expected_maturity = Some(date);
        self
    }
}

/// Builder for creating tranches with validation
pub struct TrancheBuilder {
    id: Option<String>,
    attachment_point: Option<f64>,
    detachment_point: Option<f64>,
    seniority: Option<TrancheSeniority>,
    original_balance: Option<Money>,
    coupon: Option<TrancheCoupon>,
    maturity: Option<Date>,
    rating: Option<CreditRating>,
    frequency: Tenor,
    day_count: DayCount,
    pik_enabled: bool,
}

impl TrancheBuilder {
    /// Create new tranche builder
    pub fn new() -> Self {
        Self {
            id: None,
            attachment_point: None,
            detachment_point: None,
            seniority: None,
            original_balance: None,
            coupon: None,
            maturity: None,
            rating: None,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            pik_enabled: false,
        }
    }

    /// Set tranche ID
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set attachment and detachment points (as percentages)
    #[must_use]
    pub fn attachment_detachment(mut self, attachment: f64, detachment: f64) -> Self {
        self.attachment_point = Some(attachment);
        self.detachment_point = Some(detachment);
        self
    }

    /// Set tranche seniority level
    #[must_use]
    pub fn seniority(mut self, seniority: TrancheSeniority) -> Self {
        self.seniority = Some(seniority);
        self
    }

    /// Set original tranche balance
    #[must_use]
    pub fn balance(mut self, balance: Money) -> Self {
        self.original_balance = Some(balance);
        self
    }

    /// Set coupon specification (fixed or floating)
    #[must_use]
    pub fn coupon(mut self, coupon: TrancheCoupon) -> Self {
        self.coupon = Some(coupon);
        self
    }

    /// Set legal maturity date
    #[must_use]
    pub fn maturity(mut self, date: Date) -> Self {
        self.maturity = Some(date);
        self
    }

    /// Set credit rating
    #[must_use]
    pub fn rating(mut self, rating: CreditRating) -> Self {
        self.rating = Some(rating);
        self
    }

    /// Set payment frequency
    #[must_use]
    pub fn frequency(mut self, freq: Tenor) -> Self {
        self.frequency = freq;
        self
    }

    /// Set day count convention
    #[must_use]
    pub fn day_count(mut self, dc: DayCount) -> Self {
        self.day_count = dc;
        self
    }

    /// Enable PIK (payment-in-kind) accretion for interest shortfalls.
    #[must_use]
    pub fn pik_enabled(mut self, enabled: bool) -> Self {
        self.pik_enabled = enabled;
        self
    }

    /// Build the tranche with validation
    pub fn build(self) -> finstack_core::Result<Tranche> {
        let id = self.id.ok_or(finstack_core::InputError::Invalid)?;
        let attachment_point = self
            .attachment_point
            .ok_or(finstack_core::InputError::Invalid)?;
        let detachment_point = self
            .detachment_point
            .ok_or(finstack_core::InputError::Invalid)?;
        let seniority = self.seniority.ok_or(finstack_core::InputError::Invalid)?;
        let original_balance = self
            .original_balance
            .ok_or(finstack_core::InputError::Invalid)?;

        // Validate original_balance is positive
        if original_balance.amount() <= 0.0 {
            return Err(finstack_core::InputError::Invalid.into());
        }

        let coupon = self.coupon.ok_or(finstack_core::InputError::Invalid)?;
        let maturity = self.maturity.ok_or(finstack_core::InputError::Invalid)?;

        let mut tranche = Tranche::new(
            id,
            attachment_point,
            detachment_point,
            seniority,
            original_balance,
            coupon,
            maturity,
        )?;

        if let Some(rating) = self.rating {
            tranche = tranche.with_rating(rating);
        }

        tranche.frequency = self.frequency;
        tranche.day_count = self.day_count;
        tranche.pik_enabled = self.pik_enabled;

        Ok(tranche)
    }
}

impl Default for TrancheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of tranches forming the capital structure
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TrancheStructure {
    /// Ordered tranches (typically sorted by payment priority)
    pub tranches: Vec<Tranche>,
    /// Total size of all tranches combined
    pub total_size: Money,
}

/// Deserialize a `TrancheStructure` through the same validation +
/// `payment_priority` assignment path as [`TrancheStructure::new`].
///
/// The per-`Tranche` `payment_priority` stored in a serialized structure is
/// only provisional (see `Tranche::new`); re-running assembly here guarantees
/// deserialized structures carry structurally ranked, distinct priorities and
/// makes any fixture-stored `payment_priority` purely cosmetic.
impl<'de> Deserialize<'de> for TrancheStructure {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawTrancheStructure {
            tranches: Vec<Tranche>,
            // Deserialized but intentionally ignored: `TrancheStructure::new`
            // recomputes `total_size` from tranche balances, which is the
            // authoritative value, so any stored figure is discarded.
            #[allow(dead_code)]
            total_size: Money,
        }
        let raw = RawTrancheStructure::deserialize(deserializer)?;
        TrancheStructure::new(raw.tranches).map_err(serde::de::Error::custom)
    }
}

impl TrancheStructure {
    /// Create new tranche structure.
    ///
    /// After structural validation this assigns each tranche a distinct,
    /// strictly-increasing `payment_priority` (see `Self::assign_priorities`),
    /// so that multiple notes at one `TrancheSeniority` are ranked correctly.
    pub fn new(mut tranches: Vec<Tranche>) -> finstack_core::Result<Self> {
        if tranches.is_empty() {
            return Err(finstack_core::InputError::TooFewPoints.into());
        }

        // Validate structure
        Self::validate_structure(&tranches)?;

        // Assign deterministic, distinct payment priorities by structural rank.
        Self::assign_priorities(&mut tranches);

        // Calculate total size
        let total_size = tranches.iter().try_fold(
            Money::new(0.0, tranches[0].original_balance.currency()),
            |acc, t| acc.checked_add(t.original_balance),
        )?;

        Ok(Self {
            tranches,
            total_size,
        })
    }

    /// Assign a structurally ranked, distinct `payment_priority` to each tranche.
    ///
    /// A real CLO/ABS carries multiple notes at one `TrancheSeniority` (Class
    /// A-1, A-2, A-3 all `Senior`); deriving `payment_priority` from the
    /// 4-valued seniority enum directly collapses them onto one value, which
    /// breaks `senior_to` / `subordination_amount` (strict `<`/`>` filters) and
    /// the OC/IC coverage denominators built on them.
    ///
    /// Ranking rule: `(TrancheSeniority discriminant ASCENDING, then original
    /// input-vector index ASCENDING)`. The seniority enum
    /// (`Senior < Mezzanine < Subordinated < Equity`) is the unambiguous
    /// structural ordering — the senior class is paid first and gets priority
    /// `1` (`1 = most senior`). The input-vector index breaks ties so that
    /// multiple notes at one seniority receive *distinct* strictly-increasing
    /// priorities; callers must therefore pass pari-passu Class A-1/A-2/A-3 in
    /// seniority order.
    ///
    /// Ranking on the seniority enum (rather than `attachment_point`) is
    /// deliberate: `attachment_point` is used inconsistently across the
    /// codebase's deals (some put the senior class at `0%`, some at the top of
    /// the stack), whereas `TrancheSeniority` is unambiguous. This preserves
    /// the historical relative ordering for every distinct-seniority deal and
    /// only changes behavior for genuinely same-seniority notes (the bug).
    ///
    /// After assignment the priorities are `1..=n`, distinct and contiguous.
    fn assign_priorities(tranches: &mut [Tranche]) {
        // Rank indices by seniority discriminant ascending, input index
        // ascending as the tie-break for pari-passu notes.
        let mut order: Vec<usize> = (0..tranches.len()).collect();
        order.sort_by(|&a, &b| {
            // `TrancheSeniority` derives `Ord` (Senior < Mezzanine <
            // Subordinated < Equity); the input-index tie-break orders
            // genuinely pari-passu notes by the order the caller supplied them.
            tranches[a]
                .seniority
                .cmp(&tranches[b].seniority)
                .then(a.cmp(&b))
        });
        for (rank, &idx) in order.iter().enumerate() {
            tranches[idx].payment_priority = (rank as u32) + 1;
        }

        // Priorities are distinct and contiguous (1..=n) by construction.
        debug_assert!(
            {
                let mut prios: Vec<u32> = tranches.iter().map(|t| t.payment_priority).collect();
                prios.sort_unstable();
                prios == (1..=tranches.len() as u32).collect::<Vec<_>>()
            },
            "assigned payment priorities must be distinct and contiguous 1..=n"
        );
    }

    /// Validate tranche structure for consistency
    fn validate_structure(tranches: &[Tranche]) -> finstack_core::Result<()> {
        // Validate attachment points are finite before sorting
        for tranche in tranches {
            if !tranche.attachment_point.is_finite() || !tranche.detachment_point.is_finite() {
                return Err(finstack_core::InputError::Invalid.into());
            }
        }

        // Sort by attachment point for validation
        let mut sorted_tranches = tranches.to_vec();
        sorted_tranches.sort_by(|a, b| a.attachment_point.total_cmp(&b.attachment_point));

        // Check for gaps or overlaps
        let mut expected_attachment = 0.0;
        const TOLERANCE: f64 = 1e-6;

        for tranche in &sorted_tranches {
            if (tranche.attachment_point - expected_attachment).abs() > TOLERANCE {
                return Err(finstack_core::InputError::Invalid.into());
            }
            if tranche.detachment_point <= tranche.attachment_point {
                return Err(finstack_core::InputError::Invalid.into());
            }
            expected_attachment = tranche.detachment_point;
        }

        // Should reach 100%
        if (expected_attachment - 100.0).abs() > TOLERANCE {
            return Err(finstack_core::InputError::Invalid.into());
        }

        // Check currency consistency
        let base_currency = tranches[0].original_balance.currency();
        for tranche in tranches {
            if tranche.original_balance.currency() != base_currency {
                return Err(finstack_core::Error::CurrencyMismatch {
                    expected: base_currency,
                    actual: tranche.original_balance.currency(),
                });
            }
        }

        Ok(())
    }

    /// Get tranches by seniority
    pub fn by_seniority(&self, seniority: TrancheSeniority) -> Vec<&Tranche> {
        self.tranches
            .iter()
            .filter(|t| t.seniority == seniority)
            .collect()
    }

    /// Get tranches senior to a given tranche
    pub fn senior_to(&self, tranche_id: &str) -> Vec<&Tranche> {
        let target_tranche = self.tranches.iter().find(|t| t.id.as_str() == tranche_id);

        if let Some(target) = target_tranche {
            self.tranches
                .iter()
                .filter(|t| t.payment_priority < target.payment_priority)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get total balance of senior tranches
    pub fn senior_balance(&self, tranche_id: &str) -> Money {
        self.senior_to(tranche_id)
            .iter()
            .try_fold(Money::new(0.0, self.total_size.currency()), |acc, t| {
                acc.checked_add(t.current_balance)
            })
            .unwrap_or_else(|_| Money::new(0.0, self.total_size.currency()))
    }

    /// Calculate tranche subordination amount
    pub fn subordination_amount(&self, tranche_id: &str) -> Money {
        let target_tranche = self.tranches.iter().find(|t| t.id.as_str() == tranche_id);

        if let Some(target) = target_tranche {
            self.tranches
                .iter()
                .filter(|t| t.payment_priority > target.payment_priority)
                .try_fold(Money::new(0.0, self.total_size.currency()), |acc, t| {
                    acc.checked_add(t.current_balance)
                })
                .unwrap_or_else(|_| Money::new(0.0, self.total_size.currency()))
        } else {
            Money::new(0.0, self.total_size.currency())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use time::Month;

    fn test_date() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).expect("valid date")
    }

    #[test]
    fn test_tranche_creation() {
        let tranche = Tranche::new(
            "EQUITY",
            0.0,
            10.0,
            TrancheSeniority::Equity,
            Money::new(100_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.12 },
            test_date(),
        )
        .expect("should succeed");

        assert_eq!(tranche.attachment_point, 0.0);
        assert_eq!(tranche.detachment_point, 10.0);
        assert_eq!(tranche.thickness(), 10.0);
        assert!(tranche.is_first_loss());
    }

    #[test]
    fn test_loss_allocation() {
        let tranche = Tranche::new(
            "MEZZ",
            10.0,
            15.0,
            TrancheSeniority::Mezzanine,
            Money::new(50_000_000.0, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.08 },
            test_date(),
        )
        .expect("should succeed");

        let pool_balance = Money::new(1_000_000_000.0, Currency::USD);

        // No loss case
        let loss = tranche.loss_allocation(5.0, pool_balance);
        assert_eq!(loss.amount(), 0.0);

        // Partial loss case (12% cumulative loss)
        let loss = tranche.loss_allocation(12.0, pool_balance);
        assert!(loss.amount() > 0.0);
        assert!(loss.amount() < tranche.original_balance.amount());

        // Full loss case (20% cumulative loss)
        let loss = tranche.loss_allocation(20.0, pool_balance);
        assert_eq!(loss.amount(), tranche.original_balance.amount());
    }

    #[test]
    fn test_tranche_structure_validation() {
        let equity = Tranche::builder()
            .id("EQUITY")
            .attachment_detachment(0.0, 10.0)
            .seniority(TrancheSeniority::Equity)
            .balance(Money::new(100_000_000.0, Currency::USD))
            .coupon(TrancheCoupon::Fixed { rate: 0.12 })
            .maturity(test_date())
            .build()
            .expect("should succeed");

        let senior = Tranche::builder()
            .id("SENIOR")
            .attachment_detachment(10.0, 100.0)
            .seniority(TrancheSeniority::Senior)
            .balance(Money::new(900_000_000.0, Currency::USD))
            .coupon(TrancheCoupon::Floating(
                crate::cashflow::builder::FloatingRateSpec {
                    index_id: CurveId::new("SOFR-3M".to_string()),
                    spread_bp: rust_decimal::Decimal::try_from(150.0).expect("valid"),
                    gearing: rust_decimal::Decimal::ONE,
                    gearing_includes_spread: true,
                    index_floor_bp: None,
                    all_in_cap_bp: None,
                    all_in_floor_bp: None,
                    index_cap_bp: None,
                    reset_freq: finstack_core::dates::Tenor::quarterly(),
                    reset_lag_days: 2,
                    dc: finstack_core::dates::DayCount::Act360,
                    bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
                    calendar_id: "weekends_only".to_string(),
                    fixing_calendar_id: None,
                    end_of_month: false,
                    payment_lag_days: 0,
                    overnight_compounding: None,
                    overnight_basis: None,
                    fallback: Default::default(),
                },
            ))
            .maturity(test_date())
            .build()
            .expect("should succeed");

        let structure = TrancheStructure::new(vec![equity, senior]).expect("should succeed");
        assert_eq!(structure.tranches.len(), 2);
        assert_eq!(structure.total_size.amount(), 1_000_000_000.0);
    }

    #[test]
    fn test_coverage_trigger() {
        let trigger =
            CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow).with_cure_level(1.25);

        // Breach scenario
        assert!(trigger.is_breached(1.15));
        assert!(!trigger.is_cured(1.22)); // Below cure level
        assert!(trigger.is_cured(1.26)); // Above cure level

        // Not breached
        assert!(!trigger.is_breached(1.25));
    }
}
