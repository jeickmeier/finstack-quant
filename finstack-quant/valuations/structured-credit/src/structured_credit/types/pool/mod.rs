//! Asset pool structures for structured credit instruments.

mod analytics;
mod asset;
mod config;
#[cfg(test)]
mod tests;

pub use analytics::calculate_pool_stats;
pub(crate) use analytics::weighted_avg_rating_factor;
pub use asset::PoolAsset;
pub use config::{
    PoolAccounts, PoolStats, ReinvestmentAssumptions, ReinvestmentCriteria, ReinvestmentPeriod,
    RepLine,
};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_fixed_income::bond::Bond;
use serde::{Deserialize, Serialize};

use super::collateral::{InstrumentCollateral, ReserveInterestDestination};
use super::enums::DealType;
use super::tranches::TrancheStructure;

/// Main asset pool structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AssetPool {
    /// AssetPool identifier
    pub id: InstrumentId,

    /// Deal type classification
    pub deal_type: DealType,

    /// Base currency for every asset and pool-level account.
    pub currency: Currency,

    /// Underlying assets
    pub assets: Vec<PoolAsset>,

    /// Performance tracking
    /// Cumulative defaults to date
    pub cumulative_defaults: Money,
    /// Cumulative recoveries on defaulted assets
    pub cumulative_recoveries: Money,
    /// Cumulative prepayments (voluntary early repayment)
    pub cumulative_prepayments: Money,
    /// Cumulative scheduled amortization (level-pay principal for amortizing assets).
    ///
    /// Part of the reconstructed original balance
    /// ([`AssetPool::original_balance_or_reconstructed`]) when no explicit
    /// `original_balance` is supplied.
    pub cumulative_scheduled_amortization: Money,

    /// Original (cut-off) pool balance the deal's cumulative-loss triggers,
    /// clean-up call factor and cumulative-loss/timing default curves are
    /// stated against. `None` reconstructs it as the current balance plus
    /// `cumulative_defaults`, `cumulative_prepayments` and
    /// `cumulative_scheduled_amortization` (see
    /// [`Self::original_balance_or_reconstructed`]); supply it when those
    /// tallies are incomplete. Must be at least the current total balance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_balance: Option<Money>,

    /// Reinvestment management
    /// Reinvestment period configuration (if applicable)
    pub reinvestment_period: Option<ReinvestmentPeriod>,

    /// AssetPool-level accounts
    /// Collection account balance (collected but not yet distributed)
    pub collection_account: Money,
    /// Reserve account balance (for credit enhancement)
    pub reserve_account: Money,
    /// Excess spread account (accumulated excess interest)
    pub excess_spread_account: Money,

    /// Aggregated representative lines (optional optimization)
    /// Must be used with an empty `assets` vector; normalized into the same engine.
    pub rep_lines: Option<Vec<RepLine>>,

    /// Real instruments held as collateral (bonds, term loans, revolvers).
    /// Must be used with an empty `assets` vector and no `rep_lines`; the
    /// simulation engine drives period flows from the instruments' own
    /// schedules and materializes them into asset rows for every balance
    /// consumer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruments: Option<InstrumentCollateral>,

    /// Annual simple interest rate (decimal, ACT/360 on the opening balance
    /// per legal period) earned by `reserve_account`. Defaults to `0.0`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub reserve_account_rate: f64,

    /// Target reserve balance that revolver repayments replenish toward
    /// before counting as principal collections. `None` disables
    /// replenishment from repayments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve_target: Option<Money>,

    /// Where the interest earned on `reserve_account` is paid.
    #[serde(
        default,
        skip_serializing_if = "ReserveInterestDestination::is_default"
    )]
    pub reserve_interest_destination: ReserveInterestDestination,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

/// Check that a collateral row carries a non-negative `reset_lag_days`
/// exactly when it floats on a `forward_curve_id`.
///
/// # Arguments
///
/// * `row` - Row identifier quoted in the error message.
/// * `forward_curve_id` - The row's index curve; `None` for a fixed coupon.
/// * `reset_lag_days` - The row's business-day fixing lag.
pub(crate) fn validate_reset_lag(
    row: &str,
    forward_curve_id: Option<&str>,
    reset_lag_days: Option<i32>,
) -> finstack_quant_core::Result<()> {
    match (forward_curve_id, reset_lag_days) {
        (Some(_), Some(lag)) if lag < 0 => Err(finstack_quant_core::Error::Validation(format!(
            "collateral row '{row}' has a negative reset_lag_days ({lag}); use 0 to fix on the \
             period start or a positive number of business days before it"
        ))),
        (Some(index), None) => Err(finstack_quant_core::Error::Validation(format!(
            "floating collateral row '{row}' on '{index}' requires reset_lag_days"
        ))),
        (None, Some(_)) => Err(finstack_quant_core::Error::Validation(format!(
            "fixed-rate collateral row '{row}' must not set reset_lag_days"
        ))),
        _ => Ok(()),
    }
}

