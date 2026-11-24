//! Waterfall specification, tranche definitions, validation, and fluent builder.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::validation;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Waterfall allocation style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WaterfallStyle {
    /// European style: aggregate all events at fund level
    #[default]
    European,
    /// American style: deal-by-deal allocation. Every event must carry a
    /// non-empty `deal_id`; capital, preferred return, catch-up, and carry are
    /// tracked independently for each deal, while clawback remains a fund-level
    /// lifetime reconciliation settled once at fund end.
    American,
}

/// Catch-up mode for GP profit sharing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CatchUpMode {
    /// Full catch-up: GP gets 100% until target split is reached
    #[default]
    Full,
    /// Partial catch-up: GP gets configured percentage
    Partial,
}

/// Individual tranche in the waterfall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PeFundWaterfallTranche {
    /// Return LP capital contributions before any profit sharing
    ReturnOfCapital,
    /// Preferred return to LPs at specified IRR
    PreferredIrr {
        /// LP preferred-return hurdle as an annual decimal IRR (`0.08` = 8%),
        /// compounded on the spec's `day_count`; must be finite and greater than -1.
        hurdle_irr: f64,
    },
    /// Catch-up allocation to GP
    CatchUp {
        /// GP share of each catch-up dollar, in `[0, 1]`. In
        /// [`CatchUpMode::Partial`] this caps the GP's take per dollar; in
        /// [`CatchUpMode::Full`] the GP takes 100% of catch-up dollars and
        /// this value is only the fallback target share when no promote tier
        /// follows.
        gp_share: f64,
    },
    /// Promote tier with hurdle and LP/GP split.
    ///
    /// **Hard-hurdle gating semantics**: before the split activates, the LP
    /// is paid at 100% until the LP-net IRR reaches this tier's `hurdle_irr`;
    /// only cash above the hurdle is split `lp_share`/`gp_share` (cascading
    /// to the next tier once the next tier's hurdle is reached). If a
    /// [`PeFundWaterfallTranche::CatchUp`] precedes a promote tier whose `hurdle_irr` sits above
    /// the preferred return, the LP-100% infill between the catch-up and the
    /// hurdle dilutes the GP's realized share back below the catch-up
    /// target. LPAs where all post-catch-up dollars split continuously at
    /// `lp_share`/`gp_share` should set this tier's hurdle equal to the
    /// preceding preferred return (or `0.0` when there is no gap to fill),
    /// so the LP-100% infill is empty.
    PromoteTier {
        /// Annual decimal IRR hurdle (`0.12` = 12%) the LP must reach (at
        /// 100% payout) before this tier's split activates; must be finite and greater than -1.
        hurdle_irr: f64,
        /// LP share of each split dollar, in `[0, 1]`; must sum to 1 with
        /// `gp_share`.
        lp_share: f64,
        /// GP share of each split dollar, in `[0, 1]`; must sum to 1 with
        /// `lp_share`.
        gp_share: f64,
    },
}

/// Clawback specification for GP carry reconciliation, settled at fund end
/// (the date of the last fund event).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ClawbackSpec {
    /// Optional share of GP carry held back until settlement, as a decimal
    /// fraction in `[0, 1]` (`0.2` = 20%)
    pub holdback_decimal: Option<f64>,
}

/// Complete waterfall specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct PeFundWaterfallSpec {
    /// Allocation style (European vs American)
    pub style: WaterfallStyle,
    /// Ordered sequence of waterfall tranches
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<PeFundWaterfallTranche>")
    )]
    pub tranches: SmallVec<[PeFundWaterfallTranche; 8]>,
    /// Clawback specification; `None` means no clawback (presence is the switch)
    #[serde(default)]
    pub clawback: Option<ClawbackSpec>,
    /// Day count for LP/fund IRR year fractions and hurdle compounding
    /// `(1 + hurdle_irr)^years` (default Act/365F)
    #[serde(default = "default_day_count")]
    pub day_count: DayCount,
    /// Catch-up mode
    #[serde(default)]
    pub catch_up_mode: CatchUpMode,
}

fn default_day_count() -> DayCount {
    DayCount::Act365F
}

finstack_quant_valuations_core::validate_on_deserialize!(
    PeFundWaterfallSpec,
    PeFundWaterfallSpec::validate
);

impl PeFundWaterfallSpec {
    /// Create a new waterfall specification builder.
    #[must_use]
    pub fn builder() -> WaterfallSpecBuilder {
        WaterfallSpecBuilder::default()
    }

