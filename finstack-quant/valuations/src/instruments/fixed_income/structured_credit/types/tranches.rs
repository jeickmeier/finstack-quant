//! Tranche structures for structured credit instruments.

// InterestSpec removed with loan; retain coupon for metadata only
use crate::instruments::common_impl::traits::Attributes;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::money::Money;
#[cfg(test)]
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::InstrumentId;
use rust_decimal::prelude::ToPrimitive;

use serde::{Deserialize, Serialize};

use super::enums::{TrancheSeniority, TriggerConsequence};
use finstack_quant_core::types::CreditRating;

/// Coverage-test trigger specification for a tranche or deal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CoverageTrigger {
    /// Breach threshold, expressed as a coverage ratio (1.20 means 120%).
    pub trigger_level: f64,
    /// Optional higher coverage ratio required to cure a breach.
    pub cure_level: Option<f64>,
    /// Date on which the breach was recorded, if one has occurred.
    #[serde(default, with = "finstack_quant_core::wire::optional_date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub breach_date: Option<Date>,
    /// Consequence applied while the trigger is breached.
    pub consequence: TriggerConsequence,
}

impl CoverageTrigger {
    /// Create a coverage trigger with no separate cure threshold.
    ///
    /// # Arguments
    /// * `trigger_level` - Positive coverage ratio at which the test passes, such as 1.20 for 120%.
    /// * `consequence` - Cash-distribution or reinvestment action while the test is breached.
    pub fn new(trigger_level: f64, consequence: TriggerConsequence) -> Self {
        Self {
            trigger_level,
            cure_level: None,
            breach_date: None,
            consequence,
        }
    }

    /// Set a separate cure threshold and return the updated trigger.
    ///
    /// # Arguments
    /// * `cure_level` - Coverage ratio required to exit breach, at least the breach threshold.
    pub fn with_cure_level(mut self, cure_level: f64) -> Self {
        self.cure_level = Some(cure_level);
        self
    }

    /// Determine breach status, retaining an existing breach until its cure level.
    ///
    /// # Arguments
    /// * `current_level` - Current dimensionless coverage ratio (1.20 means 120%).
    pub fn is_breached(&self, current_level: f64) -> bool {
        if self.breach_date.is_some() {
            !self.is_cured(current_level)
        } else {
            current_level < self.trigger_level
        }
    }

    /// Advance the per-path breach/cure state at a contractual payment date.
    pub(crate) fn update(&mut self, current_level: f64, date: Date) -> bool {
        let breached = self.is_breached(current_level);
        if breached {
            self.breach_date.get_or_insert(date);
        } else {
            self.breach_date = None;
        }
        breached
    }

    /// Return whether current_level has reached the cure threshold.
    ///
    /// When no cure level is configured, the breach threshold itself is used.
    ///
    /// # Arguments
    /// * `current_level` - Current dimensionless coverage ratio, such as 1.25 for 125%.
    pub fn is_cured(&self, current_level: f64) -> bool {
        if let Some(cure) = self.cure_level {
            current_level >= cure
        } else {
            current_level >= self.trigger_level
        }
    }
}