/// The business-day fixing lag of a floating collateral row.
///
/// # Arguments
///
/// * `row` - Row identifier quoted in the error message.
/// * `reset_lag_days` - The row's lag; `None` is a validation error.
pub(crate) fn require_reset_lag(
    row: &str,
    reset_lag_days: Option<i32>,
) -> finstack_quant_core::Result<i32> {
    reset_lag_days.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "floating collateral row '{row}' requires reset_lag_days"
        ))
    })
}

impl AssetPool {
    /// Create new asset pool
    pub fn new(id: impl Into<InstrumentId>, deal_type: DealType, currency: Currency) -> Self {
        let zero_money = Money::from((0_i64, currency));
        Self {
            id: id.into(),
            deal_type,
            currency,
            assets: Vec::new(),
            cumulative_defaults: zero_money,
            cumulative_recoveries: zero_money,
            cumulative_prepayments: zero_money,
            cumulative_scheduled_amortization: zero_money,
            original_balance: None,
            reinvestment_period: None,
            collection_account: zero_money,
            reserve_account: zero_money,
            excess_spread_account: zero_money,
            rep_lines: None,
            instruments: None,
            reserve_account_rate: 0.0,
            reserve_target: None,
            reserve_interest_destination: ReserveInterestDestination::Waterfall,
        }
    }

    /// Set the pool's account balances and performance tallies, returning the
    /// updated pool.
    ///
    /// # Arguments
    ///
    /// * `accounts` - New values, in the pool currency; a `None` field keeps
    ///   the pool's current value.
    #[must_use]
    pub fn with_accounts(mut self, accounts: PoolAccounts) -> Self {
        if let Some(value) = accounts.original_balance {
            self.original_balance = Some(value);
        }
        let tallies = [
            (accounts.cumulative_defaults, &mut self.cumulative_defaults),
            (
                accounts.cumulative_recoveries,
                &mut self.cumulative_recoveries,
            ),
            (
                accounts.cumulative_prepayments,
                &mut self.cumulative_prepayments,
            ),
            (
                accounts.cumulative_scheduled_amortization,
                &mut self.cumulative_scheduled_amortization,
            ),
            (accounts.collection_account, &mut self.collection_account),
            (
                accounts.excess_spread_account,
                &mut self.excess_spread_account,
            ),
        ];
        for (value, slot) in tallies {
            if let Some(value) = value {
                *slot = value;
            }
        }
        self
    }

    /// Configure the pool's reserve account, returning the updated pool.
    ///
    /// Replaces all four reserve fields at once, so a field left out falls back
    /// to its default rather than keeping a previous value.
    ///
    /// # Arguments
    ///
    /// * `reserve_account` - Opening reserve balance, in the pool currency.
    /// * `reserve_account_rate` - Annual simple interest rate earned on the
    ///   reserve, as a decimal (`0.03` = 3%), ACT/360 on the opening balance per
    ///   legal period.
    /// * `reserve_target` - Target balance that revolver repayments replenish
    ///   toward before counting as principal collections; `None` disables
    ///   replenishment from repayments.
    /// * `reserve_interest_destination` - Where reserve interest is paid;
    ///   `None` selects [`ReserveInterestDestination::default`] (the waterfall).
    ///
    /// # Returns
    ///
    /// The pool with the reserve fields set; every other field is unchanged.
    #[must_use]
    pub fn with_reserve(
        mut self,
        reserve_account: Money,
        reserve_account_rate: f64,
        reserve_target: Option<Money>,
        reserve_interest_destination: Option<ReserveInterestDestination>,
    ) -> Self {
        self.reserve_account = reserve_account;
        self.reserve_account_rate = reserve_account_rate;
        self.reserve_target = reserve_target;
        self.reserve_interest_destination = reserve_interest_destination.unwrap_or_default();
        self
    }

