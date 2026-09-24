//! Coverage-test declarations: the collateral valuation rules and the OC,
//! IC and borrowing-base tests placed as waterfall positions.

use super::*;

/// Collateral valuation rules for the OC tests: rating haircuts, the value
/// carried for defaulted collateral, the excess-CCC bucket and discount
/// obligations (CLO indenture conventions). Percentages are percent values
/// (`7.5` = 7.5%); haircuts are decimal fractions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CoverageRules {
    /// Haircut applied to the par of performing collateral by rating bucket,
    /// as a decimal fraction (`0.5` carries the asset at half par). Ratings
    /// are looked up by their letter bucket (`B+`, `B` and `B-` all read the
    /// `B` entry, see `CreditRating::bucket`). An `NR` entry applies to
    /// unrated asset rows supplied by the caller; reinvestment purchases and
    /// materialized instrument collateral are unrated by construction and
    /// stay at par. Empty (the indenture par-value convention) by default.
    #[serde(default)]
    pub rating_haircuts: BTreeMap<CreditRating, f64>,
    /// Value carried for defaulted collateral whose recovery cash has not yet
    /// arrived.
    #[serde(default)]
    pub defaulted_valuation: DefaultedValuation,
    /// Excess-CCC bucket: collateral rated CCC+ and below beyond
    /// `threshold_pct` of the performing pool is carried at market value or
    /// excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccc_bucket: Option<CccBucketRule>,
    /// Discount obligations: collateral bought below `price_threshold_pct` of
    /// par is carried at its purchase price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discount_obligation: Option<DiscountObligationRule>,
    /// Advance rates and concentration limits evaluated by
    /// `CoverageTestType::BorrowingBase` tests; `None` makes such a test a
    /// validation error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub borrowing_base: Option<super::super::borrowing_base::BorrowingBaseRules>,
}

/// How defaulted collateral enters the OC numerator until its recovery cash
/// arrives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DefaultedValuation {
    /// At the modeled recovery value of the pending claims (the default).
    #[default]
    Recovery,
    /// At `pct` percent of the defaulted par (a market-value convention).
    MarketValue {
        /// Percent of defaulted par carried (`40.0` = 40%).
        pct: f64,
    },
}

/// Excess-CCC bucket rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CccBucketRule {
    /// Share of the performing pool, in percent, that CCC+ and lower rated
    /// collateral may occupy at par (`7.5` = 7.5%).
    pub threshold_pct: f64,
    /// `true` carries the excess at the assets' `market_price_pct` (assets
    /// without a price stay at par); `false` excludes the excess entirely.
    pub carry_at_market_value: bool,
}

/// Discount-obligation rule.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscountObligationRule {
    /// Purchase price, in percent of par, below which an asset is a discount
    /// obligation carried at its purchase price (`80.0` = 80% of par).
    pub price_threshold_pct: f64,
}

impl CoverageRules {
    /// Standard CLO par-value test rules: performing collateral at par (no
    /// rating haircuts), defaulted collateral at recovery, a 7.5% CCC bucket
    /// carried at market value and an 80% discount-obligation threshold.
    #[must_use]
    pub fn clo_standard() -> Self {
        Self {
            rating_haircuts: BTreeMap::new(),
            defaulted_valuation: DefaultedValuation::Recovery,
            ccc_bucket: Some(CccBucketRule {
                threshold_pct: 7.5,
                carry_at_market_value: true,
            }),
            discount_obligation: Some(DiscountObligationRule {
                price_threshold_pct: 80.0,
            }),
            borrowing_base: None,
        }
    }

