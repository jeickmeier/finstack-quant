//! WACC calculation and 2D DCF sensitivity grid evaluation.

use super::dcf::*;
use super::types::*;
use crate::analysis::scenarios::TornadoEntry;
use crate::analysis::scenarios::sensitivity::descending_f64;
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::DecimalWire;
use finstack_quant_statements::Result;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};

/// Tornado parameter id for the discount-rate shock.
pub(super) const SENSITIVITY_PARAM_WACC: &str = "wacc";
/// Tornado parameter id for the terminal growth-rate shock.
pub(super) const SENSITIVITY_PARAM_TERMINAL_GROWTH: &str = "terminal_growth";
/// Tornado parameter id for the terminal exit-multiple shock.
pub(super) const SENSITIVITY_PARAM_EXIT_MULTIPLE: &str = "exit_multiple";

/// Threshold above which a shock is reported as clamped.
///
/// Matches the guard used by the in-line WACC sensitivity recorded in the
/// `evaluate_dcf*` explanation trace, so both surfaces flag the same shocks.
pub(super) const SENSITIVITY_CLAMP_EPSILON: f64 = 1e-12;

/// Enterprise-value tornado for the headline DCF assumptions.
///
/// Monetary fields are expressed in the model currency inferred from
/// `FinancialModelSpec::meta["currency"]`. Rates are decimal fractions
/// (`0.10` means `10%`) and multiples are plain scalars (`9.5` means `9.5x`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct DcfSensitivityResult {
    /// Unshocked enterprise value the tornado deltas are measured against.
    pub baseline_enterprise_value: Money,
    /// Tornado entries sorted by descending absolute swing (`NaN` last).
    ///
    /// `downside` and `upside` are enterprise-value **deltas** versus
    /// `baseline_enterprise_value`, following the convention of
    /// [`crate::analysis::generate_tornado_entries`]: `downside` is the
    /// impact with the parameter at its shocked minimum and `upside` the
    /// impact at its shocked maximum. A lower WACC raises enterprise value,
    /// so the WACC entry's `downside` is typically positive.
    pub entries: Vec<TornadoEntry>,
    /// Effective down-shocked WACC after the growth-denominator clamp.
    pub wacc_down: f64,
    /// `true` when the WACC down-shock was shortened by the clamp.
    pub wacc_down_clamped: bool,
    /// Effective up-shocked terminal growth rate, when a growth-perpetuity
    /// terminal value is in use (`None` for exit-multiple terminal values).
    pub terminal_growth_up: Option<f64>,
    /// `true` when the terminal-growth up-shock was shortened by the clamp.
    pub terminal_growth_up_clamped: bool,
}