/// Tranche coupon specification
///
/// Supports fixed and floating rate coupons used in standard structured credit instruments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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
    /// For Floating: returns just the spread component (use
    /// `try_current_rate_with_index` for the full projected rate)
    pub fn current_rate(&self, _date: Date) -> f64 {
        match self {
            TrancheCoupon::Fixed { rate } => *rate,
            TrancheCoupon::Floating(spec) => spec.spread_bp.to_f64().unwrap_or_default() / 10_000.0,
        }
    }

    /// Compute current rate including index forward where applicable (fallible).
    ///
    /// This method returns an error if required market data is missing or if the
    /// rate projection fails. Prefer this in pricing/valuation code paths to avoid
    /// silent mispricing.
    pub fn try_current_rate_with_index(
        &self,
        date: Date,
        context: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        let as_of = match self {
            TrancheCoupon::Fixed { .. } => date,
            TrancheCoupon::Floating(spec) => {
                context.get_forward(spec.index_id.as_str())?.base_date()
            }
        };
        self.try_rate_for_period(date, as_of, context)
    }

    /// Resolve the contractual coupon for an explicit accrual period.
    pub fn try_rate_for_period(
        &self,
        accrual_start: Date,
        as_of: Date,
        context: &finstack_quant_core::market_data::context::MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        match self {
            TrancheCoupon::Fixed { rate } => Ok(*rate),
            TrancheCoupon::Floating(spec) => {
                let fwd = context.get_forward(spec.index_id.as_str())?;
                let params = crate::cashflow::builder::FloatingRateParams::try_from(spec)?;
                let calendar_id = spec
                    .fixing_calendar_id
                    .as_deref()
                    .unwrap_or("weekends_only");
                let calendar =
                    finstack_quant_core::dates::calendar_by_id(calendar_id).ok_or_else(|| {
                        finstack_quant_core::Error::Validation(format!(
                            "structured-credit tranche fixing calendar '{}' is not registered",
                            calendar_id
                        ))
                    })?;
                let reset_date = finstack_quant_core::dates::DateExt::add_business_days(
                    accrual_start,
                    -spec.reset_lag_days,
                    calendar,
                )?;
                if reset_date <= as_of {
                    if spec.overnight_compounding.is_some() {
                        return Err(finstack_quant_core::Error::Validation(
                            "seasoned compounded-overnight tranche coupons require a canonical compounded fixing schedule"
                                .into(),
                        ));
                    }
                    let fixings = finstack_quant_core::market_data::fixings::get_fixing_series(
                        context,
                        spec.index_id.as_str(),
                    )?;
                    let raw =
                        finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                            Some(fixings),
                            spec.index_id.as_str(),
                            reset_date,
                            as_of,
                        )?;
                    return Ok(
                        crate::cashflow::builder::rate_helpers::calculate_floating_rate(
                            raw, &params,
                        ),
                    );
                }
                crate::cashflow::builder::project_floating_rate(
                    accrual_start,
                    fwd.as_ref(),
                    &params,
                )
            }
        }
    }
}

/// Structured credit tranche with attachment/detachment points
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Tranche {
    /// Unique tranche identifier
    pub id: InstrumentId,

    /// Lower structural boundary as a percent of the capital structure
    /// (`0.0` for the first-loss class). `None` until
    /// [`TrancheStructure::new`] derives it from the balance shares in
    /// payment-priority order; a declared value must match that share
    /// within the structure's thickness tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_point: Option<f64>,
    /// Upper structural boundary as a percent of the capital structure
    /// (`100.0` for the most senior class); derived like
    /// [`Self::attachment_point`] when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detachment_point: Option<f64>,

    /// Tranche characteristics
    pub seniority: TrancheSeniority,
    /// Credit rating (if rated by agencies)
    pub rating: Option<CreditRating>,

    /// Size and balances
    pub original_balance: Money,
    /// Current outstanding balance (after amortization and losses)
    pub current_balance: Money,
    /// Interest specification
    pub coupon: TrancheCoupon,

    /// Coverage test triggers
    pub oc_trigger: Option<CoverageTrigger>,
    /// Interest coverage trigger specification
    pub ic_trigger: Option<CoverageTrigger>,

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

    /// Whether the coupon is a non-deferrable claim the template pays from
    /// principal proceeds when interest proceeds fall short (and the deal's
    /// `principal_covers_senior_interest` allows it). `None` follows the
    /// seniority convention: senior notes are non-deferrable, every other
    /// class defers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_deferrable: Option<bool>,

    /// Legal final maturity date
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub maturity: Date,

    /// Payment priority (1 = most senior, paid first).
    ///
    /// Derived state, not a wire field: a standalone `Tranche` carries `0`
    /// (unassigned). [`TrancheStructure::new`] (and deserialization of a
    /// `TrancheStructure`) ranks every tranche by structural seniority, input
    /// order breaking ties, so that multiple notes at one `TrancheSeniority`
    /// (e.g. Class A-1/A-2/A-3 all `Senior`) receive distinct priorities
    /// `1..=n`.
    #[serde(skip)]
    pub payment_priority: u32,

    /// Attributes for scenario selection
    pub attributes: Attributes,
}