    /// Whether the rules leave every test at plain par with defaulted
    /// collateral at recovery.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rating_haircuts.is_empty()
            && self.defaulted_valuation == DefaultedValuation::Recovery
            && self.ccc_bucket.is_none()
            && self.discount_obligation.is_none()
            && self.borrowing_base.is_none()
    }

    /// Whether any rule changes the value of performing collateral.
    #[must_use]
    pub fn adjusts_collateral(&self) -> bool {
        !self.rating_haircuts.is_empty()
            || self.ccc_bucket.is_some()
            || self.discount_obligation.is_some()
    }

    /// Reject non-finite or out-of-range parameters.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when a haircut is outside `[0, 1]`, a
    /// market-value or threshold percent is outside `[0, 100]`, or any value
    /// is non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if let Some(rules) = &self.borrowing_base {
            rules.validate()?;
        }
        let invalid = |msg: &str| finstack_quant_core::Error::Validation(msg.to_string());
        if self
            .rating_haircuts
            .values()
            .any(|h| !h.is_finite() || !(0.0..=1.0).contains(h))
        {
            return Err(invalid(
                "coverage rating haircuts must be decimal fractions in [0, 1]",
            ));
        }
        if let DefaultedValuation::MarketValue { pct } = self.defaulted_valuation {
            if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
                return Err(invalid(
                    "defaulted market value must be a percent in [0, 100]",
                ));
            }
        }
        if let Some(bucket) = self.ccc_bucket {
            if !bucket.threshold_pct.is_finite() || !(0.0..=100.0).contains(&bucket.threshold_pct) {
                return Err(invalid(
                    "CCC bucket threshold must be a percent in [0, 100]",
                ));
            }
        }
        if let Some(rule) = self.discount_obligation {
            if !rule.price_threshold_pct.is_finite() || rule.price_threshold_pct <= 0.0 {
                return Err(invalid(
                    "discount obligation price threshold must be a positive percent of par",
                ));
            }
        }
        Ok(())
    }
}

/// What a failing coverage test does with the interest it diverts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CoverageTestAction {
    /// Pay down the notes of the senior-most principal tier, in its recipient
    /// order, until the test is cured (the standard OC/IC turbo).
    #[default]
    PayDownSenior,
    /// Retain the diverted interest as principal proceeds (a CLO
    /// reinvestment OC test): it is reinvested while the reinvestment period
    /// is active and repays notes through the principal tier afterwards.
    Reinvest,
}

/// One coverage test at a position in the waterfall.
///
/// The ratio is computed on the period's collateral and note balances
/// (overcollateralization: collateral value over the tested class and every
/// class senior to it; interest coverage: interest collections net of senior
/// fees over the interest due to the same classes); the *position* of the
/// tier that carries the test decides which cash a failure can divert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CoverageTestSpec {
    /// Unique test identifier, reported in `WaterfallDistribution::coverage_tests`
    /// (`OC_<tranche>` / `IC_<tranche>` by convention).
    pub id: String,
    /// Tested class; the ratio covers this class and every class senior to it.
    pub tranche_id: String,
    /// Overcollateralization or interest coverage.
    pub kind: CoverageTestType,
    /// Minimum ratio that passes (1.20 means 120%).
    pub trigger_level: f64,
    /// What a failure does with the diverted interest.
    #[serde(default)]
    pub action: CoverageTestAction,
    /// Template placement of the test tier; `None` places it after the
    /// tested tranche's own interest tier. Used only when the deal
    /// synthesizes its waterfall; a custom waterfall places test tiers
    /// explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<CoveragePlacement>,
    /// Cap on what a failure diverts, as a percent of the interest remaining
    /// at the test tier (`50.0` diverts at most half); `None` diverts up to
    /// the cure amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divert_pct: Option<f64>,
}

/// Where the template places a coverage test tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CoveragePlacement {
    /// After the interest tier of this tranche (a test on class B placed
    /// after class A's coupon diverts ahead of B's coupon).
    AfterTranche {
        /// Tranche whose interest tier the test follows.
        tranche_id: String,
    },
    /// After the junior (subordinated management) fee tier, ahead of the
    /// residual: the test diverts only what would otherwise reach equity.
    AfterJuniorFees,
}

