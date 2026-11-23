//! Discounted Cash Flow instrument types and implementations.
//!
//! Core DCF types used for corporate valuation:
//! - [`TerminalValueSpec`] for Gordon Growth, Exit Multiple, and H-Model terminals
//! - [`DiscountedCashFlow`] instrument implementing the standard DCF formula

use crate::dcf_equity::pricer;
use finstack_quant_core::Error;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::impl_instrument_base;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::MarketDependencies;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Timing election for the dates of the explicit FCFF projection.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ForecastTiming {
    /// Discount every cash flow on its individually authored receipt date.
    DatedReceipts,
    /// Discount each signed FCFF observation on the exact elapsed midpoint
    /// of its authored period on ACT/365.25.
    Midpoints {
        /// Actual start date for each forecast period, aligned to its authored
        /// flow end date. A forward stub begins on the valuation date.
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Vec<finstack_quant_core::wire::DateWire>")
        )]
        period_starts: Vec<Date>,
    },
}

impl ForecastTiming {
    fn validate(
        &self,
        valuation_date: Date,
        flows: &[(Date, Money)],
    ) -> finstack_quant_core::Result<()> {
        if flows.windows(2).any(|rows| rows[0].0 >= rows[1].0)
            || flows.iter().any(|(date, _)| *date <= valuation_date)
        {
            return Err(Error::Validation(
                "DCF receipts must be strictly ordered and after the valuation date".into(),
            ));
        }
        let Self::Midpoints { period_starts } = self else {
            return Ok(());
        };
        if period_starts.len() != flows.len() {
            return Err(Error::Validation(
                "DCF midpoint timing needs an explicit start for every forecast flow".into(),
            ));
        }
        let mut previous = valuation_date;
        for (&start, &(end, _)) in period_starts.iter().zip(flows) {
            if start < previous || start >= end {
                return Err(Error::Validation("DCF midpoint periods must be positive and nonoverlapping; split a period crossing valuation into an authored forward stub".into()));
            }
            previous = end;
        }
        Ok(())
    }

    fn years(
        &self,
        valuation_date: Date,
        end: Date,
        index: usize,
    ) -> finstack_quant_core::Result<f64> {
        let end_days = (end - valuation_date).whole_days() as f64;
        match self {
            Self::DatedReceipts => Ok(end_days / 365.25),
            Self::Midpoints { period_starts } => {
                let start = period_starts.get(index).ok_or_else(|| {
                    Error::Validation("DCF midpoint period start is missing".into())
                })?;
                let start_days = (*start - valuation_date).whole_days() as f64;
                Ok((start_days + end_days) / (2.0 * 365.25))
            }
        }
    }
}

