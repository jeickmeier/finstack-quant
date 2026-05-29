//! Discounted Cash Flow instrument types and implementations.
//!
//! Core DCF types used for corporate valuation:
//! - [`TerminalValueSpec`] for Gordon Growth, Exit Multiple, and H-Model terminals
//! - [`DiscountedCashFlow`] instrument implementing the standard DCF formula

use crate::impl_instrument_base;
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::common_impl::traits::{
    Attributes, CurveDependencies, Instrument, InstrumentCurves,
};
use crate::instruments::equity::dcf_equity::pricer;
use crate::pricer::InstrumentType;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_core::Error as CoreError;

/// Spread `WACC − g` below which Gordon Growth is near-singular: the terminal
/// multiplier `(1+g)/(WACC−g)` becomes extremely sensitive to small parameter
/// errors. 10 bp is the standard practitioner threshold (Damodaran):
/// `(1.03)/0.001 ≈ 1030×`, vs `(1.03)/0.05 ≈ 21×` for a healthy spread.
const GORDON_GROWTH_NEAR_SINGULARITY_THRESHOLD: f64 = 0.001;

/// Terminal value calculation method for DCF.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalValueSpec {
    /// Gordon Growth Model: TV = FCF_terminal × (1 + g) / (WACC - g)
    GordonGrowth {
        /// Perpetual growth rate (e.g., 0.02 for 2%)
        growth_rate: f64,
    },
    /// Exit Multiple: TV = Terminal_Metric × Multiple
    ExitMultiple {
        /// Terminal metric value (e.g., EBITDA)
        terminal_metric: f64,
        /// Multiple to apply (e.g., 10.0 for 10x EBITDA)
        multiple: f64,
    },
    /// H-Model: linear fade from high growth to stable growth (Damodaran).
    ///
    /// ```text
    /// TV = FCF_T × (1+g_s)/(WACC-g_s) + FCF_T × H × (g_h-g_s)/(WACC-g_s)
    /// ```
    ///
    /// where `H` is the half-life in years of the high-growth fade period.
    /// The first term is the standard Gordon Growth stable-state value;
    /// the second term captures the present value of excess growth that
    /// linearly decays from `high_growth_rate` to `stable_growth_rate`
    /// over `2 × half_life_years`.
    HModel {
        /// Initial high growth rate (e.g., 0.15 for 15%).
        high_growth_rate: f64,
        /// Long-run stable growth rate (e.g., 0.03 for 3%). Must be < WACC.
        stable_growth_rate: f64,
        /// Half-life of the growth fade in years (e.g., 5.0).
        half_life_years: f64,
    },
}

/// Structured equity bridge for converting Enterprise Value to Equity Value.
///
/// Standard professional bridge:
/// ```text
/// Equity = EV - Total Debt + Cash - Preferred Equity - Minority Interest
///          + Non-Operating Assets + Σ(other adjustments)
/// ```
///
/// When attached to a [`DiscountedCashFlow`], this takes precedence over the
/// flat `net_debt` scalar.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct EquityBridge {
    /// Total interest-bearing debt.
    #[serde(default)]
    pub total_debt: f64,
    /// Cash and cash equivalents.
    #[serde(default)]
    pub cash: f64,
    /// Preferred stock at liquidation preference.
    #[serde(default)]
    pub preferred_equity: f64,
    /// Non-controlling (minority) interests.
    #[serde(default)]
    pub minority_interest: f64,
    /// Non-operating assets (excess cash, investments, real estate, etc.).
    #[serde(default)]
    pub non_operating_assets: f64,
    /// Named adjustments (e.g., unfunded pension, contingent liabilities).
    /// Positive values increase equity; negative values decrease it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_adjustments: Vec<(String, f64)>,
}

impl EquityBridge {
    /// Net adjustment amount to subtract from Enterprise Value.
    ///
    /// `net_adjustment = total_debt - cash + preferred_equity + minority_interest
    ///                   - non_operating_assets - Σ(other_adjustments)`
    pub fn net_adjustment(&self) -> f64 {
        let other_sum: f64 = self.other_adjustments.iter().map(|(_, v)| v).sum();
        self.total_debt - self.cash + self.preferred_equity + self.minority_interest
            - self.non_operating_assets
            - other_sum
    }
}

/// Valuation discounts for private company equity.
///
/// Applied multiplicatively after the equity bridge:
/// ```text
/// FMV = Equity Value × (1 - DLOC) × (1 - DLOM) × (1 - other_discount)
/// ```
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ValuationDiscounts {
    /// Discount for Lack of Marketability (0.0–1.0, e.g., 0.25 for 25%).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dlom: Option<f64>,
    /// Discount for Lack of Control (0.0–1.0, e.g., 0.20 for 20%).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dloc: Option<f64>,
    /// Additional discount (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_discount: Option<f64>,
}

impl ValuationDiscounts {
    /// Validate that all discount rates are in `[0.0, 1.0]`.
    pub fn validate(&self) -> finstack_core::Result<()> {
        fn check(name: &str, val: Option<f64>) -> finstack_core::Result<()> {
            if let Some(v) = val {
                if !(0.0..=1.0).contains(&v) {
                    return Err(CoreError::Validation(format!(
                        "{name} must be in [0.0, 1.0], got {v:.6}"
                    )));
                }
            }
            Ok(())
        }
        check("dlom", self.dlom)?;
        check("dloc", self.dloc)?;
        check("other_discount", self.other_discount)?;
        Ok(())
    }

    /// Apply all discounts multiplicatively to an equity value.
    ///
    /// Returns `Err` if any discount is outside `[0.0, 1.0]`.
    pub fn apply(&self, equity_value: f64) -> finstack_core::Result<f64> {
        self.validate()?;
        let mut val = equity_value;
        if let Some(dloc) = self.dloc {
            val *= 1.0 - dloc;
        }
        if let Some(dlom) = self.dlom {
            val *= 1.0 - dlom;
        }
        if let Some(other) = self.other_discount {
            val *= 1.0 - other;
        }
        Ok(val)
    }
}

/// A dilutive security for the treasury stock method.
///
/// Used to compute diluted shares outstanding from options, warrants,
/// RSUs, or convertible instruments.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct DilutionSecurity {
    /// Descriptive name (e.g., "Employee Stock Options").
    pub name: String,
    /// Number of shares issuable upon exercise/conversion.
    pub quantity: f64,
    /// Exercise or strike price per share (0.0 for RSUs/convertibles with no cost).
    pub exercise_price: f64,
}

/// Discounted Cash Flow instrument for corporate valuation.
///
/// DCF values a company by discounting projected free cash flows and terminal value.
/// The equity value is calculated as: Enterprise Value - Net Debt (or structured bridge).
///
/// # Equity Bridge
///
/// When [`equity_bridge`](Self::equity_bridge) is `Some`, it takes precedence over
/// the flat [`net_debt`](Self::net_debt) field for the EV-to-equity conversion.
///
/// # Mid-Year Convention
///
/// When [`mid_year_convention`](Self::mid_year_convention) is `true`, cash flows are
/// discounted at `(t - 0.5)` years instead of `t` years, reflecting the assumption
/// that cash flows arrive mid-period (standard IB/PE practice).
///
/// # Valuation Discounts
///
/// Private company valuations can apply DLOM, DLOC, and other discounts via
/// [`valuation_discounts`](Self::valuation_discounts).
#[derive(
    Clone,
    Debug,
    finstack_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[builder(validate = DiscountedCashFlow::validate)]
