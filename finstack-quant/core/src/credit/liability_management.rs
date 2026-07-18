//! Hold-versus-tender economics for distressed exchanges and liability
//! management exercises (LMEs).
//!
//! All amounts use one caller-defined unit. The kernel performs no currency
//! conversion, so present values, fees, notionals and EBITDA must already be
//! expressed in the same unit. Percentages are **fractions**, not points:
//! `0.60` means 60 cents on the dollar and `0.40` means 40% participation.
//! Multiples are plain ratios, so a `pre_leverage` of `8.0` means 8.0x.
//!
//! Two complementary views are provided:
//!
//! * [`analyze_exchange_offer`] takes the **creditor** side of a distressed
//!   exchange and compares the present value of holding out against the total
//!   consideration offered for tendering.
//! * [`analyze_lme`] takes the **issuer** side of a liability management
//!   exercise and reports cash cost, par retired, discount captured, and the
//!   resulting change in gross leverage.
//!
//! Sums are evaluated left to right with ordinary `f64` addition rather than a
//! compensated accumulator, so results are bit-for-bit reproducible and match
//! the equivalent scalar expression written in any IEEE-754 environment.
//!
//! # References
//!
//! - Moody's Investors Service (2024). *Rating Symbols and Definitions* —
//!   definition of a distressed exchange as a default event where creditors
//!   receive less value than originally promised and the offer has the effect
//!   of allowing the issuer to avoid a bankruptcy or payment default.
//! - S&P Global Ratings (2022). "Methodology: Timeliness Of Payments: Grace
//!   Periods, Guarantees, And Use Of 'D' And 'SD' Ratings", which classifies
//!   below-par repurchases, amend-and-extend transactions and collateral
//!   transfers ("dropdowns") as selective defaults when investors are offered
//!   less than the original promise.
//! - Moyer, S. G. (2005). *Distressed Debt Analysis: Strategies for
//!   Speculative Investors*. J. Ross Publishing. Chapters 6 and 9 cover
//!   exchange-offer participation economics and discount capture.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::credit::liability_management::{
//!     analyze_exchange_offer, ExchangeType,
//! };
//! # fn main() -> finstack_quant_core::Result<()> {
//! let offer = analyze_exchange_offer(45.0, 80.0, 2.0, 0.0, ExchangeType::Discount)?;
//! assert!(offer.tender_recommended);
//! assert_eq!(offer.tender_total, 82.0);
//! # Ok(())
//! # }
//! ```

use core::fmt;
use core::str::FromStr;

use crate::{Error, Result};

/// Multiple of the hold-out present value that the tender consideration must
/// exceed before tendering is recommended.
///
/// The 2% cushion absorbs the execution risk, consent-solicitation risk and
/// valuation error embedded in the new instrument's present value: a tender
/// that is only marginally accretive does not compensate a creditor for giving
/// up its existing covenant and collateral position.
pub const TENDER_RECOMMENDATION_HURDLE: f64 = 1.02;

/// Structure of a distressed exchange offer.
///
/// The variant records how the new instrument ranks against the existing claim
/// and is reported back on [`ExchangeOfferAnalysis`] for audit purposes; it
/// does not change the arithmetic.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeType {
    /// New instrument issued at the same face amount as the old claim.
    ParForPar,
    /// New instrument issued at a face amount below the old claim.
    Discount,
    /// Participating creditors are lifted above the existing capital structure.
    Uptier,
    /// Participating creditors are subordinated relative to their old claim.
    Downtier,
}

impl ExchangeType {
    /// Canonical snake_case name of this exchange structure.
    ///
    /// # Returns
    ///
    /// The serde representation, for example `"par_for_par"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParForPar => "par_for_par",
            Self::Discount => "discount",
            Self::Uptier => "uptier",
            Self::Downtier => "downtier",
        }
    }
}

impl fmt::Display for ExchangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExchangeType {
    type Err = Error;

    /// Parse an exchange structure, accepting market shorthand.
    ///
    /// Parsing is case-insensitive, trims surrounding whitespace, normalises
    /// `-` to `_`, and accepts `"par"` as an alias for `"par_for_par"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the label is not a known structure.
    fn from_str(value: &str) -> Result<Self> {
        match normalize_label(value).as_str() {
            "par" | "par_for_par" => Ok(Self::ParForPar),
            "discount" => Ok(Self::Discount),
            "uptier" => Ok(Self::Uptier),
            "downtier" => Ok(Self::Downtier),
            _ => Err(validation_error(format!(
                "unknown exchange_type '{value}' — expected one of par_for_par, discount, \
                 uptier, downtier"
            ))),
        }
    }
}