/// Terminal value calculation method for DCF.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalValueSpec {
    /// Gordon Growth Model: TV = FCF_terminal × (1 + g) / (WACC - g)
    GordonGrowth {
        /// Perpetual stable growth rate as an annual decimal (e.g., 0.02 for
        /// 2%). Must be < WACC.
        stable_growth_rate: f64,
    },
    /// Exit Multiple: TV = Terminal_Metric × Multiple
    ExitMultiple {
        /// Terminal metric in major currency units (for example, EBITDA).
        /// Required for standalone pricing. A statement DCF can resolve it from a metric node.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal_metric: Option<f64>,
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
/// Every [`DiscountedCashFlow`] carries one; a flat net-debt deduction is
/// written as `EquityBridge { total_debt, cash, ..Default::default() }`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
///
/// The enterprise value reported by a DCF remains pre-discount. When discounts
/// are present, `enterprise_value - net_debt` reconciles to pre-discount
/// equity value, not to the discounted fair-market equity value.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        fn check(name: &str, val: Option<f64>) -> finstack_quant_core::Result<()> {
            if let Some(v) = val
                && !(0.0..=1.0).contains(&v)
            {
                return Err(Error::Validation(format!(
                    "{name} must be in [0.0, 1.0], got {v:.6}"
                )));
            }
            Ok(())
        }
        check("dlom", self.dlom)?;
        check("dloc", self.dloc)?;
        check("other_discount", self.other_discount)?;
        Ok(())
    }

    /// Apply the selected discounts to a positive common-equity pool.
    ///
    /// # Arguments
    /// * `equity_value` - Finite signed common-equity residual in its native
    ///   currency. A nonpositive residual is retained at its reported amount.
    ///
    /// # Errors
    /// Rejects an unrepresentable input or a discount outside [0, 1].
    pub fn apply(&self, equity_value: f64) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if !equity_value.is_finite() {
            return Err(Error::Validation(
                "DCF common-equity residual must be finite".into(),
            ));
        }
        if equity_value <= 0.0 {
            return Ok(equity_value);
        }
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
/// [`equity_bridge`](Self::equity_bridge) is the single EV-to-equity channel; a
/// flat net-debt deduction sets only its `total_debt` and `cash`.
///
/// # Discounting dates
/// Forecast receipts use their authored dates or exact period midpoints.
/// Terminal value uses its own date on the same ACT/365.25 clock.
///
/// # Valuation Discounts
///
/// Private company valuations can apply DLOM, DLOC, and other discounts via
/// [`valuation_discounts`](Self::valuation_discounts). These discounts apply
/// after the EV-to-equity bridge, so the reported enterprise value remains a
/// pre-discount enterprise value and does not equal discounted equity value
/// plus net debt.
#[derive(
    Clone,
    Debug,
    finstack_quant_valuations_macros::FinancialBuilder,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct DiscountedCashFlow {
    /// Unique identifier for the DCF.
    pub id: InstrumentId,
    /// Currency for all cashflows.
    pub currency: Currency,
    /// Explicit period free cash flows (date, amount pairs).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Vec<(finstack_quant_core::wire::DateWire, Money)>")
    )]
    pub flows: Vec<(Date, Money)>,
    /// Weighted Average Cost of Capital (discount rate).
    pub wacc: f64,
    /// Terminal value specification.
    pub terminal_value: TerminalValueSpec,
    /// Valuation date (as-of date for the DCF).
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub valuation_date: Date,
    /// Authored receipt or actual midpoint timing for the explicit projection.
    pub forecast_timing: ForecastTiming,
    /// Terminal horizon on or after the final forecast end, independently discounted.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub terminal_date: Date,
    /// Normalized signed annual FCFF at the terminal date, in instrument currency.
    /// Required for Gordon and H-model, and absent for exit-multiple valuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_annual_fcff: Option<Money>,
    /// Equity bridge from enterprise value to equity value (debt, cash,
    /// preferred equity, minority interest, non-operating assets and other
    /// adjustments), in instrument currency.
    pub equity_bridge: EquityBridge,
    /// Basic shares outstanding for per-share value calculation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shares_outstanding: Option<f64>,
    /// Dilutive securities (options, warrants, RSUs, convertibles) for treasury stock method.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dilution_securities: Vec<DilutionSecurity>,
    /// Private company valuation discounts (DLOM, DLOC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valuation_discounts: Option<ValuationDiscounts>,
    /// Instrument-owned pricing inputs.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides::is_empty"
    )]
    pub instrument_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides::is_empty"
    )]
    pub metric_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    #[builder(default)]
    #[serde(
        default,
        skip_serializing_if = "finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides::is_empty"
    )]
    pub scenario_pricing_overrides:
        finstack_quant_valuations_core::instruments::pricing_overrides::ScenarioPricingOverrides,
    /// Attributes for scenario selection and tagging
    pub attributes: Attributes,
}

finstack_quant_valuations_core::validate_on_deserialize!(DiscountedCashFlow);