#[serde(deny_unknown_fields, try_from = "DiscountedCashFlowUnchecked")]
pub struct DiscountedCashFlow {
    /// Unique identifier for the DCF.
    pub id: InstrumentId,
    /// Currency for all cashflows.
    pub currency: Currency,
    /// Explicit period free cash flows (date, amount pairs).
    #[schemars(with = "Vec<(String, f64)>")]
    pub flows: Vec<(Date, f64)>,
    /// Weighted Average Cost of Capital (discount rate).
    pub wacc: f64,
    /// Terminal value specification.
    pub terminal_value: TerminalValueSpec,
    /// Net debt (debt - cash) to subtract from enterprise value.
    ///
    /// Ignored when [`equity_bridge`](Self::equity_bridge) is `Some`.
    pub net_debt: f64,
    /// Valuation date (as-of date for the DCF).
    #[schemars(with = "String")]
    pub valuation_date: Date,
    /// Discount curve used to present-value the explicit cash flows and the
    /// terminal value.
    ///
    /// When this curve is loaded in the market context it acts as the discount
    /// rate — which is what gives the DCF its rate sensitivity (DV01). When the
    /// curve is absent, discounting falls back to [`wacc`](Self::wacc). The
    /// terminal value is always *capitalized* at `wacc` (see
    /// [`calculate_terminal_value`](Self::calculate_terminal_value)); the cap
    /// rate and the discount rate are intentionally distinct.
    pub discount_curve_id: CurveId,
    /// Mid-year discounting convention (default: `false` = end-of-year).
    ///
    /// When `true`, each flow is discounted at `(t - 0.5)` instead of `t`,
    /// reflecting the assumption that cash flows arrive mid-period.
    /// This is the standard convention in IB/PE practice (Koller et al.).
    #[builder(default)]
    #[serde(default)]
    pub mid_year_convention: bool,
    /// Structured equity bridge for EV-to-equity conversion.
    ///
    /// When present, takes precedence over the flat `net_debt` field.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equity_bridge: Option<EquityBridge>,
    /// Basic shares outstanding for per-share value calculation.
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares_outstanding: Option<f64>,
    /// Dilutive securities (options, warrants, RSUs, convertibles) for treasury stock method.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dilution_securities: Vec<DilutionSecurity>,
    /// Private company valuation discounts (DLOM, DLOC).
    #[builder(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valuation_discounts: Option<ValuationDiscounts>,
    /// Attributes for tagging and scenarios.
    #[serde(default)]
    #[builder(default)]
    pub pricing_overrides: crate::instruments::PricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

/// Mirror of `DiscountedCashFlow` used by serde to apply `validate()` after
/// deserialization. Not part of the public API.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct DiscountedCashFlowUnchecked {
    id: InstrumentId,
    currency: Currency,
    #[schemars(with = "Vec<(String, f64)>")]
    flows: Vec<(Date, f64)>,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    net_debt: f64,
    #[schemars(with = "String")]
    valuation_date: Date,
    discount_curve_id: CurveId,
    #[serde(default)]
    mid_year_convention: bool,
    #[serde(default)]
    equity_bridge: Option<EquityBridge>,
    #[serde(default)]
    shares_outstanding: Option<f64>,
    #[serde(default)]
    dilution_securities: Vec<DilutionSecurity>,
    #[serde(default)]
    valuation_discounts: Option<ValuationDiscounts>,
    #[serde(default)]
    pricing_overrides: crate::instruments::PricingOverrides,
    attributes: Attributes,
}

impl TryFrom<DiscountedCashFlowUnchecked> for DiscountedCashFlow {
    type Error = finstack_core::Error;

    fn try_from(value: DiscountedCashFlowUnchecked) -> std::result::Result<Self, Self::Error> {
        let inst = Self {
            id: value.id,
            currency: value.currency,
            flows: value.flows,
            wacc: value.wacc,
            terminal_value: value.terminal_value,
            net_debt: value.net_debt,
            valuation_date: value.valuation_date,
            discount_curve_id: value.discount_curve_id,
            mid_year_convention: value.mid_year_convention,
            equity_bridge: value.equity_bridge,
            shares_outstanding: value.shares_outstanding,
            dilution_securities: value.dilution_securities,
            valuation_discounts: value.valuation_discounts,
            pricing_overrides: value.pricing_overrides,
            attributes: value.attributes,
        };
        inst.validate()?;
        Ok(inst)
    }
}