    /// Add asset from existing bond
    pub fn add_bond(
        &mut self,
        bond: &Bond,
        industry: Option<String>,
    ) -> finstack_quant_core::Result<&mut Self> {
        let asset = PoolAsset::from_bond(bond, industry)?;
        self.assets.push(asset);
        Ok(self)
    }

    /// Normalize representative lines or instrument collateral into the
    /// canonical asset rows the simulation engine consumes.
    ///
    /// Representative lines are expanded one row per line; instrument
    /// collateral is validated against the pool base currency and
    /// materialized by `InstrumentCollateral::materialize`, with the
    /// instruments themselves retained on the returned pool. A pool that
    /// already holds `assets` is validated and returned unchanged.
    ///
    /// # Arguments
    ///
    /// * `closing_date` - Deal closing date used to age representative
    ///   lines (`seasoning_months`) and to count delayed draws already funded.
    ///
    /// # Errors
    ///
    /// Returns a validation error when more than one representation is
    /// populated, when an override or rate is out of range, or when an
    /// instrument fails its own validation or currency check.
    pub fn normalized(&self, closing_date: Date) -> finstack_quant_core::Result<Self> {
        self.validate_representation()?;
        let mut pool = self.clone();
        if let Some(lines) = pool.rep_lines.take() {
            for line in lines {
                for (name, value) in [
                    ("CPR", line.cpr),
                    ("CDR", line.cdr),
                    ("recovery", line.recovery_rate),
                ] {
                    if let Some(value) = value {
                        finstack_quant_core::validation::validate_f64_unit_interval(value, name)?;
                    }
                }
                if line.seasoning_months > Date::MIN.months_until(closing_date)? {
                    return Err(finstack_quant_core::Error::Validation(
                        "representative seasoning precedes the supported date range".into(),
                    ));
                }
                let months = i32::try_from(line.seasoning_months).map_err(|_| {
                    finstack_quant_core::Error::Validation(
                        "representative seasoning exceeds the supported date range".into(),
                    )
                })?;
                let origination = closing_date.add_months(-months)?;
                pool.assets.push(PoolAsset {
                    id: line.id.into(),
                    asset_type: line.asset_type,
                    balance: line.balance,
                    rate: line.rate,
                    spread_bp: line.spread_bp,
                    forward_curve_id: line.forward_curve_id,
                    index_floor_bp: line.index_floor_bp,
                    reset_lag_days: line.reset_lag_days,
                    maturity: line.maturity,
                    rating: None,
                    industry: None,
                    obligor_id: None,
                    defaulted: false,
                    recovery_amount: None,
                    default_date: None,
                    purchase_price: None,
                    acquisition_date: None,
                    origination_date: Some(origination),
                    day_count: line.day_count,
                    smm_override: line
                        .cpr
                        .map(finstack_quant_cashflows::builder::annual_to_monthly)
                        .transpose()?,
                    mdr_override: line
                        .cdr
                        .map(finstack_quant_cashflows::builder::annual_to_monthly)
                        .transpose()?,
                    recovery_rate: line.recovery_rate,
                    commitment: None,
                    contractual_payment: line.contractual_payment,
                    amortization_term_months: line.amortization_term_months,
                    io_months: line.io_months,
                    market_price_pct: None,
                    delinquency_buckets: None,
                    balloon: None,
                    prepayment_penalty: None,
                    special_servicing: None,
                    noi: None,
                    liquidation: None,
                });
            }
        }
        if let Some(instruments) = pool.instruments.as_ref() {
            instruments.validate(self.currency)?;
            pool.assets = instruments.materialize(closing_date)?;
        }
        pool.validate_rows()?;
        Ok(pool)
    }