impl Tranche {
    /// Whether the coupon is a non-deferrable claim: the explicit
    /// `non_deferrable` when set, else `true` for senior notes only.
    #[must_use]
    pub fn is_non_deferrable(&self) -> bool {
        self.non_deferrable
            .unwrap_or(self.seniority == TrancheSeniority::Senior)
    }

    /// Create a new tranche with required fields
    pub fn new(
        id: impl Into<String>,
        attachment_point: f64,
        detachment_point: f64,
        seniority: TrancheSeniority,
        original_balance: Money,
        coupon: TrancheCoupon,
        maturity: Date,
    ) -> finstack_quant_core::Result<Self> {
        if attachment_point < 0.0 || detachment_point <= attachment_point {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }
        if detachment_point > 100.0 {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        Ok(Self {
            id: InstrumentId::new(id.into()),
            attachment_point: Some(attachment_point),
            detachment_point: Some(detachment_point),
            seniority,
            rating: None,
            original_balance,
            current_balance: original_balance,
            coupon,
            oc_trigger: None,
            ic_trigger: None,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            deferred_interest: Money::from((0_i64, original_balance.currency())),
            pik_enabled: false,
            non_deferrable: None,
            maturity,
            // Unassigned until `TrancheStructure::new` ranks the notes.
            payment_priority: 0,
            attributes: Attributes::new(),
        })
    }

    /// Creates a new builder.
    #[must_use]
    pub fn builder() -> TrancheBuilder {
        TrancheBuilder::new()
    }

    /// Create a tranche whose attachment and detachment points are derived
    /// from its balance share when it joins a [`TrancheStructure`].
    ///
    /// # Arguments
    ///
    /// * `id` - Stable tranche identifier.
    /// * `seniority` - Structural seniority that ranks the note in the
    ///   capital structure (and therefore where its derived points sit).
    /// * `original_balance` - Original note balance in the deal currency;
    ///   its share of the structure's total becomes the note's thickness.
    /// * `coupon` - Fixed or floating coupon terms.
    /// * `maturity` - Legal final maturity of the note.
    ///
    /// # Errors
    ///
    /// Returns `InputError::Invalid` when `original_balance` is not positive.
    pub fn from_balance(
        id: impl Into<String>,
        seniority: TrancheSeniority,
        original_balance: Money,
        coupon: TrancheCoupon,
        maturity: Date,
    ) -> finstack_quant_core::Result<Self> {
        if original_balance.amount() <= 0.0 {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }
        let mut tranche = Self::new(
            id,
            0.0,
            100.0,
            seniority,
            original_balance,
            coupon,
            maturity,
        )?;
        tranche.attachment_point = None;
        tranche.detachment_point = None;
        Ok(tranche)
    }

    /// Resolved attachment point in percent (`0.0` for a tranche whose
    /// points have not been derived yet).
    pub fn attachment_pct(&self) -> f64 {
        self.attachment_point.unwrap_or(0.0)
    }

    /// Resolved detachment point in percent (`0.0` for a tranche whose
    /// points have not been derived yet).
    pub fn detachment_pct(&self) -> f64 {
        self.detachment_point.unwrap_or(0.0)
    }

    /// Tranche thickness (detachment - attachment)
    pub fn thickness(&self) -> f64 {
        self.detachment_pct() - self.attachment_pct()
    }

    /// Check if tranche is first loss (attachment at 0%)
    pub fn is_first_loss(&self) -> bool {
        self.attachment_pct() == 0.0
    }

    /// Check if tranche is currently impaired by losses.
    ///
    /// A tranche is impaired when cumulative pool losses, expressed in
    /// percentage points of performing pool balance, exceed the tranche's
    /// attachment point.
    ///
    /// # Arguments
    /// * `cumulative_loss_pct` - Cumulative net loss in percentage points of
    ///   performing pool balance, in `[0, 100]` (for example, `5.0` means 5%).
    pub fn is_impaired(&self, cumulative_loss_pct: f64) -> bool {
        cumulative_loss_pct > self.attachment_pct()
    }

    /// Calculate the dollar loss allocated to this tranche given cumulative pool losses.
    ///
    /// Uses attachment/detachment point convention (INTEX/Moody's Analytics):
    /// - Losses below attachment: zero allocation to this tranche
    /// - Losses between attachment and detachment: proportional allocation
    /// - Losses above detachment: tranche fully impaired (loss = original balance)
    ///
    /// # Arguments
    /// * `cumulative_loss_pct` - Cumulative net loss in percentage points of
    ///   performing pool balance, in `[0, 100]` (for example, `12.0` means 12%).
    pub fn loss_allocation(&self, cumulative_loss_pct: f64) -> Money {
        if cumulative_loss_pct <= self.attachment_pct() {
            // Losses have not reached this tranche's subordination
            Money::from((0_i64, self.original_balance.currency()))
        } else if cumulative_loss_pct >= self.detachment_pct() {
            // Tranche fully impaired — written down to zero
            self.original_balance
        } else {
            // Partial loss: only the portion between attachment and detachment
            let loss_within_tranche_pct = cumulative_loss_pct - self.attachment_pct();
            let loss_rate = loss_within_tranche_pct / self.thickness();
            self.original_balance * loss_rate
        }
    }

    /// Current tranche balance after applying cumulative losses.
    ///
    /// Returns `max(0, current_balance - loss_allocation)`.
    ///
    /// # Arguments
    ///
    /// * `cumulative_loss_pct` - Cumulative net loss in percentage points of
    ///   performing pool balance, in `[0, 100]` (for example, `12.0` means 12%).
    pub fn current_balance_after_losses(
        &self,
        cumulative_loss_pct: f64,
    ) -> finstack_quant_core::Result<Money> {
        let loss_amount = self.loss_allocation(cumulative_loss_pct);
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
    non_deferrable: Option<bool>,
    current_balance: Option<Money>,
    deferred_interest: Option<Money>,
    oc_trigger: Option<CoverageTrigger>,
    ic_trigger: Option<CoverageTrigger>,
    attributes: Option<Attributes>,
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
            non_deferrable: None,
            current_balance: None,
            deferred_interest: None,
            oc_trigger: None,
            ic_trigger: None,
            attributes: None,
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
    pub fn frequency(mut self, frequency: Tenor) -> Self {
        self.frequency = frequency;
        self
    }

    /// Set day count convention
    #[must_use]
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    /// Enable PIK (payment-in-kind) accretion for interest shortfalls.
    #[must_use]
    pub fn pik_enabled(mut self, enabled: bool) -> Self {
        self.pik_enabled = enabled;
        self
    }

    /// Mark the coupon non-deferrable (paid from principal proceeds when
    /// interest falls short) or deferrable, overriding the seniority
    /// convention.
    ///
    /// # Arguments
    ///
    /// * `non_deferrable` - `true` for a coupon the trust must pay in full
    ///   every period, `false` for one that defers.
    #[must_use]
    pub fn non_deferrable(mut self, non_deferrable: bool) -> Self {
        self.non_deferrable = Some(non_deferrable);
        self
    }

    /// Set the current (factored) balance; defaults to the original balance.
    ///
    /// # Arguments
    ///
    /// * `balance` - Outstanding principal today, in the tranche currency,
    ///   at most the original balance.
    #[must_use]
    pub fn current_balance(mut self, balance: Money) -> Self {
        self.current_balance = Some(balance);
        self
    }

    /// Set interest already deferred (unpaid, still owed) at closing.
    ///
    /// # Arguments
    ///
    /// * `amount` - Deferred interest carried into the projection, in the
    ///   tranche currency; zero when omitted.
    #[must_use]
    pub fn deferred_interest(mut self, amount: Money) -> Self {
        self.deferred_interest = Some(amount);
        self
    }

    /// Attach a per-tranche overcollateralization trigger.
    ///
    /// # Arguments
    ///
    /// * `trigger` - Breach/cure levels and the consequence applied while
    ///   breached.
    #[must_use]
    pub fn oc_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.oc_trigger = Some(trigger);
        self
    }

    /// Attach a per-tranche interest-coverage trigger.
    ///
    /// # Arguments
    ///
    /// * `trigger` - Breach/cure levels and the consequence applied while
    ///   breached.
    #[must_use]
    pub fn ic_trigger(mut self, trigger: CoverageTrigger) -> Self {
        self.ic_trigger = Some(trigger);
        self
    }

    /// Set free-form attributes (tags and metadata) on the tranche.
    ///
    /// # Arguments
    ///
    /// * `attributes` - Attribute bag replacing the empty default.
    #[must_use]
    pub fn attributes(mut self, attributes: Attributes) -> Self {
        self.attributes = Some(attributes);
        self
    }

    /// Build the tranche with validation.
    ///
    /// Attachment and detachment points may both be omitted, in which case
    /// [`TrancheStructure::new`] derives them from the balance shares;
    /// supplying only one of the two is an error.
    pub fn build(self) -> finstack_quant_core::Result<Tranche> {
        let id = self.id.ok_or(finstack_quant_core::InputError::Invalid)?;
        let points = match (self.attachment_point, self.detachment_point) {
            (Some(attachment), Some(detachment)) => Some((attachment, detachment)),
            (None, None) => None,
            _ => return Err(finstack_quant_core::InputError::Invalid.into()),
        };
        let seniority = self
            .seniority
            .ok_or(finstack_quant_core::InputError::Invalid)?;
        let original_balance = self
            .original_balance
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        if original_balance.amount() <= 0.0 {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let coupon = self
            .coupon
            .ok_or(finstack_quant_core::InputError::Invalid)?;
        let maturity = self
            .maturity
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        let mut tranche = match points {
            Some((attachment_point, detachment_point)) => Tranche::new(
                id,
                attachment_point,
                detachment_point,
                seniority,
                original_balance,
                coupon,
                maturity,
            )?,
            None => Tranche::from_balance(id, seniority, original_balance, coupon, maturity)?,
        };

        if let Some(rating) = self.rating {
            tranche = tranche.with_rating(rating);
        }

        tranche.frequency = self.frequency;
        tranche.day_count = self.day_count;
        tranche.pik_enabled = self.pik_enabled;
        tranche.non_deferrable = self.non_deferrable;
        if let Some(balance) = self.current_balance {
            tranche.current_balance = balance;
        }
        if let Some(deferred) = self.deferred_interest {
            tranche.deferred_interest = deferred;
        }
        tranche.oc_trigger = self.oc_trigger;
        tranche.ic_trigger = self.ic_trigger;
        if let Some(attributes) = self.attributes {
            tranche.attributes = attributes;
        }

        Ok(tranche)
    }
}

impl Default for TrancheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of tranches forming the capital structure
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct TrancheStructure {
    /// Ordered tranches (typically sorted by payment priority)
    pub tranches: Vec<Tranche>,
    /// Common currency of every tranche, fixed at assembly.
    #[serde(skip)]
    currency: Currency,
}

/// Deserialize a `TrancheStructure` through the same validation +
/// `payment_priority` assignment path as [`TrancheStructure::new`].
///
/// `payment_priority` is not a wire field: it is derived state, assigned
/// here from structural seniority exactly as the builder path assigns it.
impl<'de> Deserialize<'de> for TrancheStructure {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawTrancheStructure {
            tranches: Vec<Tranche>,
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
    ///
    /// # Arguments
    ///
    /// * `tranches` - Ordered notes that form the capital structure; must be non-empty
    ///   and pass structural validation before payment priorities are assigned.
    pub fn new(mut tranches: Vec<Tranche>) -> finstack_quant_core::Result<Self> {
        if tranches.is_empty() {
            return Err(finstack_quant_core::InputError::TooFewPoints.into());
        }

        // Assign deterministic, distinct payment priorities by structural rank.
        Self::assign_priorities(&mut tranches);

        // Notes built without points take the balance-share boundaries;
        // declared points are then validated against those same shares.
        Self::resolve_attachment_points(&mut tranches)?;

        Self::validate_structure(&tranches)?;

        let currency = tranches[0].original_balance.currency();
        Ok(Self { tranches, currency })
    }

    /// Currency shared by every tranche in the structure.
    #[inline]
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Total original balance of all tranches combined.
    ///
    /// # Errors
    ///
    /// Returns a currency-mismatch error if a tranche was mutated into a
    /// different currency after assembly.
    pub fn total_size(&self) -> finstack_quant_core::Result<Money> {
        self.tranches
            .iter()
            .try_fold(Money::from((0_i64, self.currency)), |acc, t| {
                acc.checked_add(t.original_balance)
            })
    }

    /// Create a structure whose attachment and detachment points come from
    /// the balance shares alone: any declared points are discarded, the
    /// first-loss class attaches at 0 and the most senior class detaches at
    /// 100, in payment-priority order.
    ///
    /// # Arguments
    ///
    /// * `tranches` - Notes of the capital structure; their `seniority` and
    ///   `original_balance` decide the derived boundaries.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::new`] (empty structure, mixed
    /// currencies, non-positive balances).
    pub fn from_balances(mut tranches: Vec<Tranche>) -> finstack_quant_core::Result<Self> {
        for tranche in &mut tranches {
            tranche.attachment_point = None;
            tranche.detachment_point = None;
        }
        Self::new(tranches)
    }

    /// Balance-share boundaries per tranche in the input order: the most
    /// junior note (highest `payment_priority`) attaches at 0 and each senior
    /// note stacks on top; the most senior note detaches at exactly 100.
    fn derived_attachment_points(
        tranches: &[Tranche],
    ) -> finstack_quant_core::Result<Vec<(f64, f64)>> {
        let total: f64 = tranches.iter().map(|t| t.original_balance.amount()).sum();
        if !total.is_finite() || total <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "tranche structure needs a positive total original balance to derive attachment points".to_string(),
            ));
        }
        let mut order: Vec<usize> = (0..tranches.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(tranches[i].payment_priority));
        let mut points = vec![(0.0, 0.0); tranches.len()];
        let mut cumulative = 0.0;
        let last = order.len() - 1;
        for (rank, &index) in order.iter().enumerate() {
            let share = tranches[index].original_balance.amount() / total * 100.0;
            let detachment = if rank == last {
                100.0
            } else {
                cumulative + share
            };
            points[index] = (cumulative, detachment);
            cumulative = detachment;
        }
        Ok(points)
    }