impl DiscountedCashFlow {
    /// Validate structural invariants required by the DCF pricer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `wacc` is non-finite or `wacc <= -1.0` (would yield `(1+wacc) <= 0`,
    ///   producing infinite / NaN present values)
    /// - `wacc` is outside the conservative sanity band `(-0.5, 1.0)` — this
    ///   catches typical typos (e.g. `wacc = 10` instead of `0.10`) before
    ///   they silently drive valuations to ~0
    /// - `net_debt` is non-finite
    /// - any explicit `flows` amount is non-finite
    /// - `flows` are not sorted by date (strictly increasing)
    /// - any flow date is on or before `valuation_date`
    /// - `shares_outstanding` is set but non-positive or non-finite
    /// - any `dilution_securities` entry has non-finite or negative quantity
    ///   or non-finite exercise price
    /// - the embedded `valuation_discounts` (when set) fails its own
    ///   `validate()` (each discount in `[0, 1]`)
    ///
    /// Terminal-value-specific checks (Gordon Growth requires
    /// `wacc > growth_rate`; H-Model requires the same plus
    /// `high_growth_rate >= stable_growth_rate`) are enforced inside
    /// [`Self::calculate_terminal_value`] at pricing time, since they cross
    /// `wacc` with the terminal-value variant.
    pub fn validate(&self) -> finstack_core::Result<()> {
        if !self.wacc.is_finite() {
            return Err(CoreError::Validation(format!(
                "DCF '{}' wacc must be finite, got {}",
                self.id.as_str(),
                self.wacc
            )));
        }
        if self.wacc <= -1.0 {
            return Err(CoreError::Validation(format!(
                "DCF '{}' wacc = {} would make (1 + wacc) <= 0; must be > -1.0",
                self.id.as_str(),
                self.wacc
            )));
        }
        if !(-0.5..1.0).contains(&self.wacc) {
            return Err(CoreError::Validation(format!(
                "DCF '{}' wacc = {} is outside the sanity band (-0.5, 1.0); \
                 typical WACC is 0.05 - 0.20 (5% - 20%). Did you mean {}?",
                self.id.as_str(),
                self.wacc,
                self.wacc / 100.0
            )));
        }
        if !self.net_debt.is_finite() {
            return Err(CoreError::Validation(format!(
                "DCF '{}' net_debt must be finite, got {}",
                self.id.as_str(),
                self.net_debt
            )));
        }
        for (i, (date, amount)) in self.flows.iter().enumerate() {
            if !amount.is_finite() {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' flows[{}] amount on {} must be finite, got {}",
                    self.id.as_str(),
                    i,
                    date,
                    amount
                )));
            }
        }
        for window in self.flows.windows(2) {
            if window[0].0 >= window[1].0 {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' flow dates must be strictly increasing; got {} >= {}",
                    self.id.as_str(),
                    window[0].0,
                    window[1].0
                )));
            }
        }
        for (i, (date, _)) in self.flows.iter().enumerate() {
            if *date <= self.valuation_date {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' flows[{}] date {} must be strictly after valuation_date {}",
                    self.id.as_str(),
                    i,
                    date,
                    self.valuation_date
                )));
            }
        }
        if let Some(shares) = self.shares_outstanding {
            if !shares.is_finite() || shares <= 0.0 {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' shares_outstanding must be finite and positive, got {}",
                    self.id.as_str(),
                    shares
                )));
            }
        }
        for (i, sec) in self.dilution_securities.iter().enumerate() {
            if !sec.quantity.is_finite() || sec.quantity < 0.0 {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' dilution_securities[{}] quantity must be finite and non-negative, got {}",
                    self.id.as_str(), i, sec.quantity
                )));
            }
            if !sec.exercise_price.is_finite() || sec.exercise_price < 0.0 {
                return Err(CoreError::Validation(format!(
                    "DCF '{}' dilution_securities[{}] exercise_price must be finite and non-negative, got {}",
                    self.id.as_str(), i, sec.exercise_price
                )));
            }
        }
        if let Some(d) = &self.valuation_discounts {
            d.validate()?;
        }
        Ok(())
    }

    /// Create a representative tech company DCF example.
    ///
    /// 5-year FCF projections ($5M-$8M), 10% WACC, Gordon Growth terminal
    /// value at 2% growth, $15M net debt, mid-year convention enabled.
    pub fn example() -> finstack_core::Result<Self> {
        let valuation_date = time::macros::date!(2025 - 01 - 01);

        let flows: Vec<(Date, f64)> = vec![
            (time::macros::date!(2026 - 01 - 01), 5_000_000.0),
            (time::macros::date!(2027 - 01 - 01), 5_750_000.0),
            (time::macros::date!(2028 - 01 - 01), 6_500_000.0),
            (time::macros::date!(2029 - 01 - 01), 7_250_000.0),
            (time::macros::date!(2030 - 01 - 01), 8_000_000.0),
        ];

        Self::builder()
            .id(InstrumentId::new("DCF-TECH-CO"))
            .currency(Currency::USD)
            .flows(flows)
            .wacc(0.10)
            .terminal_value(TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt(15_000_000.0)
            .valuation_date(valuation_date)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .mid_year_convention(true)
            .shares_outstanding_opt(Some(10_000_000.0))
            .attributes(Attributes::default())
            .build()
    }

    /// Calculate present value of explicit period cash flows using WACC.
    ///
    /// Respects [`mid_year_convention`](Self::mid_year_convention): when `true`,
    /// each flow is discounted at `(t - 0.5)` years.
    pub fn calculate_pv_explicit_flows(&self) -> f64 {
        self.flows
            .iter()
            .map(|(date, amount)| {
                let years = self.discount_years(self.valuation_date, *date);
                amount / (1.0 + self.wacc).powf(years)
            })
            .sum()
    }

    /// Calculate terminal value (undiscounted).
    ///
    /// # Errors
    ///
    /// - `GordonGrowth` / `HModel`: returns `Err` if flows are empty (last FCF is needed).
    /// - `GordonGrowth`: WACC must be > growth_rate.
    /// - `HModel`: WACC must be > stable_growth_rate; high_growth_rate must be
    ///   >= stable_growth_rate; half_life_years must be > 0.
    /// - `ExitMultiple`: never fails (does not depend on flows).
    pub fn calculate_terminal_value(&self) -> finstack_core::Result<f64> {
        match &self.terminal_value {
            TerminalValueSpec::GordonGrowth { growth_rate } => {
                let (_, last_fcf) = self.flows.last().ok_or_else(|| {
                    CoreError::Validation(
                        "DCF has no explicit flows; cannot compute terminal value".into(),
                    )
                })?;
                let g = *growth_rate;
                if self.wacc <= g {
                    return Err(CoreError::Validation(format!(
                        "Gordon Growth requires WACC ({:.6}) > growth_rate ({:.6})",
                        self.wacc, g
                    )));
                }
                // Warn on near-singularity: the multiplier (1 + g) / (WACC − g)
                // blows up as the spread closes. A 1 bp WACC/g typo at spread
                // ≈ 10 bp swings TV by ~10×; trader/analyst should see this.
                let spread = self.wacc - g;
                if spread < GORDON_GROWTH_NEAR_SINGULARITY_THRESHOLD {
                    tracing::warn!(
                        wacc = self.wacc,
                        growth_rate = g,
                        wacc_minus_g = spread,
                        "Gordon Growth: WACC − g = {:.4} bp is within near-singularity \
                         threshold ({:.0} bp); terminal value is extremely sensitive to \
                         small parameter changes",
                        spread * 10_000.0,
                        GORDON_GROWTH_NEAR_SINGULARITY_THRESHOLD * 10_000.0
                    );
                }
                Ok(last_fcf * (1.0 + g) / spread)
            }
            TerminalValueSpec::ExitMultiple {
                terminal_metric,
                multiple,
            } => Ok(terminal_metric * multiple),
            TerminalValueSpec::HModel {
                high_growth_rate,
                stable_growth_rate,
                half_life_years,
            } => {
                let (_, last_fcf) = self.flows.last().ok_or_else(|| {
                    CoreError::Validation(
                        "DCF has no explicit flows; cannot compute terminal value".into(),
                    )
                })?;
                let g_s = *stable_growth_rate;
                let g_h = *high_growth_rate;
                let h = *half_life_years;
                if self.wacc <= g_s {
                    return Err(CoreError::Validation(format!(
                        "H-Model requires WACC ({:.6}) > stable_growth_rate ({:.6})",
                        self.wacc, g_s
                    )));
                }
                if g_h < g_s {
                    return Err(CoreError::Validation(format!(
                        "H-Model requires high_growth_rate ({:.6}) >= stable_growth_rate ({:.6})",
                        g_h, g_s
                    )));
                }
                if h <= 0.0 {
                    return Err(CoreError::Validation(format!(
                        "H-Model requires half_life_years > 0, got {:.6}",
                        h
                    )));
                }
                // Standard Gordon Growth stable-state value
                let stable_tv = last_fcf * (1.0 + g_s) / (self.wacc - g_s);
                // Growth premium from the H-model (Damodaran)
                let growth_premium = last_fcf * h * (g_h - g_s) / (self.wacc - g_s);
                Ok(stable_tv + growth_premium)
            }
        }
    }

    /// Discount terminal value to present value using WACC.
    ///
    /// Returns `Err` if flows are empty.
    pub fn discount_terminal_value(&self, terminal_value: f64) -> finstack_core::Result<f64> {
        let (terminal_date, _) = self.flows.last().ok_or_else(|| {
            CoreError::Validation(
                "DCF has no explicit flows; cannot discount terminal value".into(),
            )
        })?;
        let years = self.discount_years(self.valuation_date, *terminal_date);
        Ok(terminal_value / (1.0 + self.wacc).powf(years))
    }

    /// Effective net debt amount for the EV-to-equity bridge.
    ///
    /// Uses [`equity_bridge`](Self::equity_bridge) when present, otherwise
    /// falls back to [`net_debt`](Self::net_debt).
    pub fn effective_net_debt(&self) -> f64 {
        if let Some(ref bridge) = self.equity_bridge {
            bridge.net_adjustment()
        } else {
            self.net_debt
        }
    }

    /// Apply valuation discounts (DLOM, DLOC, other) to an equity value.
    ///
    /// Returns `Err` if any discount is outside `[0.0, 1.0]`.
    pub(crate) fn apply_valuation_discounts(
        &self,
        equity_value: f64,
    ) -> finstack_core::Result<f64> {
        if let Some(ref discounts) = self.valuation_discounts {
            discounts.apply(equity_value)
        } else {
            Ok(equity_value)
        }
    }

    /// Compute diluted share count using the treasury stock method.
    ///
    /// For each in-the-money dilutive security (exercise price < implied
    /// price per share), the incremental diluted shares are:
    /// ```text
    /// incremental = quantity - (quantity × exercise_price) / price_per_share
    /// ```
    ///
    /// **Convention**: The implied price per share is derived from the *post-discount*
    /// equity value (i.e., after DLOM/DLOC are applied). This means dilution is
    /// calculated on the fair market value that shareholders actually receive,
    /// which is the standard private-company valuation convention.
    ///
    /// Returns `None` if `shares_outstanding` is not set.
    pub fn diluted_shares(&self, equity_value: f64) -> Option<f64> {
        let basic = self.shares_outstanding?;
        if basic <= 0.0 {
            return Some(basic);
        }
        let price_per_share = equity_value / basic;
        if price_per_share <= 0.0 {
            return Some(basic);
        }
        let mut diluted = basic;
        for sec in &self.dilution_securities {
            if sec.exercise_price < price_per_share && sec.quantity > 0.0 {
                // Treasury stock method: proceeds buy back shares at current price
                let proceeds = sec.quantity * sec.exercise_price;
                let shares_repurchased = proceeds / price_per_share;
                diluted += sec.quantity - shares_repurchased;
            }
        }
        Some(diluted)
    }

    /// Equity value per diluted share.
    ///
    /// Returns `None` if `shares_outstanding` is not set or diluted shares is zero.
    pub fn equity_value_per_share(&self, equity_value: f64) -> Option<f64> {
        let diluted = self.diluted_shares(equity_value)?;
        if diluted <= 0.0 {
            return None;
        }
        Some(equity_value / diluted)
    }

    /// Calculate year fraction between two dates using ACT/365.25.
    ///
    /// Corporate DCF valuations conventionally use ACT/365.25 (actual days
    /// divided by 365.25) because:
    /// 1. Cash-flow projections are typically on annual boundaries, making
    ///    the choice of day-count immaterial for integer years.
    /// 2. Sub-year precision is only needed for the first/last stub period,
    ///    where ACT/365.25 is the standard corporate-finance convention
    ///    (Damodaran, Koller et al.).
    /// 3. WACC-based discounting is quoted assuming this basis.
    ///
    /// This differs from capital-markets instruments that use `DayCount`
    /// enums (e.g., ACT/360 for money-market, ACT/365F for equity vol).
    pub(crate) fn year_fraction(&self, start: Date, end: Date) -> f64 {
        let days = (end - start).whole_days() as f64;
        days / 365.25
    }

    /// Year fraction adjusted for mid-year convention.
    ///
    /// When `mid_year_convention` is `true`, subtracts 0.5 from the raw
    /// year fraction to reflect cash flows arriving mid-period.
    pub(crate) fn discount_years(&self, start: Date, end: Date) -> f64 {
        let raw = self.year_fraction(start, end);
        if self.mid_year_convention {
            (raw - 0.5).max(0.0)
        } else {
            raw
        }
    }
}