impl DiscountedCashFlow {
    /// Validate structural invariants required by the DCF pricer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `wacc` is non-finite or `wacc <= -1.0` (would yield `(1+wacc) <= 0`,
    ///   producing infinite / NaN present values)
    /// - the terminal horizon precedes the final forecast receipt
    /// - a growth terminal has no same-currency normalized annual FCFF
    /// - an exit-multiple terminal has no nonnegative metric or multiple
    /// - any `equity_bridge` amount (including `other_adjustments`) is non-finite
    /// - any explicit cash flow differs from the instrument currency
    /// - `flows` are not sorted by date (strictly increasing)
    /// - any flow date is on or before `valuation_date`
    /// - `shares_outstanding` is set but non-positive or non-finite
    /// - any `dilution_securities` entry has non-finite or negative quantity
    ///   or non-finite exercise price
    /// - the embedded `valuation_discounts` (when set) fails its own
    ///   `validate()` (each discount in `[0, 1]`)
    ///
    /// Terminal-value-specific checks (Gordon Growth requires
    /// `wacc > stable_growth_rate`; H-Model requires the same plus
    /// `high_growth_rate >= stable_growth_rate`) are enforced inside
    /// [`Self::calculate_terminal_value`] at pricing time, since they cross
    /// `wacc` with the terminal-value variant.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.wacc.is_finite() {
            return Err(Error::Validation(format!(
                "DCF '{}' wacc must be finite, got {}",
                self.id.as_str(),
                self.wacc
            )));
        }
        if self.wacc <= -1.0 {
            return Err(Error::Validation(format!(
                "DCF '{}' wacc = {} would make (1 + wacc) <= 0; must be > -1.0",
                self.id.as_str(),
                self.wacc
            )));
        }
        let bridge = &self.equity_bridge;
        for (field, value) in [
            ("total_debt", bridge.total_debt),
            ("cash", bridge.cash),
            ("preferred_equity", bridge.preferred_equity),
            ("minority_interest", bridge.minority_interest),
            ("non_operating_assets", bridge.non_operating_assets),
        ] {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "DCF '{}' equity_bridge.{} must be finite, got {}",
                    self.id.as_str(),
                    field,
                    value
                )));
            }
        }
        for (name, value) in &bridge.other_adjustments {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "DCF '{}' equity_bridge.other_adjustments '{}' must be finite, got {}",
                    self.id.as_str(),
                    name,
                    value
                )));
            }
        }
        if self
            .flows
            .iter()
            .any(|(_, money)| money.currency() != self.currency)
        {
            return Err(Error::Validation(
                "DCF explicit FCFF must match the instrument currency".into(),
            ));
        }
        self.forecast_timing
            .validate(self.valuation_date, &self.flows)?;
        if let Some(shares) = self.shares_outstanding
            && (!shares.is_finite() || shares <= 0.0)
        {
            return Err(Error::Validation(format!(
                "DCF '{}' shares_outstanding must be finite and positive, got {}",
                self.id.as_str(),
                shares
            )));
        }
        for (i, sec) in self.dilution_securities.iter().enumerate() {
            if !sec.quantity.is_finite() || sec.quantity < 0.0 {
                return Err(Error::Validation(format!(
                    "DCF '{}' dilution_securities[{}] quantity must be finite and non-negative, got {}",
                    self.id.as_str(),
                    i,
                    sec.quantity
                )));
            }
            if !sec.exercise_price.is_finite() || sec.exercise_price < 0.0 {
                return Err(Error::Validation(format!(
                    "DCF '{}' dilution_securities[{}] exercise_price must be finite and non-negative, got {}",
                    self.id.as_str(),
                    i,
                    sec.exercise_price
                )));
            }
        }
        if let Some(d) = &self.valuation_discounts {
            d.validate()?;
        }
        self.calculate_terminal_value()?;
        Ok(())
    }

    /// Create a representative tech company DCF example.
    ///
    /// 5-year FCF projections ($5M-$8M), 10% WACC, Gordon Growth terminal
    /// value at 2% growth, $15M net debt, mid-year convention enabled.
    pub fn example() -> finstack_quant_core::Result<Self> {
        let valuation_date = time::macros::date!(2025 - 01 - 01);

        let flows: Vec<(Date, Money)> = vec![
            (
                time::macros::date!(2026 - 01 - 01),
                Money::from((5_000_000_i64, Currency::USD)),
            ),
            (
                time::macros::date!(2027 - 01 - 01),
                Money::from((5_750_000_i64, Currency::USD)),
            ),
            (
                time::macros::date!(2028 - 01 - 01),
                Money::from((6_500_000_i64, Currency::USD)),
            ),
            (
                time::macros::date!(2029 - 01 - 01),
                Money::from((7_250_000_i64, Currency::USD)),
            ),
            (
                time::macros::date!(2030 - 01 - 01),
                Money::from((8_000_000_i64, Currency::USD)),
            ),
        ];

        Self::builder()
            .id(InstrumentId::new("DCF-TECH-CO"))
            .currency(Currency::USD)
            .flows(flows)
            .wacc(0.10)
            .terminal_value(TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            })
            .equity_bridge(EquityBridge {
                total_debt: 15_000_000.0,
                ..EquityBridge::default()
            })
            .valuation_date(valuation_date)
            .forecast_timing(ForecastTiming::Midpoints {
                period_starts: vec![
                    valuation_date,
                    time::macros::date!(2026 - 01 - 01),
                    time::macros::date!(2027 - 01 - 01),
                    time::macros::date!(2028 - 01 - 01),
                    time::macros::date!(2029 - 01 - 01),
                ],
            })
            .terminal_date(time::macros::date!(2030 - 01 - 01))
            .normalized_annual_fcff(Money::from((8_000_000_i64, Currency::USD)))
            .shares_outstanding_opt(Some(10_000_000.0))
            .attributes(Attributes::default())
            .build()
    }

    /// Present value of all explicit forecast FCFF on the authored ACT/365.25 calendar.
    ///
    /// # Errors
    /// Returns an error for an invalid date, discount rate or nonrepresentable value.
    pub fn calculate_pv_explicit_flows(&self) -> finstack_quant_core::Result<f64> {
        let mut total = finstack_quant_core::math::summation::NeumaierAccumulator::new();
        for &(date, amount) in &self.flows {
            total.add(super::pricer::discount_cashflow(
                amount.amount(),
                self.wacc,
                self.explicit_flow_discount_years(date)?,
            )?)?;
        }
        total.get_total()
    }

    /// ACT/365.25 discount tenor for an explicit forecast receipt.
    ///
    /// # Arguments
    /// * `date` - Exact end or receipt date identifying a forecast cash flow.
    ///
    /// # Errors
    /// Rejects an absent receipt date or missing midpoint start.
    pub fn explicit_flow_discount_years(&self, date: Date) -> finstack_quant_core::Result<f64> {
        let index = self
            .flows
            .binary_search_by_key(&date, |(receipt, _)| *receipt)
            .map_err(|_| {
                Error::Validation(format!(
                    "DCF cash flow on {date} is not in the explicit forecast"
                ))
            })?;
        self.forecast_timing.years(self.valuation_date, date, index)
    }

    /// Undiscounted enterprise terminal value at the explicitly authored horizon.
    ///
    /// # Errors
    /// Returns an error for a missing terminal basis, invalid growth or multiple
    /// assumptions, wrong currency or an unrepresentable value.
    pub fn calculate_terminal_value(&self) -> finstack_quant_core::Result<f64> {
        self.terminal_discount_years()?;
        self.terminal_value_at_wacc(self.wacc)
    }

    pub(crate) fn terminal_value_at_wacc(&self, wacc: f64) -> finstack_quant_core::Result<f64> {
        if !wacc.is_finite() || wacc <= -1.0 {
            return Err(Error::Validation(
                "DCF WACC must be finite and above -100%".into(),
            ));
        }
        let value = match self.terminal_value {
            TerminalValueSpec::GordonGrowth { stable_growth_rate } => {
                self.normalized_fcff()? * (1.0 + stable_growth_rate)
                    / Self::growth_spread(wacc, stable_growth_rate)?
            }
            TerminalValueSpec::HModel {
                stable_growth_rate,
                high_growth_rate,
                half_life_years,
            } => {
                if !(high_growth_rate.is_finite()
                    && high_growth_rate >= stable_growth_rate
                    && half_life_years.is_finite()
                    && half_life_years > 0.0)
                {
                    return Err(Error::Validation("DCF H-model requires finite high growth at least stable growth and a positive finite half-life".into()));
                }
                let factor = 1.0
                    + stable_growth_rate
                    + half_life_years * (high_growth_rate - stable_growth_rate);
                self.normalized_fcff()? * factor / Self::growth_spread(wacc, stable_growth_rate)?
            }
            TerminalValueSpec::ExitMultiple {
                terminal_metric,
                multiple,
            } => {
                if self.normalized_annual_fcff.is_some() {
                    return Err(Error::Validation(
                        "DCF exit multiple does not use normalized annual FCFF".into(),
                    ));
                }
                let metric = terminal_metric.ok_or_else(|| {
                    Error::Validation(
                        "DCF exit multiple requires an enterprise metric at the terminal horizon"
                            .into(),
                    )
                })?;
                if !(metric.is_finite() && metric >= 0.0 && multiple.is_finite() && multiple >= 0.0)
                {
                    return Err(Error::Validation(
                        "DCF exit metric and multiple must both be finite and nonnegative".into(),
                    ));
                }
                metric * multiple
            }
        };
        if !value.is_finite() {
            return Err(Error::Validation(
                "DCF terminal value cannot be represented".into(),
            ));
        }
        Ok(value)
    }

    fn growth_spread(wacc: f64, growth: f64) -> finstack_quant_core::Result<f64> {
        if !(growth.is_finite() && growth > -1.0 && growth < wacc) {
            return Err(Error::Validation(
                "DCF stable growth must be finite, above -100% and below WACC".into(),
            ));
        }
        Ok(wacc - growth)
    }

    fn normalized_fcff(&self) -> finstack_quant_core::Result<f64> {
        let amount = self.normalized_annual_fcff.ok_or_else(|| {
            Error::Validation("DCF growth terminal value requires normalized annual FCFF".into())
        })?;
        if amount.currency() != self.currency {
            return Err(Error::Validation(
                "DCF normalized annual FCFF must match the instrument currency".into(),
            ));
        }
        Ok(amount.amount())
    }

    /// Present value of the terminal component at the authored date.
    ///
    /// # Arguments
    /// * `terminal_value` - Undiscounted enterprise terminal value in instrument currency.
    ///
    /// # Errors
    /// Rejects an invalid horizon, WACC or unrepresentable discount factor or result.
    pub fn discount_terminal_value(&self, terminal_value: f64) -> finstack_quant_core::Result<f64> {
        super::pricer::discount_cashflow(terminal_value, self.wacc, self.terminal_discount_years()?)
    }

    /// Terminal ACT/365.25 tenor from the authored valuation date.
    ///
    /// # Errors
    /// Rejects an empty projection or a terminal date before its final cash flow.
    pub fn terminal_discount_years(&self) -> finstack_quant_core::Result<f64> {
        if self
            .flows
            .last()
            .is_none_or(|(end, _)| self.terminal_date < *end)
            || self.terminal_date <= self.valuation_date
        {
            return Err(Error::Validation("DCF terminal date must be after valuation and at or after the final forecast cash flow".into()));
        }
        Ok(self.year_fraction(self.valuation_date, self.terminal_date))
    }

    /// Effective net debt amount for the EV-to-equity bridge.
    ///
    /// Equals [`EquityBridge::net_adjustment`] of
    /// [`equity_bridge`](Self::equity_bridge), in instrument currency.
    pub fn effective_net_debt(&self) -> f64 {
        self.equity_bridge.net_adjustment()
    }

    /// Apply valuation discounts (DLOM, DLOC, other) to an equity value.
    ///
    /// Returns `Err` if any discount is outside `[0.0, 1.0]`.
    pub(crate) fn apply_valuation_discounts(
        &self,
        equity_value: f64,
    ) -> finstack_quant_core::Result<f64> {
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
    /// Solve `B*p + sum(Q*max(p-K, 0)) = equity_value` on sorted strike
    /// intervals, then verify the residual. No iterative convergence is assumed.
    ///
    /// # Arguments
    /// * `equity_value` - Post-discount equity value in the instrument currency,
    ///   before treasury-stock dilution. Zero retains basic shares and a
    ///   negative residual has no diluted share count under this method.
    ///
    /// Returns `None` when basic shares are absent or invalid, security inputs
    /// are invalid, or finite arithmetic cannot satisfy the valuation equation.
    pub fn diluted_shares(&self, equity_value: f64) -> Option<f64> {
        let basic = self.shares_outstanding?;
        if !basic.is_finite() || basic <= 0.0 || !equity_value.is_finite() || equity_value < 0.0 {
            return None;
        }
        let mut securities: Vec<_> = self.dilution_securities.iter().collect();
        if securities.iter().any(|security| {
            !security.quantity.is_finite()
                || security.quantity < 0.0
                || !security.exercise_price.is_finite()
                || security.exercise_price < 0.0
        }) {
            return None;
        }
        if equity_value == 0.0 {
            return Some(basic);
        }
        securities.sort_by(|left, right| left.exercise_price.total_cmp(&right.exercise_price));
        let mut shares = basic;
        let mut proceeds = 0.0;
        let mut price = equity_value / basic;
        for security in securities {
            if price <= security.exercise_price {
                break;
            }
            shares += security.quantity;
            proceeds += security.quantity * security.exercise_price;
            price = (equity_value + proceeds) / shares;
            if !price.is_finite() || price < security.exercise_price {
                return None;
            }
        }
        let diluted = self.diluted_shares_at_price(basic, price);
        let residual = (price * diluted - equity_value).abs();
        (diluted.is_finite() && residual <= 1e-10 * equity_value.abs().max(1.0)).then_some(diluted)
    }

    /// Treasury-stock-method diluted share count at a given price per share:
    /// each ITM security's exercise proceeds repurchase shares at that price.
    pub(super) fn diluted_shares_at_price(&self, basic: f64, price_per_share: f64) -> f64 {
        let mut diluted = basic;
        for sec in &self.dilution_securities {
            if sec.exercise_price < price_per_share && sec.quantity > 0.0 {
                let proceeds = sec.quantity * sec.exercise_price;
                let shares_repurchased = proceeds / price_per_share;
                diluted += sec.quantity - shares_repurchased;
            }
        }
        diluted
    }

    /// Equity value per diluted share.
    ///
    /// # Arguments
    /// * `equity_value` - Finite signed post-discount common equity in instrument currency.
    ///
    /// Returns `None` for negative equity or an unavailable diluted share count.
    /// At zero equity with positive basic shares the per-share price is zero.
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
}