    /// Fill the attachment/detachment points of notes that were built
    /// without them from the balance shares.
    fn resolve_attachment_points(tranches: &mut [Tranche]) -> finstack_quant_core::Result<()> {
        if tranches
            .iter()
            .all(|t| t.attachment_point.is_some() && t.detachment_point.is_some())
        {
            return Ok(());
        }
        let derived = Self::derived_attachment_points(tranches)?;
        for (tranche, (attachment, detachment)) in tranches.iter_mut().zip(derived) {
            if tranche.attachment_point.is_none() || tranche.detachment_point.is_none() {
                tranche.attachment_point = Some(attachment);
                tranche.detachment_point = Some(detachment);
            }
        }
        Ok(())
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
    fn validate_structure(tranches: &[Tranche]) -> finstack_quant_core::Result<()> {
        // Validate attachment points are resolved and finite before sorting
        for tranche in tranches {
            if !tranche.attachment_pct().is_finite() || !tranche.detachment_pct().is_finite() {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
        }

        // Sort by attachment point for validation
        let mut sorted_tranches = tranches.to_vec();
        sorted_tranches.sort_by(|a, b| a.attachment_pct().total_cmp(&b.attachment_pct()));

        let mut expected_attachment = 0.0;
        const TOLERANCE: f64 = 1e-6;

        for tranche in &sorted_tranches {
            if (tranche.attachment_pct() - expected_attachment).abs() > TOLERANCE {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            if tranche.detachment_pct() <= tranche.attachment_pct() {
                return Err(finstack_quant_core::InputError::Invalid.into());
            }
            expected_attachment = tranche.detachment_pct();
        }

        // Should reach 100%
        if (expected_attachment - 100.0).abs() > TOLERANCE {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        // Reconcile declared tranche thickness against the actual balance
        // share of the capital structure. The waterfall allocates cash and
        // losses off balances while attachment/detachment points are consumed
        // by the stochastic pricer's reporting (thickness, loss multiple), so
        // a deal whose declared points do not match its balances would price
        // and report inconsistently. The tolerance allows the rounded
        // percentages deals quote (e.g. 63.5% for 63.478...%) while rejecting
        // structurally inconsistent inputs.
        const THICKNESS_TOLERANCE_PCT: f64 = 0.5;
        let total_balance: f64 = tranches.iter().map(|t| t.original_balance.amount()).sum();
        if total_balance > 0.0 {
            for tranche in tranches {
                let balance_share_pct = tranche.original_balance.amount() / total_balance * 100.0;
                let thickness_pct = tranche.thickness();
                if (balance_share_pct - thickness_pct).abs() > THICKNESS_TOLERANCE_PCT {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "tranche '{}' declares attachment/detachment [{}, {}] \
                         (thickness {:.4}%) but its balance is {:.4}% of the \
                         capital structure; declared points must match balance \
                         shares within {THICKNESS_TOLERANCE_PCT}%",
                        tranche.id,
                        tranche.attachment_pct(),
                        tranche.detachment_pct(),
                        thickness_pct,
                        balance_share_pct,
                    )));
                }
            }
        }

        let base_currency = tranches[0].original_balance.currency();
        for tranche in tranches {
            if tranche.original_balance.currency() != base_currency {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
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
            .try_fold(Money::from((0_i64, self.currency)), |acc, t| {
                acc.checked_add(t.current_balance)
            })
            .unwrap_or_else(|_| Money::from((0_i64, self.currency)))
    }

    /// Calculate tranche subordination amount
    pub fn subordination_amount(&self, tranche_id: &str) -> Money {
        let target_tranche = self.tranches.iter().find(|t| t.id.as_str() == tranche_id);

        if let Some(target) = target_tranche {
            self.tranches
                .iter()
                .filter(|t| t.payment_priority > target.payment_priority)
                .try_fold(Money::from((0_i64, self.currency)), |acc, t| {
                    acc.checked_add(t.current_balance)
                })
                .unwrap_or_else(|_| Money::from((0_i64, self.currency)))
        } else {
            Money::from((0_i64, self.currency))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
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
            Money::from((100_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.12 },
            test_date(),
        )
        .expect("should succeed");

        assert_eq!(tranche.attachment_point, Some(0.0));
        assert_eq!(tranche.detachment_point, Some(10.0));
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
            Money::from((50_000_000_i64, Currency::USD)),
            TrancheCoupon::Fixed { rate: 0.08 },
            test_date(),
        )
        .expect("should succeed");

        // No loss case
        let loss = tranche.loss_allocation(5.0);
        assert_eq!(loss.amount(), 0.0);

        // Partial loss case (12% cumulative loss)
        let loss = tranche.loss_allocation(12.0);
        assert!(loss.amount() > 0.0);
        assert!(loss.amount() < tranche.original_balance.amount());

        // Full loss case (20% cumulative loss)
        let loss = tranche.loss_allocation(20.0);
        assert_eq!(loss.amount(), tranche.original_balance.amount());
    }

    #[test]
    fn test_tranche_structure_validation() {
        let equity = Tranche::builder()
            .id("EQUITY")
            .attachment_detachment(0.0, 10.0)
            .seniority(TrancheSeniority::Equity)
            .balance(Money::from((100_000_000_i64, Currency::USD)))
            .coupon(TrancheCoupon::Fixed { rate: 0.12 })
            .maturity(test_date())
            .build()
            .expect("should succeed");

        let senior = Tranche::builder()
            .id("SENIOR")
            .attachment_detachment(10.0, 100.0)
            .seniority(TrancheSeniority::Senior)
            .balance(Money::from((900_000_000_i64, Currency::USD)))
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
                    overnight_index_constraints: Default::default(),
                    reset_frequency: finstack_quant_core::dates::Tenor::quarterly(),
                    index_tenor: None,
                    reset_lag_days: 2,
                    fixing_calendar_id: None,
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
        assert_eq!(
            structure.total_size().expect("total").amount(),
            1_000_000_000.0
        );
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

    #[test]
    fn production_waterfall_breached_trigger_requires_cure_threshold() {
        let mut trigger =
            CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow).with_cure_level(1.25);
        trigger.breach_date = Some(test_date());
        assert!(
            trigger.is_breached(1.22),
            "an existing breach persists until the cure threshold"
        );
        assert!(!trigger.is_breached(1.25));
    }
}
