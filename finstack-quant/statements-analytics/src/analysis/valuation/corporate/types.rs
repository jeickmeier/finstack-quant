//! Corporate DCF inputs, options, projection specs, and valuation result types.

use super::dcf::{extract_currency_from_model, trailing_year_metric};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::{Error, Result};
use finstack_quant_valuations::instruments::equity::dcf_equity::{
    DiscountedCashFlow, EquityBridge, TerminalValueSpec, ValuationDiscounts,
};

/// Corporate valuation result containing DCF outputs.
///
/// Monetary outputs are returned in the model currency inferred from
/// `FinancialModelSpec::meta["currency"]`. Ratios such as
/// `equity_value_per_share` are plain scalars.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CorporateValuationResult {
    /// Equity value (EV - Net Debt, after discounts)
    pub equity_value: Money,
    /// Enterprise value (PV of all cash flows + terminal value)
    pub enterprise_value: Money,
    /// Interest-bearing debt less cash in model currency.
    pub net_debt: Money,
    /// Net non-debt claims to deduct from enterprise value in model currency.
    /// A negative balance increases common equity.
    pub other_equity_claims: Money,
    /// Total valuation discount (DLOM, DLOC, other) deducted after the
    /// EV-to-equity bridge; zero when `DcfOptions::valuation_discounts` is unset.
    ///
    /// `enterprise_value - net_debt - other_equity_claims - valuation_discount` equals
    /// `equity_value`, so a bridge built from these three steps reconciles to
    /// the reported equity value.
    pub valuation_discount: Money,
    /// Terminal value (present value)
    pub terminal_value_pv: Money,
    /// Equity value per diluted share (if shares_outstanding was provided)
    pub equity_value_per_share: Option<f64>,
    /// Diluted share count (if shares_outstanding was provided)
    pub diluted_shares: Option<f64>,
    /// Discount rate (WACC) applied to every explicit flow and to the
    /// terminal value, as an annually compounded decimal (`0.10` = 10%).
    ///
    /// `None` only for a result deserialized from JSON written before this
    /// field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wacc: Option<f64>,
    /// One row per explicit forecast period that was discounted, in date
    /// order: the free cash flow, its discounting tenor and factor, and its
    /// present value.
    ///
    /// The `present_value` column sums to [`pv_explicit`](Self::pv_explicit),
    /// and that sum plus [`terminal_value_pv`](Self::terminal_value_pv) is
    /// [`enterprise_value`](Self::enterprise_value). Rows carry unrounded
    /// `f64` amounts while the `Money` totals are rounded to the currency's
    /// minor unit, so the reconciliation holds to within one minor unit.
    /// Empty only for a result deserialized from JSON written before this
    /// field existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub periods: Vec<DcfPeriodRow>,
    /// Present value of the explicit forecast flows: the sum of
    /// `periods[i].present_value`.
    ///
    /// `None` only for a result deserialized from JSON written before this
    /// field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pv_explicit: Option<Money>,
    /// Terminal value at the horizon date, before discounting to the
    /// valuation date.
    ///
    /// `terminal_value / (1 + wacc)^terminal_discount_years` is
    /// [`terminal_value_pv`](Self::terminal_value_pv). `None` only for a
    /// result deserialized from JSON written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_value: Option<Money>,
    /// Discounting tenor of the terminal value in years (ACT/365.25 from the
    /// valuation date to the last explicit flow date).
    ///
    /// An exit-multiple terminal value always uses the full tenor; a
    /// growth-perpetuity terminal value uses the mid-year-adjusted tenor when
    /// `DcfOptions::mid_year_convention` is set. `None` only for a result
    /// deserialized from JSON written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_discount_years: Option<f64>,
    /// Gross debt, cash, preferred equity, minority interest, non-operating
    /// assets and named adjustments, in model currency.
    /// An authored flat net debt amount is recorded as debt with zero cash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equity_bridge: Option<EquityBridge>,
    /// The underlying DCF instrument (for further analysis).
    ///
    /// Not part of the wire form: it is skipped on serialization and
    /// deserializes as `None`.
    #[serde(skip)]
    pub dcf_instrument: Option<DiscountedCashFlow>,
}