/// Rank the headline DCF assumptions by their enterprise-value impact.
///
/// The statement model is evaluated **once**; every shocked point re-runs only
/// the DCF over the cached [`StatementResult`](finstack_quant_statements::evaluator::StatementResult), mirroring
/// [`crate::analysis::CorporateAnalysisBuilder`]. Three parameters are covered:
///
/// | Parameter id | Shock | Source of the bump |
/// |---|---|---|
/// | `wacc` | `wacc ± bump` | [`DcfOptions::wacc_sensitivity_bump`] |
/// | `terminal_growth` | growth rate `± bump` | [`DcfOptions::wacc_sensitivity_bump`] |
/// | `exit_multiple` | multiple `± bump` | [`DcfOptions::exit_multiple_bump`] |
///
/// `terminal_growth` is emitted only for growth-perpetuity terminal values
/// ([`TerminalValueSpec::GordonGrowth`], [`TerminalValueSpec::HModel`], where
/// the stable growth rate is shocked and the high growth rate is held at or
/// above it); `exit_multiple` only for [`TerminalValueSpec::ExitMultiple`].
///
/// # Denominator clamping
///
/// The Gordon / H-Model terminal multiplier `1/(WACC − g)` diverges as the
/// spread collapses, which would swamp the tornado with a meaningless bar.
/// Both shocks that narrow the spread are therefore clamped to leave at least
/// [`DcfOptions::wacc_denominator_epsilon`] between the two rates: the WACC
/// down-shock is floored at `g + epsilon` and the growth up-shock is capped at
/// `WACC − epsilon`. Whenever a clamp binds it is reported through
/// `wacc_down_clamped` / `terminal_growth_up_clamped` and the effective shocked
/// levels through `wacc_down` / `terminal_growth_up`, so a shortened bump is
/// never silently attributed to a genuine sensitivity.
///
/// # Arguments
///
/// * `model` - Statement model containing forecast periods plus a currency in
///   metadata
/// * `wacc` - Baseline discount rate in decimal form (`0.10` means `10%`)
/// * `terminal_value` - Baseline terminal-value convention; determines which
///   terminal parameter is shocked
/// * `ufcf_node` - Node id containing unlevered free cash flow for forecast
///   periods
/// * `net_debt_override` - Optional flat net-debt amount used instead of the
///   model-derived bridge; enterprise value is independent of it, but the
///   underlying DCF still requires either the override or balance-sheet nodes
/// * `options` - Supplies the shock sizes and the denominator epsilon, and is
///   applied unchanged to every shocked re-run
///
/// # Returns
///
/// Returns [`DcfSensitivityResult`] with the baseline enterprise value, the
/// ranked tornado entries, and the effective (possibly clamped) shock levels.
///
/// # Errors
///
/// Returns an error if the model cannot be evaluated, if `ufcf_node` has no
/// forecast cash flows, if the model currency cannot be inferred, or if the
/// baseline or any shocked terminal-value assumption is internally
/// inconsistent (for example a growth rate at or above WACC).
///
/// # Examples
///
/// ```
/// use finstack_quant_statements_analytics::analysis::{dcf_sensitivity, DcfOptions};
/// use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
///
/// # fn run(model: &finstack_quant_statements::types::FinancialModelSpec)
/// #     -> Result<(), Box<dyn std::error::Error>> {
/// let tornado = dcf_sensitivity(
///     model,
///     0.10,
///     TerminalValueSpec::GordonGrowth { stable_growth_rate: 0.02 },
///     "ufcf",
///     Some(0.0),
///     &DcfOptions::default())?;
///
/// // Entries are ranked by absolute swing, widest first.
/// assert!(!tornado.entries.is_empty());
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Koller, T., Goedhart, M., & Wessels, D. (2020). *Valuation: Measuring and
///   Managing the Value of Companies* (7th ed.). Wiley. Ch. 15 (continuing
///   value) on the sensitivity of `1/(WACC − g)` to the assumed spread. `docs/REFERENCES.md#koller-valuation`
/// - Damodaran, A. (2012). *Investment Valuation* (3rd ed.). Wiley. Ch. 12 on
///   stable-growth constraints (`g < WACC`).
pub fn dcf_sensitivity(
    model: &FinancialModelSpec,
    wacc: f64,
    terminal_value: TerminalValueSpec,
    ufcf_node: &str,
    net_debt_override: Option<f64>,
    options: &DcfOptions,
) -> Result<DcfSensitivityResult> {
    let currency = extract_currency_from_model(model)?;

    // Evaluate the statement model exactly once; each shocked point below
    // re-runs only the DCF over these cached results.
    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(model)?;
    let context = DcfEvalContext {
        net_debt_override,
        options,
        projection: None,
    };

    let enterprise_value = |terminal: TerminalValueSpec, discount_rate: f64| -> Result<f64> {
        let result = evaluate_dcf_from_results_impl(
            model,
            &results,
            discount_rate,
            terminal,
            ufcf_node,
            context,
            None,
        )?;
        Ok(result.enterprise_value.amount())
    };

    let baseline = enterprise_value(terminal_value.clone(), wacc)?;

    let bump = options.wacc_sensitivity_bump.abs();
    let epsilon = options.wacc_denominator_epsilon.max(0.0);
    let growth_floor: f64 = match &terminal_value {
        TerminalValueSpec::GordonGrowth { stable_growth_rate }
        | TerminalValueSpec::HModel {
            stable_growth_rate, ..
        } => *stable_growth_rate,
        TerminalValueSpec::ExitMultiple { .. } => f64::NEG_INFINITY,
    };

    // WACC shock. The down-shock is floored so `1/(wacc - g)` stays defined.
    let wacc_up = wacc + bump;
    let wacc_down_raw = wacc - bump;
    let wacc_down = wacc_down_raw.max((growth_floor + epsilon).max(epsilon));
    let wacc_down_clamped = (wacc_down - wacc_down_raw).abs() > SENSITIVITY_CLAMP_EPSILON;

    let mut entries = vec![TornadoEntry {
        parameter_id: SENSITIVITY_PARAM_WACC.to_string(),
        downside: enterprise_value(terminal_value.clone(), wacc_down)? - baseline,
        upside: enterprise_value(terminal_value.clone(), wacc_up)? - baseline,
    }];

    // Terminal-parameter shock. The growth up-shock is capped symmetrically
    // against the same denominator epsilon.
    let growth_ceiling = wacc - epsilon;
    let mut terminal_growth_up = None;
    let mut terminal_growth_up_clamped = false;

    match &terminal_value {
        TerminalValueSpec::GordonGrowth { stable_growth_rate } => {
            let up_raw = stable_growth_rate + bump;
            let up = up_raw.min(growth_ceiling);
            terminal_growth_up = Some(up);
            terminal_growth_up_clamped = (up - up_raw).abs() > SENSITIVITY_CLAMP_EPSILON;
            entries.push(TornadoEntry {
                parameter_id: SENSITIVITY_PARAM_TERMINAL_GROWTH.to_string(),
                downside: enterprise_value(
                    TerminalValueSpec::GordonGrowth {
                        stable_growth_rate: stable_growth_rate - bump,
                    },
                    wacc,
                )? - baseline,
                upside: enterprise_value(
                    TerminalValueSpec::GordonGrowth {
                        stable_growth_rate: up,
                    },
                    wacc,
                )? - baseline,
            });
        }
        TerminalValueSpec::HModel {
            high_growth_rate,
            stable_growth_rate,
            half_life_years,
        } => {
            let up_raw = stable_growth_rate + bump;
            let up = up_raw.min(growth_ceiling);
            terminal_growth_up = Some(up);
            terminal_growth_up_clamped = (up - up_raw).abs() > SENSITIVITY_CLAMP_EPSILON;
            let down = stable_growth_rate - bump;
            entries.push(TornadoEntry {
                parameter_id: SENSITIVITY_PARAM_TERMINAL_GROWTH.to_string(),
                downside: enterprise_value(
                    TerminalValueSpec::HModel {
                        // The H-Model requires `high >= stable`; hold the high
                        // rate at the shocked stable rate when the shock would
                        // otherwise invert them.
                        high_growth_rate: high_growth_rate.max(down),
                        stable_growth_rate: down,
                        half_life_years: *half_life_years,
                    },
                    wacc,
                )? - baseline,
                upside: enterprise_value(
                    TerminalValueSpec::HModel {
                        high_growth_rate: high_growth_rate.max(up),
                        stable_growth_rate: up,
                        half_life_years: *half_life_years,
                    },
                    wacc,
                )? - baseline,
            });
        }
        TerminalValueSpec::ExitMultiple {
            terminal_metric,
            multiple,
        } => {
            let shock = match options.exit_multiple_bump {
                ExitMultipleBump::Absolute(b) => b.abs(),
                ExitMultipleBump::Relative(r) => multiple.abs() * r.abs(),
            };
            entries.push(TornadoEntry {
                parameter_id: SENSITIVITY_PARAM_EXIT_MULTIPLE.to_string(),
                downside: enterprise_value(
                    TerminalValueSpec::ExitMultiple {
                        terminal_metric: *terminal_metric,
                        // Absolute bumps are clamped at zero on the downside;
                        // a negative multiple has no economic meaning.
                        multiple: (multiple - shock).max(0.0),
                    },
                    wacc,
                )? - baseline,
                upside: enterprise_value(
                    TerminalValueSpec::ExitMultiple {
                        terminal_metric: *terminal_metric,
                        multiple: multiple + shock,
                    },
                    wacc,
                )? - baseline,
            });
        }
    }

    entries.sort_by(|lhs, rhs| descending_f64(lhs.swing().abs(), rhs.swing().abs()));

    Ok(DcfSensitivityResult {
        baseline_enterprise_value: Money::new(baseline, currency)?,
        entries,
        wacc_down,
        wacc_down_clamped,
        terminal_growth_up,
        terminal_growth_up_clamped,
    })
}