    /// [`Self::normalized`] without copying a pool that already holds its
    /// rows as `assets`: borrowed after the same validation, owned when
    /// representative lines or instrument collateral must be expanded.
    ///
    /// # Arguments
    ///
    /// * `closing_date` - Deal closing date, as for [`Self::normalized`].
    ///
    /// # Errors
    ///
    /// The errors of [`Self::normalized`].
    pub(crate) fn normalized_view(
        &self,
        closing_date: Date,
    ) -> finstack_quant_core::Result<std::borrow::Cow<'_, Self>> {
        if self.rep_lines.is_none() && self.instruments.is_none() {
            self.validate_representation()?;
            self.validate_rows()?;
            return Ok(std::borrow::Cow::Borrowed(self));
        }
        self.normalized(closing_date).map(std::borrow::Cow::Owned)
    }

    /// Currency and override-range checks on every asset row.
    fn validate_rows(&self) -> finstack_quant_core::Result<()> {
        for asset in &self.assets {
            self.validate_asset_currency(asset)?;
            validate_reset_lag(
                asset.id.as_str(),
                asset.forward_curve_id.as_deref(),
                asset.reset_lag_days,
            )?;
            for (name, value) in [
                ("SMM", asset.smm_override),
                ("MDR", asset.mdr_override),
                ("recovery", asset.recovery_rate),
            ] {
                if let Some(value) = value {
                    finstack_quant_core::validation::validate_f64_unit_interval(value, name)?;
                }
            }
        }
        Ok(())
    }

    fn validate_representation(&self) -> finstack_quant_core::Result<()> {
        let has_assets = !self.assets.is_empty();
        let has_rep_lines = self
            .rep_lines
            .as_ref()
            .is_some_and(|lines| !lines.is_empty());
        let has_instruments = self
            .instruments
            .as_ref()
            .is_some_and(|collateral| !collateral.is_empty());
        if has_instruments && self.reinvestment_period.is_some() {
            return Err(finstack_quant_core::Error::Validation(
                "reinvestment_period is not supported for instrument collateral: recycled \
                 principal cannot be placed into schedule-driven instruments"
                    .into(),
            ));
        }
        if has_rep_lines && (has_assets || has_instruments) {
            return Err(finstack_quant_core::Error::Validation(
                "asset pool must contain exactly one of assets, representative lines or \
                 instruments"
                    .into(),
            ));
        }
        // Assets may coexist with instruments only as their materialization
        // (same ids, same count), which is what `normalized` produces.
        if let Some(collateral) = self
            .instruments
            .as_ref()
            .filter(|_| has_assets && has_instruments)
        {
            let aligned = self.assets.len() == collateral.len()
                && self
                    .assets
                    .iter()
                    .zip(collateral.iter())
                    .all(|(asset, held)| &asset.id == held.id());
            if !aligned {
                return Err(finstack_quant_core::Error::Validation(
                    "asset pool must contain exactly one of assets, representative lines or \
                     instruments; assets alongside instruments must be their materialized rows"
                        .into(),
                ));
            }
        }
        Ok(())
    }

    /// Validate the reserve-account configuration against the deal tranches.
    ///
    /// # Arguments
    ///
    /// * `tranches` - Deal capital structure; a `Tranche` interest destination
    ///   must name one of its tranches.
    pub fn validate_reserve_config(
        &self,
        tranches: &TrancheStructure,
    ) -> finstack_quant_core::Result<()> {
        if !self.reserve_account_rate.is_finite() || self.reserve_account_rate < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "reserve_account_rate must be a finite non-negative decimal, got {}",
                self.reserve_account_rate
            )));
        }
        if let Some(target) = self.reserve_target
            && (target.currency() != self.currency || target.amount() < 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "reserve_target must be a non-negative {} amount, got {target}",
                self.currency
            )));
        }
        if let ReserveInterestDestination::Tranche { tranche_id } =
            &self.reserve_interest_destination
            && !tranches
                .tranches
                .iter()
                .any(|t| t.id.as_str() == tranche_id)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "reserve_interest_destination names unknown tranche '{tranche_id}'"
            )));
        }
        Ok(())
    }

    /// An explicit original balance must be a finite amount in the pool
    /// currency and at least the current total balance.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` (or `CurrencyMismatch`) when it is not.
    pub fn validate_original_balance(&self) -> finstack_quant_core::Result<()> {
        let Some(original) = self.original_balance else {
            return Ok(());
        };
        if original.currency() != self.currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.currency,
                actual: original.currency(),
            });
        }
        let current = self.total_balance()?;
        if !original.amount().is_finite() || original.amount() + 1e-6 < current.amount() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "pool original_balance {} must be finite and at least the current balance {}",
                original.amount(),
                current.amount()
            )));
        }
        Ok(())
    }

    fn validate_asset_currency(&self, asset: &PoolAsset) -> finstack_quant_core::Result<()> {
        let actual = asset.balance.currency();
        if actual != self.currency {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.currency,
                actual,
            });
        }
        if let Some(payment) = asset.contractual_payment
            && payment.currency() != self.currency
        {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: self.currency,
                actual: payment.currency(),
            });
        }
        Ok(())
    }
}