/// One explicit forecast period of a DCF: the flow and how it was discounted.
///
/// Amounts are unrounded `f64` values in the model currency of the owning
/// [`CorporateValuationResult`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfPeriodRow {
    /// Statement period the flow was read from.
    pub period_id: PeriodId,
    /// Date the flow is discounted from: the last day of the period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Unlevered free cash flow of the period, in the model currency.
    pub free_cash_flow: f64,
    /// Discounting tenor in years: ACT/365.25 from the valuation date to
    /// `date`, less half the average flow spacing (floored at zero) under the
    /// mid-year convention.
    pub discount_years: f64,
    /// Discount factor `(1 + wacc)^-discount_years` (unitless).
    pub discount_factor: f64,
    /// Present value `free_cash_flow / (1 + wacc)^discount_years`, in the
    /// model currency.
    pub present_value: f64,
}

/// Node id the DCF entry points read unlevered free cash flow from when the
/// caller names none (`"ufcf"`).
///
/// The `dcf` step of [`CorporateAnalysisBuilder`](crate::analysis::CorporateAnalysisBuilder)
/// reads this node, and the host bindings use it as the default `ufcf_node`
/// of `dcf_sensitivity` / `evaluate_dcf`.
pub const DEFAULT_UFCF_NODE: &str = "ufcf";

/// Serde default of [`DcfInputs::ufcf_node`]: [`DEFAULT_UFCF_NODE`].
fn default_ufcf_node() -> String {
    DEFAULT_UFCF_NODE.to_string()
}

/// Headline DCF assumptions of [`evaluate_dcf`](crate::analysis::evaluate_dcf): discount rate, terminal value,
/// cash-flow node and optional net-debt override.
///
/// The serde form is the host input of `evaluate_dcf`: `wacc` and
/// `terminal_value` are required, `ufcf_node` defaults to
/// [`DEFAULT_UFCF_NODE`] and `net_debt_override` to `None`; an unknown field
/// is rejected. For example
/// `{"wacc": 0.10, "terminal_value": {"type": "gordon_growth", "stable_growth_rate": 0.02}}`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfInputs {
    /// Weighted average cost of capital as a decimal (`0.10` means `10%`).
    pub wacc: f64,
    /// Terminal-value convention applied after the explicit forecast period.
    pub terminal_value: TerminalValueSpec,
    /// Node id containing monetary unlevered free cash flow for forecast
    /// periods (default [`DEFAULT_UFCF_NODE`], `"ufcf"`).
    #[serde(default = "default_ufcf_node")]
    pub ufcf_node: String,
    /// Flat net-debt amount in model currency used instead of the
    /// model-derived equity bridge; `None` (the default) derives net debt
    /// from the model's debt and cash.
    #[serde(default)]
    pub net_debt_override: Option<f64>,
}

impl DcfInputs {
    /// Validate assumptions independent of statement data.
    ///
    /// # Errors
    /// Rejects a nonfinite WACC, WACC at or below −100%, an empty cashflow
    /// node identity, or a nonfinite override.
    pub fn validate(&self) -> Result<()> {
        if !self.wacc.is_finite() || self.wacc <= -1.0 {
            return Err(Error::invalid_input(
                "DCF WACC must be finite and above -100%",
            ));
        }
        if self.ufcf_node.trim().is_empty() {
            return Err(Error::invalid_input("DCF cashflow node must be named"));
        }
        if self
            .net_debt_override
            .is_some_and(|amount| !amount.is_finite())
        {
            return Err(Error::invalid_input("DCF net debt override must be finite"));
        }
        Ok(())
    }
}