/// Capital-structure inputs of a [`wacc`] calculation.
///
/// All rates follow the crate-wide decimal convention (`0.10` means `10%`);
/// weights are decimal fractions of total capital and must sum to one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaccInputs {
    /// Equity share of total capital as a decimal fraction (`0.6` means 60%
    /// equity-funded); non-negative.
    pub equity_weight: DecimalWire,
    /// Required return on equity in decimal form, typically from CAPM
    /// (`0.115` means `11.5%`).
    pub cost_of_equity: f64,
    /// Debt share of total capital as a decimal fraction (`0.4` means 40%
    /// debt-funded); non-negative, and sums with `equity_weight` to `1.0`.
    pub debt_weight: DecimalWire,
    /// **Pre-tax** cost of debt in decimal form: the marginal borrowing yield
    /// before the tax shield (`0.06` means `6%`).
    pub cost_of_debt: f64,
    /// Marginal corporate tax rate as a decimal fraction in `[0, 1]` (`0.25`
    /// means `25%`).
    pub tax_rate: f64,
}

/// Weighted-average cost of capital (WACC).
///
/// Blends the required return on equity with the *after-tax* cost of debt at
/// the firm's target capital structure:
///
/// ```text
/// WACC = w_E · r_E + w_D · r_D · (1 − T)
/// ```
///
/// The `(1 − T)` factor is the interest tax shield: because interest is
/// deductible, the cash cost of debt to the firm is lower than the coupon
/// investors receive. Equity carries no equivalent shield, so `cost_of_equity`
/// enters untaxed. Pass market-value weights, not book-value weights, for a
/// valuation discount rate. The result is the rate to feed to
/// [`evaluate_dcf`] or [`dcf_sensitivity`].
///
/// # Arguments
///
/// * `inputs` - Capital weights, the cost of equity, the pre-tax cost of debt
///   and the marginal tax rate, all decimals (see [`WaccInputs`]).
///
/// # Returns
///
/// Returns the blended discount rate as a decimal fraction.
///
/// # Errors
///
/// Returns an error if a cost is nonfinite or at or below -100%, if either weight is
/// negative, if the weights do not sum exactly to one, if
/// `tax_rate` falls outside `[0, 1]`, or if the annual effective result
/// is non-finite or at or below -100%.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::wire::DecimalWire;
/// use finstack_quant_statements_analytics::analysis::{wacc, WaccInputs};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // 60% equity at 11.5%, 40% debt at 6% pre-tax, 25% tax rate.
/// let rate = wacc(&WaccInputs {
///     equity_weight: DecimalWire("0.6".parse()?),
///     cost_of_equity: 0.115,
///     debt_weight: DecimalWire("0.4".parse()?),
///     cost_of_debt: 0.06,
///     tax_rate: 0.25,
/// })?;
/// assert!((rate - 0.087).abs() < 1e-12);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Modigliani, F., & Miller, M. H. (1958). "The Cost of Capital, Corporation
///   Finance and the Theory of Investment." *American Economic Review*, 48(3),
///   261-297.
/// - Modigliani, F., & Miller, M. H. (1963). "Corporate Income Taxes and the
///   Cost of Capital: A Correction." *American Economic Review*, 53(3),
///   433-443 — the source of the `(1 − T)` debt tax shield.
/// - Koller, T., Goedhart, M., & Wessels, D. (2020). *Valuation: Measuring and
///   Managing the Value of Companies* (7th ed.). Wiley. Ch. 13. `docs/REFERENCES.md#koller-valuation`
pub fn wacc(inputs: &WaccInputs) -> Result<f64> {
    let WaccInputs {
        equity_weight: DecimalWire(equity),
        cost_of_equity,
        debt_weight: DecimalWire(debt),
        cost_of_debt,
        tax_rate,
    } = *inputs;
    let invalid = |message: &str| finstack_quant_statements::Error::Eval(message.into());
    if equity < Decimal::ZERO || debt < Decimal::ZERO {
        return Err(invalid("WACC requires non-negative capital weights"));
    }
    // With equity held between zero and one, the complement is exact within
    // Decimal's complete scale. Addition could round a sum back to one.
    if equity > Decimal::ONE || debt != Decimal::ONE - equity {
        return Err(invalid(
            "WACC requires capital weights summing exactly to one",
        ));
    }
    if [cost_of_equity, cost_of_debt]
        .into_iter()
        .any(|rate| !rate.is_finite() || rate <= -1.0)
    {
        return Err(invalid(
            "WACC component costs must be finite annual effective rates above -100%",
        ));
    }
    if !(0.0..=1.0).contains(&tax_rate) {
        return Err(invalid("WACC requires a finite tax_rate in [0, 1]"));
    }
    let equity = equity
        .to_f64()
        .ok_or_else(|| invalid("WACC equity weight cannot be represented"))?;
    let debt = debt
        .to_f64()
        .ok_or_else(|| invalid("WACC debt weight cannot be represented"))?;
    let rate = equity * cost_of_equity + (debt * (1.0 - tax_rate)) * cost_of_debt;
    if !rate.is_finite() || rate <= -1.0 {
        return Err(invalid(
            "WACC result cannot be represented as a finite effective rate above -100%",
        ));
    }
    Ok(rate)
}