impl Instrument for DiscountedCashFlow {
    impl_instrument_base!(InstrumentType::DCF);

    fn market_dependencies(&self) -> finstack_core::Result<MarketDependencies> {
        MarketDependencies::from_curve_dependencies(self)
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> finstack_core::Result<Money> {
        pricer::compute_pv(self, market, as_of)
    }

    fn effective_start_date(&self) -> Option<Date> {
        None
    }

    fn pricing_overrides_mut(
        &mut self,
    ) -> Option<&mut crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&mut self.pricing_overrides)
    }

    fn pricing_overrides(
        &self,
    ) -> Option<&crate::instruments::pricing_overrides::PricingOverrides> {
        Some(&self.pricing_overrides)
    }
}
impl CurveDependencies for DiscountedCashFlow {
    fn curve_dependencies(&self) -> finstack_core::Result<InstrumentCurves> {
        InstrumentCurves::builder()
            .discount(self.discount_curve_id.clone())
            .build()
    }
}

impl crate::cashflow::traits::CashflowProvider for DiscountedCashFlow {
    fn cashflow_schedule(
        &self,
        _curves: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<crate::cashflow::builder::CashFlowSchedule> {
        let flows = self
            .flows
            .iter()
            .filter(|(date, _)| *date >= as_of)
            .map(|(date, amount)| (*date, Money::new(*amount, self.currency)))
            .collect();

        Ok(crate::cashflow::traits::schedule_from_dated_flows(
            flows,
            finstack_core::dates::DayCount::Act365F,
            crate::cashflow::traits::ScheduleBuildOpts {
                representation: crate::cashflow::builder::CashflowRepresentation::Projected,
                ..Default::default()
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::CashflowProvider;
    use crate::metrics::{MetricContext, MetricId};
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::{context::MarketContext, term_structures::DiscountCurve};
    use finstack_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn build_simple_dcf_gordon() -> DiscountedCashFlow {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
        let cf_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

        let discount_curve_id = CurveId::new("USD-OIS");

        DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-GORDON"),
            currency: Currency::USD,
            flows: vec![(cf_date, 100.0)],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth { growth_rate: 0.02 },
            net_debt: 0.0,
            valuation_date,
            discount_curve_id,
            mid_year_convention: false,
            equity_bridge: None,
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::default(),
        }
    }

    #[test]
    fn validate_accepts_simple_dcf() {
        assert!(build_simple_dcf_gordon().validate().is_ok());
    }

    #[test]
    fn validate_rejects_non_finite_wacc() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = f64::NAN;
        assert!(dcf.validate().is_err());
    }

    #[test]
    fn validate_rejects_wacc_at_minus_one() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = -1.0;
        let err = dcf.validate().expect_err("wacc=-1 must error");
        assert!(err.to_string().contains("(1 + wacc) <= 0"));
    }

    #[test]
    fn validate_rejects_wacc_above_sanity_band() {
        // 10 (1000%) is a typical typo for 0.10; the sanity band catches it.
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = 10.0;
        let err = dcf.validate().expect_err("wacc=10 must error");
        assert!(err.to_string().contains("sanity band"));
    }

    #[test]
    fn validate_rejects_unsorted_flows() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows = vec![
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                100.0,
            ),
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                100.0,
            ),
        ];
        let err = dcf.validate().expect_err("unsorted must error");
        assert!(err.to_string().contains("strictly increasing"));
    }

    #[test]
    fn validate_rejects_flow_on_or_before_valuation_date() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows = vec![(dcf.valuation_date, 100.0)];
        let err = dcf
            .validate()
            .expect_err("flow on valuation_date must error");
        assert!(err.to_string().contains("strictly after valuation_date"));
    }

    #[test]
    fn validate_rejects_non_positive_shares_outstanding() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.shares_outstanding = Some(0.0);
        assert!(dcf.validate().is_err());
        dcf.shares_outstanding = Some(-100.0);
        assert!(dcf.validate().is_err());
    }

    #[test]
    fn gordon_growth_terminal_value_matches_formula() {
        let dcf = build_simple_dcf_gordon();

        let tv = dcf
            .calculate_terminal_value()
            .expect("terminal value should succeed");

        // TV = FCF_terminal × (1 + g) / (WACC - g)
        let expected_tv = 100.0 * (1.0 + 0.02) / (0.10 - 0.02);
        let diff = (tv - expected_tv).abs();
        assert!(
            diff < 1e-9,
            "terminal value mismatch: got {}, expected {}, diff {}",
            tv,
            expected_tv,
            diff
        );
    }

    #[test]
    fn exit_multiple_terminal_value_matches_product() {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
        let cf_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

        let dcf = DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-EXIT"),
            currency: Currency::USD,
            flows: vec![(cf_date, 100.0)],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::ExitMultiple {
                terminal_metric: 150.0,
                multiple: 8.0,
            },
            net_debt: 0.0,
            valuation_date,
            discount_curve_id: CurveId::new("USD-OIS"),
            mid_year_convention: false,
            equity_bridge: None,
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::default(),
        };

        let tv = dcf
            .calculate_terminal_value()
            .expect("terminal value should succeed");
        let expected_tv = 150.0 * 8.0;
        assert!(
            (tv - expected_tv).abs() < 1e-9,
            "exit multiple TV mismatch: got {}, expected {}",
            tv,
            expected_tv
        );
    }

    #[test]
    fn npv_gordon_growth_matches_manual_calculation() {
        let dcf = build_simple_dcf_gordon();
        let market = MarketContext::new();
        let as_of = dcf.valuation_date;

        let money = dcf.value(&market, as_of).expect("value should succeed");
        let npv = money.amount();

        // Manual NPV:
        // PV_explicit = 100 / (1.10)^1
        // TV = 100 * (1.02) / (0.10 - 0.02)
        // PV_TV = TV / (1.10)^1
        let pv_explicit = 100.0 / 1.10_f64.powf(1.0);
        let tv = 100.0 * (1.0 + 0.02) / (0.10 - 0.02);
        let pv_tv = tv / 1.10_f64.powf(1.0);
        let expected_npv = pv_explicit + pv_tv;

        let diff = (npv - expected_npv).abs();
        // Allow for minor rounding differences introduced by Money's fixed-point representation.
        assert!(
            diff < 0.1,
            "NPV mismatch: got {}, expected {}, diff {}",
            npv,
            expected_npv,
            diff
        );
    }

    #[test]
    fn npv_errors_when_wacc_not_greater_than_growth() {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
        let cf_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

        let dcf = DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-BAD-G"),
            currency: Currency::USD,
            flows: vec![(cf_date, 100.0)],
            // WACC <= growth_rate should be rejected
            wacc: 0.02,
            terminal_value: TerminalValueSpec::GordonGrowth { growth_rate: 0.03 },
            net_debt: 0.0,
            valuation_date,
            discount_curve_id: CurveId::new("USD-OIS"),
            mid_year_convention: false,
            equity_bridge: None,
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            pricing_overrides: crate::instruments::PricingOverrides::default(),
            attributes: Attributes::default(),
        };

        let market = MarketContext::new();
        let as_of = valuation_date;

        let result = dcf.value(&market, as_of);
        assert!(
            result.is_err(),
            "expected validation error when WACC <= growth"
        );
    }

    #[test]
    fn required_discount_curves_and_has_discount_curve_are_consistent() {
        let dcf = build_simple_dcf_gordon();

        let required = dcf
            .market_dependencies()
            .expect("market_dependencies should succeed")
            .curve_dependencies()
            .discount_curves
            .clone();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], dcf.discount_curve_id);

        let deps = dcf.curve_dependencies().expect("curve_dependencies");
        assert_eq!(deps.discount_curves.len(), 1);
        assert_eq!(deps.discount_curves[0], dcf.discount_curve_id);
    }

    fn build_flat_discount_curve(id: &CurveId, as_of: Date, rate: f64) -> DiscountCurve {
        // Simple flat discount curve with a few knots approximating exp(-r t)
        let knots = [(0.0, 1.0), (1.0, (-rate).exp()), (5.0, (-rate * 5.0).exp())];

        DiscountCurve::builder(id.clone())
            .base_date(as_of)
            .knots(knots)
            .build()
            .expect("failed to build flat discount curve")
    }

    fn build_market_with_flat_curve(as_of: Date, curve_id: &CurveId, rate: f64) -> MarketContext {
        let curve = build_flat_discount_curve(curve_id, as_of, rate);
        MarketContext::new().insert(curve)
    }

    #[test]
    fn curve_present_discounts_at_curve_not_wacc() {
        let as_of =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid valuation date");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        // Curve at 5% is below the 10% WACC, so present-valuing on the curve must
        // yield a strictly higher value than the WACC fallback. This pins the
        // curve-present path and proves the loaded curve is the discount rate.
        let market_curve = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);
        let pv_curve = dcf.value(&market_curve, as_of).expect("curve pv");
        let pv_wacc = dcf
            .value(&MarketContext::new(), as_of)
            .expect("wacc fallback pv");

        assert!(
            pv_curve.amount() > pv_wacc.amount(),
            "curve(5%) discounting should exceed WACC(10%) fallback: curve={}, wacc={}",
            pv_curve.amount(),
            pv_wacc.amount()
        );

        // The pricer path must reconstruct exactly from the same curve discount
        // factors: terminal value is capitalized at WACC, then discounted on the
        // curve along with the explicit flows.
        let curve = market_curve
            .get_discount(&dcf.discount_curve_id)
            .expect("curve loaded");
        let tv = dcf.calculate_terminal_value().expect("terminal value");
        let mut expected_ev = 0.0;
        for (date, amount) in &dcf.flows {
            let years = dcf.discount_years(dcf.valuation_date, *date);
            expected_ev += amount * curve.df(years);
        }
        if let Some((tdate, _)) = dcf.flows.last() {
            let years = dcf.discount_years(dcf.valuation_date, *tdate);
            expected_ev += tv * curve.df(years);
        }
        // `build_simple_dcf_gordon` has zero net debt and no bridge/discounts, so
        // equity value equals enterprise value.
        assert!((pv_curve.amount() - expected_ev).abs() < 1e-6);
    }

    fn build_metric_context(
        dcf: DiscountedCashFlow,
        market: MarketContext,
        as_of: Date,
    ) -> MetricContext {
        let base_value = dcf.value(&market, as_of).expect("base value");
        let instrument: std::sync::Arc<dyn Instrument> = std::sync::Arc::new(dcf);
        MetricContext::new(
            instrument,
            std::sync::Arc::new(market),
            as_of,
            base_value,
            MetricContext::default_config(),
        )
    }

    #[test]
    fn dcf_theta_metric_computes() {
        let as_of =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test date for theta");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);
        let mut mctx = build_metric_context(dcf, market, as_of);

        let mut registry = crate::metrics::standard_registry().clone();
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(&mut registry);

        let results = registry
            .compute(&[MetricId::Theta], &mut mctx)
            .expect("theta metric should compute");

        assert!(
            results.contains_key(&MetricId::Theta),
            "Theta metric should be present for DCF"
        );
    }

    #[test]
    fn dcf_dv01_metric_computes_and_is_reasonable() {
        let as_of =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test date for dv01");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);
        let mut mctx = build_metric_context(dcf, market, as_of);

        let mut registry = crate::metrics::standard_registry().clone();
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(&mut registry);

        let results = registry
            .compute(&[MetricId::Dv01], &mut mctx)
            .expect("Dv01 metric should compute");

        let dv01 = *results
            .get(&MetricId::Dv01)
            .expect("Dv01 metric should be present");

        // Higher rates should reduce PV, so DV01 (PV_up - PV_base) should be <= 0
        assert!(
            dv01 <= 0.0,
            "DCF DV01 should be non-positive (PV decreases when rates increase), got {}",
            dv01
        );
    }

    #[test]
    fn dcf_bucketed_dv01_metric_computes() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid test date for bucketed dv01");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);
        let mut mctx = build_metric_context(dcf, market, as_of);

        let mut registry = crate::metrics::standard_registry().clone();
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(&mut registry);

        let results = registry
            .compute(&[MetricId::BucketedDv01], &mut mctx)
            .expect("BucketedDv01 metric should compute");

        assert!(
            results.contains_key(&MetricId::BucketedDv01),
            "BucketedDv01 metric should be present for DCF"
        );
    }

    #[test]
    fn dcf_cashflow_schedule_emits_projected_explicit_flows() {
        let dcf = build_simple_dcf_gordon();
        let schedule = dcf
            .cashflow_schedule(&MarketContext::new(), dcf.valuation_date)
            .expect("dcf schedule");

        assert_eq!(
            schedule.meta.representation,
            crate::cashflow::builder::CashflowRepresentation::Projected
        );
        assert_eq!(schedule.flows.len(), 1);
        assert_eq!(schedule.flows[0].date, dcf.flows[0].0);
        assert_eq!(schedule.flows[0].amount.amount(), dcf.flows[0].1);
        assert_eq!(schedule.flows[0].amount.currency(), Currency::USD);
    }

    // ──────────────────────────────────────────────────────────────────
    //  Mid-year convention tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn mid_year_convention_increases_pv() {
        let dcf_end = build_simple_dcf_gordon();
        let mut dcf_mid = build_simple_dcf_gordon();
        dcf_mid.mid_year_convention = true;

        let market = MarketContext::new();
        let val_end = dcf_end
            .value(&market, dcf_end.valuation_date)
            .expect("end-of-year value");
        let val_mid = dcf_mid
            .value(&market, dcf_mid.valuation_date)
            .expect("mid-year value");

        // Mid-year discounts at (t-0.5), so PV should be higher
        assert!(
            val_mid.amount() > val_end.amount(),
            "mid-year PV ({}) should exceed end-of-year PV ({})",
            val_mid.amount(),
            val_end.amount()
        );

        // The difference should be approximately (1+WACC)^0.5 - 1 ≈ 4.88%
        let ratio = val_mid.amount() / val_end.amount();
        let expected_ratio = (1.10_f64).powf(0.5);
        let diff = (ratio - expected_ratio).abs();
        assert!(
            diff < 0.01,
            "mid-year / end-of-year ratio ({:.4}) should be ~{:.4} (diff {})",
            ratio,
            expected_ratio,
            diff
        );
    }

    // ──────────────────────────────────────────────────────────────────
    //  Equity bridge tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn equity_bridge_matches_flat_net_debt_when_simple() {
        // EquityBridge with only total_debt and cash should match a flat net_debt
        let mut dcf_flat = build_simple_dcf_gordon();
        dcf_flat.net_debt = 50.0;

        let mut dcf_bridge = build_simple_dcf_gordon();
        dcf_bridge.net_debt = 999.0; // Should be ignored
        dcf_bridge.equity_bridge = Some(EquityBridge {
            total_debt: 80.0,
            cash: 30.0,
            ..Default::default()
        });

        let market = MarketContext::new();
        let val_flat = dcf_flat
            .value(&market, dcf_flat.valuation_date)
            .expect("flat");
        let val_bridge = dcf_bridge
            .value(&market, dcf_bridge.valuation_date)
            .expect("bridge");

        assert!(
            (val_flat.amount() - val_bridge.amount()).abs() < 0.1,
            "bridge (debt=80, cash=30, net=50) should match flat net_debt=50: flat={}, bridge={}",
            val_flat.amount(),
            val_bridge.amount()
        );
    }

    #[test]
    fn equity_bridge_with_preferred_and_minority() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.equity_bridge = Some(EquityBridge {
            total_debt: 100.0,
            cash: 20.0,
            preferred_equity: 30.0,
            minority_interest: 10.0,
            non_operating_assets: 5.0,
            other_adjustments: vec![("pension".into(), -15.0)], // negative = reduces equity
        });

        let bridge = dcf
            .equity_bridge
            .as_ref()
            .expect("equity_bridge should be set in this test");
        // net_adjustment = 100 - 20 + 30 + 10 - 5 - (-15) = 130
        let expected = 100.0 - 20.0 + 30.0 + 10.0 - 5.0 + 15.0;
        assert!(
            (bridge.net_adjustment() - expected).abs() < 1e-9,
            "bridge net_adjustment ({}) should be {}",
            bridge.net_adjustment(),
            expected
        );
    }

    // ──────────────────────────────────────────────────────────────────
    //  H-Model terminal value tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn h_model_terminal_value_matches_damodaran_formula() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.15,
            stable_growth_rate: 0.03,
            half_life_years: 5.0,
        };

        let tv = dcf
            .calculate_terminal_value()
            .expect("h-model terminal value");

        // Expected: stable_tv + growth_premium
        // stable_tv = 100 * (1.03) / (0.10 - 0.03) = 1471.43
        // growth_premium = 100 * 5.0 * (0.15 - 0.03) / (0.10 - 0.03) = 857.14
        // total = 2328.57
        let last_fcf = 100.0;
        let expected_stable = last_fcf * (1.0 + 0.03) / (0.10 - 0.03);
        let expected_premium = last_fcf * 5.0 * (0.15 - 0.03) / (0.10 - 0.03);
        let expected_tv = expected_stable + expected_premium;

        let diff = (tv - expected_tv).abs();
        assert!(
            diff < 1e-6,
            "H-model TV mismatch: got {}, expected {}, diff {}",
            tv,
            expected_tv,
            diff
        );
    }

    #[test]
    fn h_model_errors_when_wacc_not_greater_than_stable_growth() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = 0.02;
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.15,
            stable_growth_rate: 0.03,
            half_life_years: 5.0,
        };

        assert!(
            dcf.calculate_terminal_value().is_err(),
            "H-model should error when WACC <= stable_growth_rate"
        );
    }

    // ──────────────────────────────────────────────────────────────────
    //  Valuation discounts (DLOM / DLOC) tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn valuation_discounts_reduce_equity_value() {
        let dcf_base = build_simple_dcf_gordon();
        let mut dcf_disc = build_simple_dcf_gordon();
        dcf_disc.valuation_discounts = Some(ValuationDiscounts {
            dlom: Some(0.25),
            dloc: Some(0.15),
            other_discount: None,
        });

        let market = MarketContext::new();
        let val_base = dcf_base
            .value(&market, dcf_base.valuation_date)
            .expect("base value");
        let val_disc = dcf_disc
            .value(&market, dcf_disc.valuation_date)
            .expect("discounted value");

        // Expected: val_disc = val_base * (1 - 0.15) * (1 - 0.25)
        let expected = val_base.amount() * (1.0 - 0.15) * (1.0 - 0.25);
        let diff = (val_disc.amount() - expected).abs();
        assert!(
            diff < 0.1,
            "DLOM/DLOC mismatch: got {}, expected {}, diff {}",
            val_disc.amount(),
            expected,
            diff
        );
    }

    // ──────────────────────────────────────────────────────────────────
    //  Dilution / per-share tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn diluted_shares_treasury_stock_method() {
        // Build a DCF with equity value ~1,250 on a small share base
        // so that per-share price is high enough for ITM options.
        let mut dcf = build_simple_dcf_gordon();
        dcf.shares_outstanding = Some(100.0); // 100 shares => ~$12.50/share

        let market = MarketContext::new();
        let equity = dcf
            .value(&market, dcf.valuation_date)
            .expect("value")
            .amount();
        let pps = equity / 100.0;
        assert!(pps > 10.0, "sanity: price per share should be > $10");

        dcf.dilution_securities = vec![
            DilutionSecurity {
                name: "ITM Options".into(),
                quantity: 20.0,
                exercise_price: 5.0, // well in-the-money
            },
            DilutionSecurity {
                name: "OTM Warrants".into(),
                quantity: 10.0,
                exercise_price: 999.0, // far out-of-money
            },
        ];

        let diluted = dcf.diluted_shares(equity).expect("diluted shares");

        // Only the in-the-money options should dilute
        // incremental = 20 - (20 * 5.0) / pps
        let expected_incremental = 20.0 - (20.0 * 5.0) / pps;
        let expected_diluted = 100.0 + expected_incremental;

        let diff = (diluted - expected_diluted).abs();
        assert!(
            diff < 0.01,
            "diluted shares mismatch: got {:.4}, expected {:.4}, diff {:.4}",
            diluted,
            expected_diluted,
            diff
        );

        // Per-share value should be less than undiluted price
        let eps = dcf.equity_value_per_share(equity).expect("eps");
        assert!(
            eps < pps,
            "diluted EPS ({:.4}) should be less than basic PPS ({:.4})",
            eps,
            pps
        );
    }

    #[test]
    fn diluted_shares_none_when_no_shares() {
        let dcf = build_simple_dcf_gordon();
        assert!(dcf.diluted_shares(1000.0).is_none());
        assert!(dcf.equity_value_per_share(1000.0).is_none());
    }

    // ──────────────────────────────────────────────────────────────────
    //  Serde round-trip tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn serde_roundtrip_basic_dcf() {
        let dcf = build_simple_dcf_gordon();
        let json = serde_json::to_string(&dcf).expect("serialize");
        let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dcf.id, dcf2.id);
        assert!(!dcf2.mid_year_convention);
        assert!(dcf2.equity_bridge.is_none());
        assert!(dcf2.shares_outstanding.is_none());
        assert!(dcf2.dilution_securities.is_empty());
        assert!(dcf2.valuation_discounts.is_none());
    }

    #[test]
    fn serde_roundtrip_full_dcf() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.mid_year_convention = true;
        dcf.equity_bridge = Some(EquityBridge {
            total_debt: 100.0,
            cash: 20.0,
            preferred_equity: 30.0,
            minority_interest: 5.0,
            non_operating_assets: 10.0,
            other_adjustments: vec![("pension".into(), -8.0)],
        });
        dcf.shares_outstanding = Some(1_000_000.0);
        dcf.dilution_securities = vec![DilutionSecurity {
            name: "Options".into(),
            quantity: 50_000.0,
            exercise_price: 10.0,
        }];
        dcf.valuation_discounts = Some(ValuationDiscounts {
            dlom: Some(0.25),
            dloc: Some(0.15),
            other_discount: None,
        });

        let json = serde_json::to_string_pretty(&dcf).expect("serialize");
        let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");
        assert!(dcf2.mid_year_convention);
        assert!(dcf2.equity_bridge.is_some());
        assert_eq!(dcf2.shares_outstanding, Some(1_000_000.0));
        assert_eq!(dcf2.dilution_securities.len(), 1);
        assert!(dcf2.valuation_discounts.is_some());
        let disc = dcf2
            .valuation_discounts
            .expect("valuation_discounts should be present after roundtrip");
        assert_eq!(disc.dlom, Some(0.25));
        assert_eq!(disc.dloc, Some(0.15));
    }

    #[test]
    fn serde_old_json_without_new_fields_deserializes() {
        // Simulate an old-format JSON (no new fields)
        let json = r#"{
            "id": "TEST-OLD",
            "currency": "USD",
            "flows": [["2026-01-01", 100.0]],
            "wacc": 0.10,
            "terminal_value": {"type": "gordon_growth", "growth_rate": 0.02},
            "net_debt": 50.0,
            "valuation_date": "2025-01-01",
            "discount_curve_id": "USD-OIS",
            "attributes": {"tags": [], "meta": {}}
        }"#;

        let dcf: DiscountedCashFlow = serde_json::from_str(json).expect("old JSON should parse");
        assert_eq!(dcf.id.as_str(), "TEST-OLD");
        assert!(!dcf.mid_year_convention);
        assert!(dcf.equity_bridge.is_none());
        assert!(dcf.shares_outstanding.is_none());
        assert!(dcf.dilution_securities.is_empty());
        assert!(dcf.valuation_discounts.is_none());
    }

    #[test]
    fn serde_h_model_terminal_value_roundtrip() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.15,
            stable_growth_rate: 0.03,
            half_life_years: 5.0,
        };

        let json = serde_json::to_string(&dcf).expect("serialize");
        let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");

        match dcf2.terminal_value {
            TerminalValueSpec::HModel {
                high_growth_rate,
                stable_growth_rate,
                half_life_years,
            } => {
                assert!((high_growth_rate - 0.15).abs() < 1e-10);
                assert!((stable_growth_rate - 0.03).abs() < 1e-10);
                assert!((half_life_years - 5.0).abs() < 1e-10);
            }
            _ => panic!("Expected HModel terminal value after roundtrip"),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    //  Metric consistency: EV metric matches value() with curve
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn ev_metric_consistent_with_value_when_curve_present() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid test date for EV metric");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);

        // Get equity from value()
        let equity = dcf.value(&market, as_of).expect("value").amount();
        let bridge = dcf.effective_net_debt();
        let expected_ev = equity + bridge;

        // Get EV from metric
        let mut mctx = build_metric_context(dcf, market, as_of);
        let mut registry = crate::metrics::standard_registry().clone();
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(&mut registry);

        let results = registry
            .compute(&[MetricId::EnterpriseValue], &mut mctx)
            .expect("EV metric should compute");

        let ev_metric = *results
            .get(&MetricId::EnterpriseValue)
            .expect("EV metric should be present");

        // Allow for small rounding differences
        let diff = (ev_metric - expected_ev).abs();
        assert!(
            diff < 1.0,
            "EV metric ({}) should be close to value()+bridge ({}), diff {}",
            ev_metric,
            expected_ev,
            diff
        );
    }

    #[test]
    fn terminal_value_pv_metric_uses_curve_when_present() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid test date for terminal value pv metric");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &dcf.discount_curve_id, 0.05);
        let terminal_value = dcf.calculate_terminal_value().expect("terminal value");
        let (terminal_date, _) = dcf.flows.last().expect("at least one flow");
        let years = dcf.discount_years(dcf.valuation_date, *terminal_date);
        let curve_df = market
            .get_discount(&dcf.discount_curve_id)
            .expect("discount curve available")
            .df(years);
        let expected_pv_terminal = terminal_value * curve_df;

        let mut mctx = build_metric_context(dcf, market, as_of);
        let mut registry = crate::metrics::standard_registry().clone();
        crate::instruments::equity::dcf_equity::metrics::register_dcf_metrics(&mut registry);

        let results = registry
            .compute(&[MetricId::TerminalValuePV], &mut mctx)
            .expect("TerminalValuePV metric should compute");

        let pv_metric = *results
            .get(&MetricId::TerminalValuePV)
            .expect("TerminalValuePV metric should be present");
        let diff = (pv_metric - expected_pv_terminal).abs();
        assert!(
            diff < 1e-9,
            "TerminalValuePV metric ({}) should match curve-based PV ({}), diff {}",
            pv_metric,
            expected_pv_terminal,
            diff
        );
    }

    // ──────────────────────────────────────────────────────────────────
    //  Builder pattern tests
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn builder_with_all_new_fields() {
        let valuation_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let cf_date = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let dcf = DiscountedCashFlow::builder()
            .id(InstrumentId::new("BUILDER-TEST"))
            .currency(Currency::USD)
            .flows(vec![(cf_date, 100.0)])
            .wacc(0.10)
            .terminal_value(TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
            .net_debt(50.0)
            .valuation_date(valuation_date)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .mid_year_convention(true)
            .equity_bridge(EquityBridge {
                total_debt: 80.0,
                cash: 30.0,
                ..Default::default()
            })
            .shares_outstanding(1_000_000.0)
            .valuation_discounts(ValuationDiscounts {
                dlom: Some(0.20),
                ..Default::default()
            })
            .build()
            .expect("builder should succeed");

        assert!(dcf.mid_year_convention);
        assert!(dcf.equity_bridge.is_some());
        assert_eq!(dcf.shares_outstanding, Some(1_000_000.0));
        assert!(dcf.valuation_discounts.is_some());
    }

    // ──────────────────────────────────────────────────────────────────
    //  calculate_terminal_value error cases
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn calculate_terminal_value_errors_on_empty_flows() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows.clear();
        assert!(
            dcf.calculate_terminal_value().is_err(),
            "should error on empty flows"
        );
    }

    #[test]
    fn discount_terminal_value_errors_on_empty_flows() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows.clear();
        assert!(
            dcf.discount_terminal_value(100.0).is_err(),
            "should error on empty flows"
        );
    }

    #[test]
    fn exit_multiple_terminal_value_succeeds_with_empty_flows() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows.clear();
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: 500.0,
            multiple: 8.0,
        };
        let tv = dcf
            .calculate_terminal_value()
            .expect("ExitMultiple does not need flows");
        assert!((tv - 4000.0).abs() < 1e-10);
    }

    #[test]
    fn h_model_errors_when_high_growth_less_than_stable() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.02,
            stable_growth_rate: 0.05, // high < stable
            half_life_years: 5.0,
        };
        let result = dcf.calculate_terminal_value();
        assert!(
            result.is_err(),
            "H-model should error when high_growth_rate < stable_growth_rate"
        );
        let msg = result
            .expect_err("expected validation error when high_growth_rate < stable_growth_rate")
            .to_string();
        assert!(
            msg.contains("high_growth_rate"),
            "Error should mention high_growth_rate: {}",
            msg
        );
    }

    #[test]
    fn h_model_errors_when_half_life_zero_or_negative() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.15,
            stable_growth_rate: 0.03,
            half_life_years: 0.0, // invalid
        };
        assert!(
            dcf.calculate_terminal_value().is_err(),
            "H-model should error when half_life_years = 0"
        );

        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.15,
            stable_growth_rate: 0.03,
            half_life_years: -1.0, // invalid
        };
        assert!(
            dcf.calculate_terminal_value().is_err(),
            "H-model should error when half_life_years < 0"
        );
    }

    #[test]
    fn valuation_discounts_rejects_out_of_range() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_discounts = Some(ValuationDiscounts {
            dlom: Some(1.5), // out of range
            dloc: None,
            other_discount: None,
        });

        let market = MarketContext::new();
        let result = dcf.value(&market, dcf.valuation_date);
        assert!(result.is_err(), "should reject DLOM > 1.0");

        dcf.valuation_discounts = Some(ValuationDiscounts {
            dlom: None,
            dloc: Some(-0.1), // out of range
            other_discount: None,
        });
        let result = dcf.value(&market, dcf.valuation_date);
        assert!(result.is_err(), "should reject negative DLOC");
    }
}