/// Optional configuration for DCF valuation beyond the core WACC/terminal parameters.
///
/// Percentage-style inputs use decimal form, so `0.10` means `10%`.
/// [`Default`] caps perpetual stable growth at 5%.
///
/// The serde form is the host input for DCF options: every field is optional
/// and a missing field takes its [`Default`] value, while an unknown field is
/// rejected. For example `{"exit_multiple_bump": {"relative": 0.10}}` keeps
/// every other default and shocks the exit multiple by ±10%.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct DcfOptions {
    /// Enable mid-year discounting convention (default: false).
    pub mid_year_convention: bool,
    /// Structured equity bridge; when `Some` it is used as-is and the model's
    /// debt/cash (and any net-debt override) are not consulted.
    pub equity_bridge: Option<EquityBridge>,
    /// Basic shares outstanding for per-share value.
    pub shares_outstanding: Option<f64>,
    /// Private company valuation discounts (DLOM, DLOC).
    ///
    /// Discounts apply after the DCF EV-to-equity bridge. The reported DCF
    /// enterprise value remains pre-discount and will not reconcile to
    /// discounted equity value plus net debt when discounts are present.
    pub valuation_discounts: Option<ValuationDiscounts>,
    /// WACC sensitivity bump, in decimal (default `0.01` = ±100 bp).
    ///
    /// The down-shock is clamped so the terminal-value denominator
    /// `wacc - g` cannot collapse (the resulting EV would blow up and
    /// obscure the true sensitivity). See
    /// [`DcfOptions::wacc_denominator_epsilon`].
    pub wacc_sensitivity_bump: f64,
    /// Minimum spread between WACC-down and the Gordon / H-Model growth
    /// rate in the terminal-value formula (default `0.005` = 50 bp).
    ///
    /// When `wacc - bump < growth + epsilon`, the down-shock is clamped
    /// to `growth + epsilon` so the `1/(wacc - g)` denominator stays
    /// well defined. The clamp is reported in the trace so the caller
    /// knows the bump was shortened.
    pub wacc_denominator_epsilon: f64,
    /// Maximum perpetual stable growth rate accepted by Gordon Growth and the
    /// H-Model (default `0.05` = 5%).
    ///
    /// This is a policy ceiling, not merely the mathematical `g < WACC`
    /// constraint. Set it to the valuation's defensible long-run nominal
    /// economy or risk-free growth assumption.
    pub max_stable_growth_rate: f64,
    /// Exit-multiple sensitivity bump (default: `ExitMultipleBump::Absolute(1.0)`).
    ///
    /// Use `ExitMultipleBump::Relative` for a proportional shock,
    /// e.g. `Relative(0.10)` for ±10% of the base multiple. Absolute
    /// bumps are clamped at zero on the downside.
    pub exit_multiple_bump: ExitMultipleBump,
    /// Statement flow node whose complete trailing calendar year supplies the
    /// [`TerminalValueSpec::ExitMultiple`] metric (default: `None`).
    ///
    /// When `Some`, sum contiguous actual and forecast periods over the year ending
    /// at the last forecast boundary. Values must be finite and monetary. Missing
    /// history or a partial boundary requires an explicit annual metric. When `None`, the
    /// spec's explicit `terminal_metric` is used. Ignored for Gordon
    /// Growth and H-Model terminals.
    pub exit_multiple_metric_node: Option<String>,
}

impl Default for DcfOptions {
    fn default() -> Self {
        Self {
            mid_year_convention: false,
            equity_bridge: None,
            shares_outstanding: None,
            valuation_discounts: None,
            wacc_sensitivity_bump: 0.01,
            wacc_denominator_epsilon: 0.005,
            max_stable_growth_rate: 0.05,
            exit_multiple_bump: ExitMultipleBump::default(),
            exit_multiple_metric_node: None,
        }
    }
}

/// Exit-multiple sensitivity shock shape.
///
/// Absolute bumps are in multiple-turn units (e.g. `Absolute(1.0)` is
/// ±1.0x). Relative bumps are decimal fractions of the base multiple
/// (e.g. `Relative(0.10)` is ±10%).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExitMultipleBump {
    /// Absolute bump in turns of the multiple.
    Absolute(f64),
    /// Proportional bump in decimal (0.10 = ±10%).
    Relative(f64),
}

impl Default for ExitMultipleBump {
    fn default() -> Self {
        ExitMultipleBump::Absolute(1.0)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DcfEvalContext<'a> {
    pub(crate) net_debt_override: Option<f64>,
    pub(crate) options: &'a DcfOptions,
    pub(crate) projection: Option<&'a DcfProjection>,
}

/// Explicit timing basis for a point-in-time corporate valuation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementValuationCut {
    /// Effective calendar date from which projected cash receipts are discounted.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Latest publication date eligible for the statement run, inclusive.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub observed_through: Date,
    /// Single reporting currency shared by authored cashflows, bridges and the model.
    pub currency: Currency,
}

impl StatementValuationCut {
    fn check(&self, model: &FinancialModelSpec) -> Result<()> {
        if self.observed_through > self.as_of {
            return Err(Error::invalid_input(
                "DCF observation cut follows its valuation date",
            ));
        }
        if self.currency != extract_currency_from_model(model)? {
            return Err(Error::invalid_input(
                "DCF valuation currency must match the statement model",
            ));
        }
        Ok(())
    }