/// Hold-versus-tender economics of a distressed exchange offer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExchangeOfferAnalysis {
    /// Structure of the offer, echoed back in canonical form.
    pub exchange_type: ExchangeType,
    /// Present value of the existing claim if the holder does not tender.
    pub old_npv: f64,
    /// Present value of the new instrument received on tendering.
    pub new_npv: f64,
    /// Cash consent or early-tender fee.
    pub consent_fee: f64,
    /// Estimated value of attached equity or warrants.
    pub equity_sweetener_value: f64,
    /// Total tender consideration: new instrument plus fee plus sweetener.
    pub tender_total: f64,
    /// Tender consideration less the hold-out present value.
    pub delta_npv: f64,
    /// Hold-out recovery, as a fraction of the existing claim's present value,
    /// at which tendering and holding out break even. Capped at `1.0`.
    pub breakeven_recovery: f64,
    /// Whether the tender consideration clears the
    /// [`TENDER_RECOMMENDATION_HURDLE`] cushion over the hold-out value.
    pub tender_recommended: bool,
}

/// Compare hold-versus-tender economics for a distressed exchange offer.
///
/// The tender consideration is the sum of the new instrument's present value,
/// any cash consent or early-tender fee, and the value of attached equity or
/// warrants. Tendering is recommended only when that total exceeds the
/// hold-out present value by more than [`TENDER_RECOMMENDATION_HURDLE`].
///
/// `breakeven_recovery` answers "how much would I have to recover by holding
/// out to match the tender?" and is reported as a fraction of the hold-out
/// present value, capped at `1.0` because a hold-out cannot recover more than
/// its own claim in this comparison. When `old_pv` is zero the ratio is
/// undefined and `1.0` is returned.
///
/// # Errors
///
/// Returns [`Error::Validation`] when any amount is negative or non-finite.
///
/// # Arguments
///
/// * `old_pv` - Present value of the existing claim if it is not tendered, in
///   the caller's monetary unit.
/// * `new_pv` - Present value of the new instrument received on tendering, in
///   the same unit as `old_pv`.
/// * `consent_fee` - Cash consent or early-tender fee paid to participating
///   holders, in the same unit as `old_pv`.
/// * `equity_sweetener_value` - Estimated value of equity or warrants attached
///   to the new instrument, in the same unit as `old_pv`.
/// * `exchange_type` - Structure of the offer, echoed onto the result.
///
/// # Returns
///
/// An [`ExchangeOfferAnalysis`] carrying the inputs alongside the tender
/// total, NPV pickup, breakeven recovery, and the tender recommendation.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::liability_management::{
///     analyze_exchange_offer, ExchangeType,
/// };
/// # fn main() -> finstack_quant_core::Result<()> {
/// let offer = analyze_exchange_offer(45.0, 80.0, 2.0, 0.0, ExchangeType::Discount)?;
/// assert_eq!(offer.delta_npv, 37.0);
/// assert_eq!(offer.breakeven_recovery, 1.0);
/// # Ok(())
/// # }
/// ```
pub fn analyze_exchange_offer(
    old_pv: f64,
    new_pv: f64,
    consent_fee: f64,
    equity_sweetener_value: f64,
    exchange_type: ExchangeType,
) -> Result<ExchangeOfferAnalysis> {
    validate_non_negative_finite("old_pv", old_pv)?;
    validate_non_negative_finite("new_pv", new_pv)?;
    validate_non_negative_finite("consent_fee", consent_fee)?;
    validate_non_negative_finite("equity_sweetener_value", equity_sweetener_value)?;

    let tender_total = new_pv + consent_fee + equity_sweetener_value;
    let delta_npv = tender_total - old_pv;
    let breakeven_recovery = if old_pv > 0.0 {
        (tender_total / old_pv).min(1.0)
    } else {
        1.0
    };
    let tender_recommended = tender_total > old_pv * TENDER_RECOMMENDATION_HURDLE;

    Ok(ExchangeOfferAnalysis {
        exchange_type,
        old_npv: old_pv,
        new_npv: new_pv,
        consent_fee,
        equity_sweetener_value,
        tender_total,
        delta_npv,
        breakeven_recovery,
        tender_recommended,
    })
}

/// Structure of a liability management exercise.
///
/// Each variant reinterprets the `repurchase_price_pct` argument of
/// [`analyze_lme`]; see that function's documentation for the per-variant
/// meaning and admissible range.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LmeType {
    /// Open-market repurchase of debt below par.
    OpenMarketRepurchase,
    /// Public tender offer for debt at a stated price.
    TenderOffer,
    /// Maturity extension in exchange for a consent fee; no par is retired.
    AmendAndExtend,
    /// Transfer of collateral to an unrestricted subsidiary; no par is
    /// retired and remaining holders are structurally diluted.
    Dropdown,
}