impl Instrument for DiscountedCashFlow {
    fn includes_valuation_date_cashflows(&self) -> bool {
        true
    }
    impl_instrument_base!(InstrumentType::Dcf, crate::EQUITY);

    fn validate_invariants(&self) -> finstack_quant_core::Result<()> {
        self.validate()
    }

    fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
        let deps = MarketDependencies::new();
        Ok(deps)
    }

    fn base_value(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Money> {
        self.validate()?;
        pricer::compute_pv(self, market, as_of)
    }

    fn resolve_pricing_as_of(&self, _market: &MarketContext, _requested: Date) -> Date {
        self.valuation_date
    }

    finstack_quant_valuations_core::impl_focused_pricing_overrides!();
}
impl finstack_quant_cashflows::CashflowScheduleSource for DiscountedCashFlow {
    /// Projected cash-flow schedule containing only the explicit `flows` on
    /// or after `as_of`.
    ///
    /// The terminal value is intentionally excluded: it is a capitalized
    /// perpetuity (or point-in-time sale proxy), not a contractual cash
    /// flow, so it belongs in valuation output, not a cash ladder. As a
    /// consequence, discounting this schedule does **not** reconcile to
    /// [`Instrument::base_value`], which typically derives most of its PV
    /// from the terminal value. The `Act365F` day count below is schedule
    /// metadata only; pricing discounts on ACT/365.25 (see
    /// `Self::year_fraction`).
    fn raw_cashflow_schedule(
        &self,
        _curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_cashflows::builder::CashFlowSchedule> {
        let flows = self
            .flows
            .iter()
            .filter(|(date, _)| *date >= as_of)
            .copied()
            .collect();

        finstack_quant_cashflows::schedule_from_dated_flows(
            flows,
            finstack_quant_core::cashflow::CFKind::Fixed,
            finstack_quant_core::dates::DayCount::Act365F,
            finstack_quant_cashflows::ScheduleBuildOpts {
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    representation:
                        finstack_quant_cashflows::builder::CashflowRepresentation::Projected,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
    }
}
