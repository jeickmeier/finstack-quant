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
    fn diluted_shares_at_price(&self, basic: f64, price_per_share: f64) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_cashflows::CashflowProvider;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::{
        context::MarketContext, term_structures::DiscountCurve,
    };
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations_core::metrics::{MetricContext, MetricId};
    use time::Month;

    fn build_simple_dcf_gordon() -> DiscountedCashFlow {
        let valuation_date =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid test valuation date");
        let cf_date =
            Date::from_calendar_date(2026, Month::January, 1).expect("valid test cashflow date");

        DiscountedCashFlow {
            id: InstrumentId::new("TEST-DCF-GORDON"),
            currency: Currency::USD,
            flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            },
            valuation_date,
            forecast_timing: ForecastTiming::DatedReceipts,
            terminal_date: cf_date,
            normalized_annual_fcff: Some(Money::from((100_i64, Currency::USD))),
            equity_bridge: EquityBridge::default(),
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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
    fn validate_accepts_finite_positive_wacc_above_one() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = 1.1;
        assert!(dcf.validate().is_ok());
    }

    #[test]
    fn validate_accepts_wacc_above_minus_one_when_growth_is_lower() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.wacc = -0.70;
        dcf.terminal_value = TerminalValueSpec::GordonGrowth {
            stable_growth_rate: -0.80,
        };
        assert!(dcf.validate().is_ok());
    }

    #[test]
    fn validate_rejects_unsorted_flows() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows = vec![
            (
                Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                Money::from((100_i64, Currency::USD)),
            ),
            (
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
                Money::from((100_i64, Currency::USD)),
            ),
        ];
        let err = dcf.validate().expect_err("unsorted must error");
        assert!(err.to_string().contains("strictly ordered"));
    }

    #[test]
    fn validate_rejects_flow_on_or_before_valuation_date() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows = vec![(dcf.valuation_date, Money::from((100_i64, Currency::USD)))];
        let err = dcf
            .validate()
            .expect_err("flow on valuation_date must error");
        assert!(err.to_string().contains("after the valuation date"));
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
    fn validate_rejects_non_finite_terminal_value_params() {
        // NaN Gordon growth must fail validation (would bypass wacc > g).
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::GordonGrowth {
            stable_growth_rate: f64::NAN,
        };
        assert!(dcf.validate().is_err());

        // NaN H-Model parameters must fail validation.
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: f64::NAN,
            stable_growth_rate: 0.02,
            half_life_years: 5.0,
        };
        assert!(dcf.validate().is_err());

        // Non-finite exit-multiple inputs must fail validation.
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(f64::INFINITY),
            multiple: 10.0,
        };
        assert!(dcf.validate().is_err());
    }

    #[test]
    fn standalone_exit_multiple_requires_a_resolved_metric() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.normalized_annual_fcff = None;
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: None,
            multiple: 10.0,
        };
        let error = dcf.validate().expect_err("unresolved terminal metric");
        assert!(error.to_string().contains("requires an enterprise metric"));
    }

    #[test]
    fn validate_rejects_negative_exit_multiple_product() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.normalized_annual_fcff = None;
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(-100.0),
            multiple: 10.0,
        };
        let err = dcf.validate().expect_err("negative TV product must error");
        assert!(err.to_string().contains("nonnegative"));

        // A sign-consistent (non-negative) product passes.
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(100.0),
            multiple: 10.0,
        };
        assert!(dcf.validate().is_ok());
    }

    #[test]
    fn growth_requires_authored_normalized_annual_fcff_regardless_of_grid() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.normalized_annual_fcff = None;
        let error = dcf
            .validate()
            .expect_err("annual forecast does not establish terminal basis");
        assert!(error.to_string().contains("normalized annual FCFF"));
        dcf.normalized_annual_fcff = Some(Money::from((100_i64, Currency::USD)));
        assert!(dcf.validate().is_ok());
    }

    #[test]
    fn validate_rejects_terminal_fcff_in_a_different_currency() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.normalized_annual_fcff = Some(Money::from((100_i64, Currency::GBP)));
        assert!(dcf.validate().is_err());
    }

    #[test]
    fn terminal_value_guards_fail_closed_on_nan() {
        // NaN growth must error at terminal-value time, not produce NaN.
        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::GordonGrowth {
            stable_growth_rate: f64::NAN,
        };
        assert!(dcf.calculate_terminal_value().is_err());

        let mut dcf = build_simple_dcf_gordon();
        dcf.terminal_value = TerminalValueSpec::HModel {
            high_growth_rate: 0.10,
            stable_growth_rate: f64::NAN,
            half_life_years: 5.0,
        };
        assert!(dcf.calculate_terminal_value().is_err());
    }

    #[test]
    fn normalized_annual_fcff_drives_growth_perpetuity_tv() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.normalized_annual_fcff = Some(Money::from((400_i64, Currency::USD)));

        let tv = dcf.calculate_terminal_value().expect("terminal value");
        let expected = 400.0 * 1.02 / (0.10 - 0.02);
        assert!(
            (tv - expected).abs() < 1e-9,
            "TV should capitalize the override, expected {expected}, got {tv}"
        );

        // Exit multiples require their own separately authored metric.
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(50.0),
            multiple: 10.0,
        };
        assert!(dcf.calculate_terminal_value().is_err());
        dcf.normalized_annual_fcff = None;
        let tv = dcf.calculate_terminal_value().expect("terminal value");
        assert!((tv - 500.0).abs() < 1e-12);
    }

    #[test]
    fn asymmetric_forecast_periods_use_exact_midpoints_and_independent_terminal_date() {
        let mut dcf = build_simple_dcf_gordon();
        let date = dcf.valuation_date;
        let first_end = date + time::Duration::days(90);
        let final_end = date + time::Duration::days(365);
        dcf.flows = vec![
            (first_end, Money::from((15_i64, Currency::USD))),
            (final_end, Money::from((85_i64, Currency::USD))),
        ];
        dcf.forecast_timing = ForecastTiming::Midpoints {
            period_starts: vec![date, first_end],
        };
        dcf.terminal_date = final_end;
        dcf.normalized_annual_fcff = Some(Money::from((120_i64, Currency::USD)));
        dcf.wacc = 0.08;
        assert_eq!(
            dcf.explicit_flow_discount_years(first_end).unwrap(),
            45.0 / 365.25
        );
        assert_eq!(
            dcf.explicit_flow_discount_years(final_end).unwrap(),
            227.5 / 365.25
        );
        assert_eq!(dcf.terminal_discount_years().unwrap(), 365.0 / 365.25);
        assert!((dcf.calculate_pv_explicit_flows().unwrap() - 95.879998631628).abs() < 1e-10);
        let value = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, &|_| 0.0).unwrap();
        assert!((value - 1984.868391069042).abs() < 1e-9);
        let up = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, &|_| 0.0001).unwrap();
        let down = crate::dcf_equity::pricer::pv_with_rf_bump(&dcf, &|_| -0.0001).unwrap();
        assert!(((up - down) / 2.0 + 3.327952064358).abs() < 1e-9);
    }

    #[test]
    fn terminal_date_may_follow_the_explicit_forecast_but_cannot_precede_it() {
        let mut dcf = build_simple_dcf_gordon();
        let receipt = dcf.flows[0].0;
        dcf.terminal_date = receipt + time::Duration::days(2);
        assert_eq!(dcf.terminal_discount_years().unwrap(), 367.0 / 365.25);
        dcf.terminal_date = receipt - time::Duration::days(1);
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
            flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
            wacc: 0.10,
            terminal_value: TerminalValueSpec::ExitMultiple {
                terminal_metric: Some(150.0),
                multiple: 8.0,
            },
            valuation_date,
            forecast_timing: ForecastTiming::DatedReceipts,
            terminal_date: cf_date,
            normalized_annual_fcff: None,
            equity_bridge: EquityBridge::default(),
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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
            flows: vec![(cf_date, Money::from((100_i64, Currency::USD)))],
            // WACC <= stable_growth_rate should be rejected
            wacc: 0.02,
            terminal_value: TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.03,
            },
            valuation_date,
            forecast_timing: ForecastTiming::DatedReceipts,
            terminal_date: cf_date,
            normalized_annual_fcff: Some(Money::from((100_i64, Currency::USD))),
            equity_bridge: EquityBridge::default(),
            shares_outstanding: None,
            dilution_securities: Vec::new(),
            valuation_discounts: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
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

    /// A DCF discounts at its own WACC, so it declares no discount-curve
    /// dependency; rate sensitivity bumps the risk-free component of the WACC.
    #[test]
    fn dcf_declares_no_discount_curve_dependency() {
        let dcf = build_simple_dcf_gordon();

        let deps = dcf
            .market_dependencies()
            .expect("market_dependencies should succeed")
            .curves;
        assert!(
            deps.discount_curves.is_empty(),
            "DCF must not force callers to load a curve no computation reads, got {:?}",
            deps.discount_curves
        );
    }

    fn build_flat_discount_curve(id: &CurveId, as_of: Date, rate: f64) -> DiscountCurve {
        // Simple flat discount curve with a few knots approximating exp(-r t)
        let knots = [(0.0, 1.0), (1.0, (-rate).exp()), (5.0, (-rate * 5.0).exp())];

        DiscountCurve::builder(id.clone())
            .base_date(as_of)
            .knots(knots)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("failed to build flat discount curve")
    }

    fn build_market_with_flat_curve(as_of: Date, curve_id: &CurveId, rate: f64) -> MarketContext {
        let curve = build_flat_discount_curve(curve_id, as_of, rate);
        MarketContext::new()
            .try_insert(curve)
            .expect("insert market curve")
    }

    /// the PV must always discount at WACC; loading the
    /// named curve in the market context must not change the valuation.
    #[test]
    fn pv_identical_with_and_without_curve_loaded() {
        let as_of =
            Date::from_calendar_date(2025, Month::January, 1).expect("valid valuation date");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market_curve = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
        let pv_curve = dcf.value(&market_curve, as_of).expect("pv with curve");
        let pv_no_curve = dcf
            .value(&MarketContext::new(), as_of)
            .expect("pv without curve");

        assert!(
            (pv_curve.amount() - pv_no_curve.amount()).abs() < 1e-9,
            "PV must not depend on whether the curve is loaded: with={}, without={}",
            pv_curve.amount(),
            pv_no_curve.amount()
        );

        // And the WACC discounting must reconstruct exactly.
        let tv = dcf.calculate_terminal_value().expect("terminal value");
        let expected = dcf.calculate_pv_explicit_flows().unwrap()
            + dcf.discount_terminal_value(tv).expect("terminal pv");
        assert!((pv_no_curve.amount() - expected).abs() < 1e-6);
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

        let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
        let mut mctx = build_metric_context(dcf, market, as_of);

        let registry = crate::EQUITY.metrics();

        let results = registry
            .compute(&[MetricId::Theta], &mut mctx)
            .expect("theta metric should compute");

        assert!(
            results.contains_key(&MetricId::Theta),
            "Theta metric should be present for DCF"
        );
    }

    #[test]
    fn dcf_wacc01_metric_computes_and_is_reasonable() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid WACC01 date");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
        let mut mctx = build_metric_context(dcf, market, as_of);

        let registry = crate::EQUITY.metrics();

        let wacc01_id = MetricId::custom("dcf::wacc01");
        let results = registry
            .compute(std::slice::from_ref(&wacc01_id), &mut mctx)
            .expect("WACC01 metric should compute");

        let wacc01 = *results
            .get(&wacc01_id)
            .expect("WACC01 metric should be present");

        assert!(wacc01 < 0.0, "DCF WACC01 should be negative, got {wacc01}");

        // WACC01 equals the analytic dPV/dWACC per 1bp.
        // one-flow Gordon DCF: PV(w) = F/(1+w)^t + F(1+g)/((w-g)(1+w)^t).
        let f = 100.0;
        let g = 0.02;
        let w = 0.10;
        let t = {
            let mut d = build_simple_dcf_gordon();
            d.valuation_date = as_of;
            d.explicit_flow_discount_years(d.flows[0].0).unwrap()
        };
        let pv = |w: f64| (f + f * (1.0 + g) / (w - g)) / (1.0 + w).powf(t);
        let h = 1e-7;
        let analytic_per_bp = (pv(w + h) - pv(w - h)) / (2.0 * h) / 10_000.0;
        assert!(
            (wacc01 - analytic_per_bp).abs() < 1e-4 * analytic_per_bp.abs(),
            "WACC01 ({wacc01}) should equal analytic dPV/dWACC per bp ({analytic_per_bp})"
        );

        // And it must not depend on whether the curve is loaded.
        let mut dcf2 = build_simple_dcf_gordon();
        dcf2.valuation_date = as_of;
        let mut mctx_no_curve = build_metric_context(dcf2, MarketContext::new(), as_of);
        let results_no_curve = registry
            .compute(std::slice::from_ref(&wacc01_id), &mut mctx_no_curve)
            .expect("WACC01 should compute without curve");
        let wacc01_no_curve = *results_no_curve.get(&wacc01_id).expect("WACC01 present");
        assert!(
            (wacc01 - wacc01_no_curve).abs() < 1e-9,
            "WACC01 must not depend on curve presence: with={wacc01}, without={wacc01_no_curve}"
        );
    }

    #[test]
    fn dcf_cashflow_schedule_emits_projected_explicit_flows() {
        let dcf = build_simple_dcf_gordon();
        let schedule = dcf
            .cashflow_schedule(&MarketContext::new(), dcf.valuation_date)
            .expect("dcf schedule");

        assert_eq!(
            schedule.get_meta().representation,
            finstack_quant_cashflows::builder::CashflowRepresentation::Projected
        );
        assert_eq!(schedule.get_flows().len(), 1);
        assert_eq!(schedule.get_flows()[0].get_date(), dcf.flows[0].0);
        assert_eq!(
            schedule.get_flows()[0].get_amount().amount(),
            dcf.flows[0].1.amount()
        );
        assert_eq!(
            schedule.get_flows()[0].get_amount().currency(),
            Currency::USD
        );
    }

    //  Mid-year convention tests

    #[test]
    fn midpoint_timing_changes_only_the_explicit_forecast_pv() {
        let mut dcf = build_simple_dcf_gordon();
        let pv_end = dcf.calculate_pv_explicit_flows().unwrap();
        let terminal_value = dcf.calculate_terminal_value().unwrap();
        let terminal_pv_end = dcf.discount_terminal_value(terminal_value).unwrap();
        dcf.forecast_timing = ForecastTiming::Midpoints {
            period_starts: vec![dcf.valuation_date],
        };
        let expected_pv = 100.0 / 1.1_f64.powf(182.5 / 365.25);
        assert!((dcf.calculate_pv_explicit_flows().unwrap() - expected_pv).abs() < 1e-10);
        assert!(expected_pv > pv_end);
        assert_eq!(
            dcf.discount_terminal_value(terminal_value).unwrap(),
            terminal_pv_end
        );
    }

    //  Equity bridge tests

    #[test]
    fn equity_bridge_matches_flat_net_debt_when_simple() {
        // EquityBridge with only total_debt and cash should match a flat net_debt
        let mut dcf_flat = build_simple_dcf_gordon();
        dcf_flat.equity_bridge = EquityBridge {
            total_debt: 50.0,
            ..Default::default()
        };

        let mut dcf_bridge = build_simple_dcf_gordon();
        dcf_bridge.equity_bridge = EquityBridge {
            total_debt: 80.0,
            cash: 30.0,
            ..Default::default()
        };

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
        dcf.equity_bridge = EquityBridge {
            total_debt: 100.0,
            cash: 20.0,
            preferred_equity: 30.0,
            minority_interest: 10.0,
            non_operating_assets: 5.0,
            other_adjustments: vec![("pension".into(), -15.0)], // negative = reduces equity
        };

        let bridge = &dcf.equity_bridge;
        // net_adjustment = 100 - 20 + 30 + 10 - 5 - (-15) = 130
        let expected = 100.0 - 20.0 + 30.0 + 10.0 - 5.0 + 15.0;
        assert!(
            (bridge.net_adjustment() - expected).abs() < 1e-9,
            "bridge net_adjustment ({}) should be {}",
            bridge.net_adjustment(),
            expected
        );
    }

    //  H-Model terminal value tests

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

    //  Valuation discounts (DLOM / DLOC) tests

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

    //  Dilution / per-share tests

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

        // Only the in-the-money options dilute. TSM iterates to the
        // self-consistent diluted price p = E/d, so the fixed point solves
        //   d = basic + q - q * X * d / E
        // => d = (basic + q) / (1 + q * X / E)
        let expected_diluted = (100.0 + 20.0) / (1.0 + 20.0 * 5.0 / equity);

        let diff = (diluted - expected_diluted).abs();
        assert!(
            diff < 1e-6,
            "diluted shares mismatch: got {:.6}, expected {:.6}, diff {:.6}",
            diluted,
            expected_diluted,
            diff
        );

        // Self-consistency: dilution evaluated at the converged diluted
        // price must reproduce the converged share count.
        let converged_price = equity / diluted;
        let reproduced = dcf.diluted_shares_at_price(100.0, converged_price);
        assert!(
            (reproduced - diluted).abs() < 1e-6,
            "fixed point not self-consistent: {reproduced} vs {diluted}"
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
    fn large_warrant_issue_solves_piecewise_equity_equation() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.shares_outstanding = Some(100.0);
        dcf.dilution_securities = vec![DilutionSecurity {
            name: "Warrants".into(),
            quantity: 1000.0,
            exercise_price: 5.0,
        }];
        let shares = dcf.diluted_shares(1000.0).expect("diluted shares");
        assert!((shares - 183.33333333333333).abs() < 1e-10);
        assert!((1000.0 / shares - 6000.0 / 1100.0).abs() < 1e-12);
        assert_eq!(dcf.diluted_shares(500.0), Some(100.0));
        dcf.dilution_securities.push(DilutionSecurity {
            name: "RSUs".into(),
            quantity: 20.0,
            exercise_price: 0.0,
        });
        dcf.dilution_securities.push(DilutionSecurity {
            name: "OTM".into(),
            quantity: 1e6,
            exercise_price: 100.0,
        });
        let shares = dcf.diluted_shares(1000.0).expect("multiple classes");
        let price = 1000.0 / shares;
        assert!((120.0 * price + 1000.0 * (price - 5.0).max(0.0) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn discounts_and_dilution_preserve_signed_negative_common_equity() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.shares_outstanding = Some(100.0);
        dcf.valuation_discounts = Some(ValuationDiscounts {
            dlom: Some(0.20),
            ..ValuationDiscounts::default()
        });
        assert_eq!(dcf.apply_valuation_discounts(-25.0).unwrap(), -25.0);
        assert_eq!(dcf.apply_valuation_discounts(25.0).unwrap(), 20.0);
        assert_eq!(dcf.diluted_shares(-25.0), None);
        assert_eq!(dcf.equity_value_per_share(-25.0), None);
        assert_eq!(dcf.diluted_shares(0.0), Some(100.0));
        assert_eq!(dcf.equity_value_per_share(0.0), Some(0.0));
    }

    #[test]
    fn diluted_shares_none_when_no_shares() {
        let dcf = build_simple_dcf_gordon();
        assert!(dcf.diluted_shares(1000.0).is_none());
        assert!(dcf.equity_value_per_share(1000.0).is_none());
    }

    //  Serde round-trip tests

    #[test]
    fn serde_roundtrip_basic_dcf() {
        let dcf = build_simple_dcf_gordon();
        let json = serde_json::to_string(&dcf).expect("serialize");
        let dcf2: DiscountedCashFlow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(dcf.id, dcf2.id);
        assert_eq!(dcf2.forecast_timing, ForecastTiming::DatedReceipts);
        assert_eq!(dcf2.equity_bridge.net_adjustment(), 0.0);
        assert!(dcf2.shares_outstanding.is_none());
        assert!(dcf2.dilution_securities.is_empty());
        assert!(dcf2.valuation_discounts.is_none());
    }

    #[test]
    fn serde_roundtrip_full_dcf() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.forecast_timing = ForecastTiming::Midpoints {
            period_starts: vec![dcf.valuation_date],
        };
        dcf.equity_bridge = EquityBridge {
            total_debt: 100.0,
            cash: 20.0,
            preferred_equity: 30.0,
            minority_interest: 5.0,
            non_operating_assets: 10.0,
            other_adjustments: vec![("pension".into(), -8.0)],
        };
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
        assert_eq!(dcf2.forecast_timing, dcf.forecast_timing);
        assert_eq!(dcf2.equity_bridge.other_adjustments.len(), 1);
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
    fn old_wire_requires_authored_timing_and_terminal_basis() {
        let mut value = serde_json::to_value(build_simple_dcf_gordon()).unwrap();
        value.as_object_mut().unwrap().remove("forecast_timing");
        assert!(serde_json::from_value::<DiscountedCashFlow>(value).is_err());
    }

    #[test]
    fn validate_rejects_non_finite_equity_bridge() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.equity_bridge.total_debt = f64::NAN;
        let err = dcf.validate().expect_err("NaN total_debt must be rejected");
        assert!(
            err.to_string().contains("equity_bridge.total_debt"),
            "{err}"
        );

        let mut dcf = build_simple_dcf_gordon();
        dcf.equity_bridge.other_adjustments = vec![("pension".into(), f64::INFINITY)];
        assert!(dcf.validate().is_err());
    }

    fn minimal_dcf_json(extra: &str, terminal_value: &str) -> String {
        format!(
            r#"{{
            "id": "TEST",
            "currency": "USD",
            "flows": [["2026-01-01", 100.0]],
            "wacc": 0.10,
            "terminal_value": {terminal_value},
            "valuation_date": "2025-01-01",
            {extra}
            "attributes": {{"tags": [], "meta": {{}}}}
        }}"#
        )
    }

    #[test]
    // schema-rejection-test: retired `net_debt` (now `equity_bridge`)
    fn retired_net_debt_key_is_rejected() {
        let json = minimal_dcf_json(
            r#""net_debt": 50.0, "equity_bridge": {"total_debt": 50.0},"#,
            r#"{"type": "gordon_growth", "stable_growth_rate": 0.02}"#,
        );
        assert!(serde_json::from_str::<DiscountedCashFlow>(&json).is_err());
    }

    #[test]
    // schema-rejection-test: retired `growth_rate` (now `stable_growth_rate`)
    fn retired_gordon_growth_rate_key_is_rejected() {
        let json = minimal_dcf_json(
            r#""equity_bridge": {"total_debt": 50.0},"#,
            r#"{"type": "gordon_growth", "growth_rate": 0.02}"#,
        );
        assert!(serde_json::from_str::<DiscountedCashFlow>(&json).is_err());
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

    //  Metric consistency: EV metric matches value() with curve

    #[test]
    fn ev_metric_consistent_with_value_when_curve_present() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid test date for EV metric");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);

        // Get equity from value()
        let equity = dcf.value(&market, as_of).expect("value").amount();
        let bridge = dcf.effective_net_debt();
        let expected_ev = equity + bridge;

        // Get EV from metric
        let mut mctx = build_metric_context(dcf, market, as_of);
        let registry = crate::EQUITY.metrics();

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

    /// the terminal value PV metric discounts at WACC
    /// even when the named curve is loaded.
    #[test]
    fn terminal_value_pv_metric_uses_wacc_even_with_curve_present() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1)
            .expect("valid test date for terminal value pv metric");
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;

        let market = build_market_with_flat_curve(as_of, &CurveId::new("USD-OIS"), 0.05);
        let terminal_value = dcf.calculate_terminal_value().expect("terminal value");
        let expected_pv_terminal = dcf
            .discount_terminal_value(terminal_value)
            .expect("wacc-discounted terminal value");

        let mut mctx = build_metric_context(dcf, market, as_of);
        let registry = crate::EQUITY.metrics();

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

    /// Per-share metrics must fail loudly (not emit NaN) when
    /// `shares_outstanding` is unset, and compute when it is set.
    #[test]
    fn per_share_metrics_error_without_shares_outstanding() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let registry = crate::EQUITY.metrics();

        // Without shares_outstanding: both metrics error with a clear message.
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;
        let mut mctx = build_metric_context(dcf, MarketContext::new(), as_of);
        for metric in [MetricId::EquityShares, MetricId::EquityPricePerShare] {
            let err = registry
                .compute(std::slice::from_ref(&metric), &mut mctx)
                .expect_err("per-share metric must error without shares_outstanding");
            assert!(
                err.to_string().contains("shares_outstanding"),
                "error should name shares_outstanding, got: {err}"
            );
        }

        // With shares_outstanding: both metrics compute finite values.
        let mut dcf = build_simple_dcf_gordon();
        dcf.valuation_date = as_of;
        dcf.shares_outstanding = Some(100.0);
        let mut mctx = build_metric_context(dcf, MarketContext::new(), as_of);
        let results = registry
            .compute(
                &[MetricId::EquityShares, MetricId::EquityPricePerShare],
                &mut mctx,
            )
            .expect("per-share metrics should compute with shares set");
        for metric in [MetricId::EquityShares, MetricId::EquityPricePerShare] {
            let value = *results.get(&metric).expect("metric present");
            assert!(value.is_finite() && value > 0.0, "{metric:?} = {value}");
        }
    }

    //  Builder pattern tests

    #[test]
    fn builder_with_all_new_fields() {
        let valuation_date = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let cf_date = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let dcf = DiscountedCashFlow::builder()
            .id(InstrumentId::new("BUILDER-TEST"))
            .currency(Currency::USD)
            .flows(vec![(cf_date, Money::from((100_i64, Currency::USD)))])
            .wacc(0.10)
            .terminal_value(TerminalValueSpec::GordonGrowth {
                stable_growth_rate: 0.02,
            })
            .valuation_date(valuation_date)
            .forecast_timing(ForecastTiming::Midpoints {
                period_starts: vec![valuation_date],
            })
            .terminal_date(cf_date)
            .normalized_annual_fcff(Money::from((100_i64, Currency::USD)))
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

        assert!(matches!(
            dcf.forecast_timing,
            ForecastTiming::Midpoints { .. }
        ));
        assert_eq!(dcf.equity_bridge.net_adjustment(), 50.0);
        assert_eq!(dcf.shares_outstanding, Some(1_000_000.0));
        assert!(dcf.valuation_discounts.is_some());
    }

    //  calculate_terminal_value error cases

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
    fn exit_multiple_requires_an_explicit_projection() {
        let mut dcf = build_simple_dcf_gordon();
        dcf.flows.clear();
        dcf.normalized_annual_fcff = None;
        dcf.terminal_value = TerminalValueSpec::ExitMultiple {
            terminal_metric: Some(500.0),
            multiple: 8.0,
        };
        assert!(dcf.calculate_terminal_value().is_err());
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
            msg.contains("high growth"),
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