impl CoverageTestSpec {
    fn new(tranche_id: impl Into<String>, kind: CoverageTestType, trigger_level: f64) -> Self {
        let tranche_id = tranche_id.into();
        Self {
            id: format!("{}_{tranche_id}", kind.label()),
            tranche_id,
            kind,
            trigger_level,
            action: CoverageTestAction::default(),
            placement: None,
            divert_pct: None,
        }
    }

    /// Overcollateralization test on `tranche_id` at `trigger_level`
    /// (1.20 means 120%), id `OC_<tranche_id>`, paying down senior notes on
    /// failure.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; the denominator is this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn oc(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::Oc, trigger_level)
    }

    /// Interest coverage test on `tranche_id` at `trigger_level` (1.10 means
    /// 110%), id `IC_<tranche_id>`, paying down senior notes on failure.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; interest due covers this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn ic(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::Ic, trigger_level)
    }

    /// Borrowing-base test on `tranche_id` at `trigger_level` (1.0 means the
    /// borrowing base must cover the tested class and every class senior to
    /// it), id `BB_<tranche_id>`, paying down senior notes on failure. The
    /// advance rates and concentration limits come from
    /// `CoverageRules::borrowing_base`.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Tested class; the denominator is this class plus
    ///   every class senior to it.
    /// * `trigger_level` - Minimum passing ratio as a decimal multiple.
    #[must_use]
    pub fn borrowing_base(tranche_id: impl Into<String>, trigger_level: f64) -> Self {
        Self::new(tranche_id, CoverageTestType::BorrowingBase, trigger_level)
    }

    /// Set what a failure does with the diverted interest.
    #[must_use]
    pub fn with_action(mut self, action: CoverageTestAction) -> Self {
        self.action = action;
        self
    }

    /// Place the template test tier after `tranche_id`'s interest tier.
    ///
    /// # Arguments
    ///
    /// * `tranche_id` - Class whose interest tier the test follows.
    #[must_use]
    pub fn after_tranche(mut self, tranche_id: impl Into<String>) -> Self {
        self.placement = Some(CoveragePlacement::AfterTranche {
            tranche_id: tranche_id.into(),
        });
        self
    }

    /// Place the test after the junior fee tier, ahead of the residual.
    #[must_use]
    pub fn after_junior_fees(mut self) -> Self {
        self.placement = Some(CoveragePlacement::AfterJuniorFees);
        self
    }

    /// Cap what a failure diverts at `pct` percent of the interest remaining
    /// at the test tier.
    ///
    /// # Arguments
    ///
    /// * `pct` - Percent of the remaining interest in `(0, 100]`.
    #[must_use]
    pub fn with_divert_pct(mut self, pct: f64) -> Self {
        self.divert_pct = Some(pct);
        self
    }

    /// Tranche whose interest tier the template places this test after
    /// (the tested tranche unless placed after another one); `None` when
    /// placed after the junior fees.
    pub fn placement_tranche(&self) -> Option<&str> {
        match &self.placement {
            None => Some(&self.tranche_id),
            Some(CoveragePlacement::AfterTranche { tranche_id }) => Some(tranche_id),
            Some(CoveragePlacement::AfterJuniorFees) => None,
        }
    }
}

/// Type of coverage test (simplified to OC/IC only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CoverageTestType {
    /// Overcollateralization test
    Oc,
    /// Interest coverage test
    Ic,
    /// Borrowing-base test: advance-rate-weighted eligible collateral (after
    /// the concentration limits in `CoverageRules::borrowing_base`) over the
    /// tested class plus every class senior to it.
    BorrowingBase,
}

impl CoverageTestType {
    /// Upper-case label used in test ids (`OC` / `IC`).
    pub fn label(self) -> &'static str {
        match self {
            Self::Oc => "OC",
            Self::Ic => "IC",
            Self::BorrowingBase => "BB",
        }
    }
}