    /// Validate the waterfall specification.
    ///
    /// # Errors
    ///
    /// Returns an error when the spec has no tranches, a promote tier's
    /// LP/GP shares are non-finite, negative, or do not sum to 1.0, a
    /// preferred-return or promote-tier hurdle rate is non-finite or at/below
    /// -100%, a
    /// catch-up GP share is outside `[0, 1]`, or a clawback holdback
    /// percentage is outside `[0, 1]`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        validation::require_or(
            !self.tranches.is_empty(),
            finstack_quant_core::InputError::TooFewPoints,
        )?;
        // LP IRR and hurdle compounding share this annual basis; only bases
        // with an exact conversion from canonical XIRR are admitted
        // (record CORE-CONVENTIONS-017 #12).
        finstack_quant_valuations_core::metrics::returns::YieldBasis::try_from(self.day_count)?;

        for tranche in &self.tranches {
            match tranche {
                PeFundWaterfallTranche::ReturnOfCapital => {}
                PeFundWaterfallTranche::PreferredIrr { hurdle_irr: irr } => {
                    if !irr.is_finite() || *irr <= -1.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "tranches[].preferred_irr.hurdle_irr must be finite and greater than -100%, got {irr}"
                        )));
                    }
                }
                PeFundWaterfallTranche::CatchUp { gp_share } => {
                    if !gp_share.is_finite() || !(0.0..=1.0).contains(gp_share) {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "catch-up GP share must be in [0, 1], got {gp_share}"
                        )));
                    }
                }
                PeFundWaterfallTranche::PromoteTier {
                    hurdle_irr: rate,
                    lp_share,
                    gp_share,
                } => {
                    if !rate.is_finite() || *rate <= -1.0 {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "tranches[].promote_tier.hurdle_irr must be finite and greater than -100%, got {rate}"
                        )));
                    }
                    let sum = lp_share + gp_share;
                    validation::require_or(
                        lp_share.is_finite() && gp_share.is_finite(),
                        finstack_quant_core::Error::Validation(
                            "promote tier lp_share and gp_share must be finite".to_string(),
                        ),
                    )?;
                    validation::require_or(
                        (sum - 1.0).abs() <= 1e-6,
                        finstack_quant_core::Error::Validation(
                            "promote tier lp_share and gp_share must sum to one".to_string(),
                        ),
                    )?;
                    validation::require_or(
                        *lp_share >= 0.0 && *gp_share >= 0.0,
                        finstack_quant_core::InputError::NegativeValue,
                    )?;
                }
            }
        }

        if let Some(holdback_decimal) = self.clawback.as_ref().and_then(|c| c.holdback_decimal)
            && (!holdback_decimal.is_finite() || !(0.0..=1.0).contains(&holdback_decimal))
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "clawback holdback percentage must be in [0, 1], got {holdback_decimal}"
            )));
        }

        Ok(())
    }
}

/// Builder for waterfall specifications.
pub struct WaterfallSpecBuilder {
    style: WaterfallStyle,
    tranches: SmallVec<[PeFundWaterfallTranche; 8]>,
    clawback: Option<ClawbackSpec>,
    day_count: DayCount,
    catch_up_mode: CatchUpMode,
}

impl Default for WaterfallSpecBuilder {
    fn default() -> Self {
        Self {
            style: WaterfallStyle::default(),
            tranches: SmallVec::new(),
            clawback: None,
            day_count: default_day_count(),
            catch_up_mode: CatchUpMode::default(),
        }
    }
}

impl WaterfallSpecBuilder {
    /// style.
    pub fn style(mut self, style: WaterfallStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the IRR / hurdle-compounding day count.
    ///
    /// # Arguments
    ///
    /// * `day_count` - Day count for LP IRR year fractions and hurdle
    ///   compounding (default Act/365F).
    pub fn day_count(mut self, day_count: DayCount) -> Self {
        self.day_count = day_count;
        self
    }

    /// Set the catch-up mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Full (GP takes 100% of catch-up dollars) or partial (GP
    ///   takes the tranche's `gp_share`).
    pub fn catch_up_mode(mut self, mode: CatchUpMode) -> Self {
        self.catch_up_mode = mode;
        self
    }

    /// return of capital.
    pub fn return_of_capital(mut self) -> Self {
        self.tranches.push(PeFundWaterfallTranche::ReturnOfCapital);
        self
    }

    /// Append a preferred-return tranche.
    ///
    /// # Arguments
    ///
    /// * `irr` - Annual preferred-return hurdle as a decimal rate (`0.08` =
    ///   8%), compounded per the spec's `day_count` day count. Must be finite;
    ///   `build()` rejects non-finite rates.
    pub fn preferred_irr(mut self, irr: f64) -> Self {
        self.tranches
            .push(PeFundWaterfallTranche::PreferredIrr { hurdle_irr: irr });
        self
    }

    /// Append a GP catch-up tranche.
    ///
    /// # Arguments
    ///
    /// * `gp_share` - GP share of each catch-up dollar as a decimal in
    ///   `[0, 1]` (see [`PeFundWaterfallTranche::CatchUp`] for mode-dependent semantics);
    ///   `build()` rejects values outside that range.
    pub fn catch_up(mut self, gp_share: f64) -> Self {
        self.tranches
            .push(PeFundWaterfallTranche::CatchUp { gp_share });
        self
    }

    /// Append a promote tier with an IRR hurdle and LP/GP split.
    ///
    /// See [`PeFundWaterfallTranche::PromoteTier`] for the hard-hurdle gating semantics:
    /// the LP is paid at 100% until `hurdle_irr` is reached, then cash
    /// splits `lp_share`/`gp_share`.
    ///
    /// # Arguments
    ///
    /// * `hurdle_irr` - Annual IRR hurdle as a decimal rate (`0.12` = 12%)
    ///   the LP must reach before the split activates. Must be finite.
    /// * `lp_share` - LP share of each split dollar, in `[0, 1]`; must sum
    ///   to 1 with `gp_share`.
    /// * `gp_share` - GP share of each split dollar, in `[0, 1]`; must sum
    ///   to 1 with `lp_share`.
    pub fn promote_tier(mut self, hurdle_irr: f64, lp_share: f64, gp_share: f64) -> Self {
        self.tranches.push(PeFundWaterfallTranche::PromoteTier {
            hurdle_irr,
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
    pub fn build(self) -> finstack_quant_core::Result<PeFundWaterfallSpec> {
        let spec = PeFundWaterfallSpec {
            style: self.style,
            tranches: self.tranches,
            clawback: self.clawback,
            day_count: self.day_count,
            catch_up_mode: self.catch_up_mode,
        };
        spec.validate()?;
        Ok(spec)
    }
}