impl LmeType {
    /// Canonical snake_case name of this LME structure.
    ///
    /// # Returns
    ///
    /// The serde representation, for example `"open_market_repurchase"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenMarketRepurchase => "open_market_repurchase",
            Self::TenderOffer => "tender_offer",
            Self::AmendAndExtend => "amend_and_extend",
            Self::Dropdown => "dropdown",
        }
    }
}

impl fmt::Display for LmeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LmeType {
    type Err = Error;

    /// Parse an LME structure, accepting market shorthand.
    ///
    /// Parsing is case-insensitive, trims surrounding whitespace, and
    /// normalises both `-` and `&` to `_`, so `"A&E"` and `"amend-and-extend"`
    /// both resolve to [`LmeType::AmendAndExtend`]. Additional accepted
    /// aliases are `"open_market"` and `"omr"`, `"tender"`, and `"ae"`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when the label is not a known structure.
    fn from_str(value: &str) -> Result<Self> {
        match normalize_label(value).replace('&', "_").as_str() {
            "open_market" | "open_market_repurchase" | "omr" => Ok(Self::OpenMarketRepurchase),
            "tender_offer" | "tender" => Ok(Self::TenderOffer),
            "amend_and_extend" | "ae" | "a_e" => Ok(Self::AmendAndExtend),
            "dropdown" => Ok(Self::Dropdown),
            _ => Err(validation_error(format!(
                "unknown lme_type '{value}' — expected open_market, tender_offer, \
                 amend_and_extend, dropdown"
            ))),
        }
    }
}

/// Gross-leverage impact of a liability management exercise.
///
/// Leverage is gross debt over EBITDA, so a value of `8.0` reads as 8.0x.
/// Only debt retired at par reduces leverage; consent fees and collateral
/// transfers leave gross debt unchanged.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LeverageImpact {
    /// Gross debt of the target instrument before the exercise.
    pub pre_total_debt: f64,
    /// Gross debt of the target instrument after the exercise.
    pub post_total_debt: f64,
    /// Gross debt over EBITDA before the exercise, as a multiple.
    pub pre_leverage: f64,
    /// Gross debt over EBITDA after the exercise, as a multiple.
    pub post_leverage: f64,
    /// Turns of leverage removed: `pre_leverage - post_leverage`.
    pub leverage_reduction: f64,
}

/// Issuer-side economics of a liability management exercise.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LmeAnalysis {
    /// Structure of the exercise, echoed back in canonical form.
    pub lme_type: LmeType,
    /// Cash paid by the issuer, in the caller's monetary unit.
    pub cost: f64,
    /// Face amount retired; zero for structures that do not extinguish debt.
    pub notional_reduction: f64,
    /// Par retired less cash paid — the discount captured by the issuer.
    pub discount_capture: f64,
    /// Discount captured as a fraction of par retired; zero when no par is
    /// retired.
    pub discount_capture_pct: f64,
    /// Fraction of value diverted away from non-participating holders; nonzero
    /// only for a [`LmeType::Dropdown`].
    pub remaining_holder_impact_pct: f64,
    /// Gross-leverage impact, present only when a positive EBITDA is supplied.
    pub leverage_impact: Option<LeverageImpact>,
}