    pub(crate) fn evaluate(
        &self,
        model: &FinancialModelSpec,
        market: Option<&MarketContext>,
    ) -> Result<StatementResult> {
        use finstack_quant_statements::evaluator::ObservationCutoff;
        self.check(model)?;
        let empty_market = MarketContext::new();
        Evaluator::new().evaluate_with_market(
            model,
            market.unwrap_or(&empty_market),
            self.as_of,
            ObservationCutoff::Through(self.observed_through),
        )
    }

    pub(crate) fn check_result<'a>(
        &self,
        model: &FinancialModelSpec,
        results: &'a StatementResult,
    ) -> Result<&'a finstack_quant_statements::evaluator::RunProvenance> {
        use finstack_quant_statements::evaluator::ObservationCutoff;
        self.check(model)?;
        let provenance = results.provenance.as_ref().ok_or_else(|| {
            Error::invalid_input("Precomputed DCF requires identified statement run provenance")
        })?;
        if provenance.model_id != model.id
            || provenance.model_content_hash != model.content_hash()?
            || provenance.source_snapshot != model.source_snapshot
            || provenance.source_snapshot.is_none()
            || provenance.observation_cutoff != ObservationCutoff::Through(self.observed_through)
            || provenance.valuation_date != Some(self.as_of)
        {
            return Err(Error::invalid_input(
                "Statement run does not match the DCF model, snapshot, cut or valuation date",
            ));
        }
        Ok(provenance)
    }
}

/// Explicit source of normalized annual FCFF for a perpetual-growth terminal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalFcffBasis {
    /// Same-currency annualized FCFF authored directly for the terminal date.
    Authored {
        /// Signed FCFF per year, measured on a nominal currency basis.
        amount: Money,
    },
    /// Sum the complete 12 months of additive forecast FCFF at the horizon.
    TrailingAdditive,
}

/// Authored terminal horizon and optional normalized annual FCFF election.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DcfProjection {
    /// Terminal value is discounted to the same cut on its own ACT/365.25 tenor.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub terminal_date: Date,
    /// Required for a growth terminal and absent for an exit multiple.
    pub normalized_annual_fcff: Option<TerminalFcffBasis>,
    /// Exact monetary node for the forward portion of the one forecast period
    /// straddling the cut. Its modeled receipt is the end of that period.
    pub forward_remainder_node: Option<String>,
    /// Elected maximum annual stable growth as a decimal rate; absent means no ceiling.
    pub stable_growth_ceiling: Option<f64>,
}

/// A complete point-in-time corporate valuation request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CorporateDcfRequest {
    /// Explicit effective valuation date, observation cutoff and currency.
    pub cut: StatementValuationCut,
    /// WACC, terminal method and explicitly named operating cashflow.
    pub inputs: DcfInputs,
    /// Projection selection and normalized annual terminal basis.
    pub projection: DcfProjection,
    /// Additional equity bridge, dilution and timing assumptions.
    pub options: DcfOptions,
}

impl DcfProjection {
    pub(super) fn normalized_fcff(
        &self,
        model: &FinancialModelSpec,
        results: &StatementResult,
        terminal_value: &TerminalValueSpec,
        fcff_node: &str,
        currency: Currency,
        projection_end: Date,
    ) -> Result<Option<Money>> {
        match (&self.normalized_annual_fcff, terminal_value) {
            (None, TerminalValueSpec::ExitMultiple { .. }) => Ok(None),
            (Some(_), TerminalValueSpec::ExitMultiple { .. }) => Err(Error::invalid_input(
                "An exit multiple does not use normalized annual FCFF",
            )),
            (Some(TerminalFcffBasis::Authored { amount }), _) if amount.currency() == currency => {
                Ok(Some(*amount))
            }
            (Some(TerminalFcffBasis::Authored { .. }), _) => Err(Error::invalid_input(
                "Annual terminal FCFF must use the valuation currency",
            )),
            (Some(TerminalFcffBasis::TrailingAdditive), _) => {
                if self.terminal_date != projection_end {
                    return Err(Error::invalid_input(
                        "The terminal date must match the qualified trailing FCFF horizon",
                    ));
                }
                let end = self.terminal_date.next_day().ok_or_else(|| {
                    Error::invalid_input("Terminal FCFF horizon has no exclusive endpoint")
                })?;
                trailing_year_metric(model, results, fcff_node, end, currency).map(Some)
            }
            (None, _) => Err(Error::invalid_input(
                "Growth terminal valuation requires an explicit annual FCFF basis",
            )),
        }
    }
}