/// Compute discount capture and leverage impact for an LME transaction.
///
/// The participating amount is `notional * opt_acceptance_pct`. What happens to
/// that amount depends on the structure, and `repurchase_price_pct` is
/// reinterpreted accordingly:
///
/// | `lme_type` | `repurchase_price_pct` means | Range | Par retired | Cash cost |
/// |---|---|---|---|---|
/// | [`OpenMarketRepurchase`][LmeType::OpenMarketRepurchase] | price as a fraction of par | `(0, 1.5]` | participating | participating × price |
/// | [`TenderOffer`][LmeType::TenderOffer] | price as a fraction of par | `(0, 1.5]` | participating | participating × price |
/// | [`AmendAndExtend`][LmeType::AmendAndExtend] | extension fee as a fraction of par | `[0, 0.10]` | none | participating × fee |
/// | [`Dropdown`][LmeType::Dropdown] | fraction of assets transferred away | `[0, 1]` | none | none |
///
/// The upper bound of `1.5` on repurchase prices admits premium tenders for
/// deeply discounted or high-coupon paper while still rejecting a price quoted
/// in points (for example `60` instead of `0.60`).
///
/// # Errors
///
/// Returns [`Error::Validation`] when `notional` is not positive and finite,
/// `opt_acceptance_pct` falls outside `[0, 1]`, or `repurchase_price_pct`
/// falls outside the range admitted by `lme_type`.
///
/// # Arguments
///
/// * `lme_type` - Structure of the exercise, which selects how
///   `repurchase_price_pct` is interpreted.
/// * `notional` - Outstanding face amount of the target instrument, in the
///   caller's monetary unit. Must be strictly positive.
/// * `repurchase_price_pct` - Price, fee, or transferred-asset fraction as
///   described in the table above. Always a fraction, never points.
/// * `opt_acceptance_pct` - Fraction of holders participating, in `[0, 1]`.
///   Use `1.0` for full participation.
/// * `ebitda` - Optional EBITDA in the same unit as `notional`. A positive
///   value produces the `leverage_impact` block; `None` or a non-positive
///   value omits it.
///
/// # Returns
///
/// An [`LmeAnalysis`] with cash cost, par retired, discount captured in
/// absolute and percentage terms, the impact on non-participating holders, and
/// the optional leverage block.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::liability_management::{analyze_lme, LmeType};
/// # fn main() -> finstack_quant_core::Result<()> {
/// // Buy back 40% of a 200mm bond at 60 cents, against 25mm of EBITDA.
/// let lme = analyze_lme(
///     LmeType::OpenMarketRepurchase,
///     200_000_000.0,
///     0.60,
///     0.40,
///     Some(25_000_000.0),
/// )?;
/// assert_eq!(lme.cost, 48_000_000.0);
/// assert_eq!(lme.notional_reduction, 80_000_000.0);
/// assert_eq!(lme.discount_capture, 32_000_000.0);
/// # Ok(())
/// # }
/// ```
pub fn analyze_lme(
    lme_type: LmeType,
    notional: f64,
    repurchase_price_pct: f64,
    opt_acceptance_pct: f64,
    ebitda: Option<f64>,
) -> Result<LmeAnalysis> {
    if !notional.is_finite() || notional <= 0.0 {
        return Err(validation_error(format!(
            "notional must be finite and positive, got {notional}"
        )));
    }
    if !(0.0..=1.0).contains(&opt_acceptance_pct) {
        return Err(validation_error(format!(
            "opt_acceptance_pct must be in [0.0, 1.0], got {opt_acceptance_pct}"
        )));
    }

    let participating = notional * opt_acceptance_pct;

    let (par_retired, cost, remaining_holder_impact_pct) = match lme_type {
        LmeType::OpenMarketRepurchase | LmeType::TenderOffer => {
            if !(repurchase_price_pct > 0.0 && repurchase_price_pct <= 1.5) {
                return Err(validation_error(format!(
                    "repurchase_price_pct must be in (0.0, 1.5], got {repurchase_price_pct}"
                )));
            }
            (participating, participating * repurchase_price_pct, 0.0)
        }
        LmeType::AmendAndExtend => {
            if !(0.0..=0.10).contains(&repurchase_price_pct) {
                return Err(validation_error(format!(
                    "extension_fee must be in [0.0, 0.10], got {repurchase_price_pct}"
                )));
            }
            (0.0, participating * repurchase_price_pct, 0.0)
        }
        LmeType::Dropdown => {
            if !(0.0..=1.0).contains(&repurchase_price_pct) {
                return Err(validation_error(format!(
                    "transferred-asset fraction must be in [0.0, 1.0], got {repurchase_price_pct}"
                )));
            }
            (0.0, 0.0, repurchase_price_pct)
        }
    };

    let discount_capture = par_retired - cost;
    let discount_capture_pct = if par_retired > 0.0 {
        discount_capture / par_retired
    } else {
        0.0
    };

    let leverage_impact = ebitda.filter(|value| *value > 0.0).map(|ebitda| {
        let post_total_debt = notional - par_retired;
        LeverageImpact {
            pre_total_debt: notional,
            post_total_debt,
            pre_leverage: notional / ebitda,
            post_leverage: post_total_debt / ebitda,
            leverage_reduction: (notional - post_total_debt) / ebitda,
        }
    });

    Ok(LmeAnalysis {
        lme_type,
        cost,
        notional_reduction: par_retired,
        discount_capture,
        discount_capture_pct,
        remaining_holder_impact_pct,
        leverage_impact,
    })
}

fn normalize_label(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

fn validate_non_negative_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(validation_error(format!(
            "{field} must be finite and non-negative, got {value}"
        )))
    }
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::Validation(message.into())
}
