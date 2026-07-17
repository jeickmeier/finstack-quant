//! Waterfall configuration types for dynamic cash flow allocation.
//!
//! These are serializable specifications that define how payments are
//! prioritized and how excess cash flow sweeps and PIK toggles behave.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Waterfall specification for dynamic cash flow allocation.
///
/// Defines the priority of payments and sweep mechanics for capital structure.
///
/// Payment priorities and optional sweep / PIK controls model common leveraged
/// finance behavior where scheduled debt service, excess cash flow sweeps, and
/// equity leakage compete for the same cash pool.
///
/// # Limitations
///
/// - **No intra-category seniority.** Allocation within a payment category
///   (e.g. `Interest`, `Amortization`) is single-class **pro-rata** across all
///   instruments; there is no tranche seniority. A cash shortfall is shared
///   proportionally between a first-lien term loan and a mezzanine note alike.
///   Model strict 1L/2L subordination by running separate waterfalls or by
///   pre-allocating cash upstream.
/// - **Prepayment penalties, call premiums, and original issue discount (OID)
///   are unsupported.** Prepayments (sweep, mandatory, voluntary) are applied
///   at par with no penalty or premium, and no OID accretion is modeled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterfallSpec {
    /// Priority order of payments (default: Fees > Interest > Amortization > Sweep > Equity)
    #[serde(default = "default_priority_of_payments")]
    pub priority_of_payments: Vec<PaymentPriority>,

    /// Optional formula or node reference for cash available to allocate in the waterfall.
    ///
    /// When omitted, the runtime preserves the legacy fully-funded scheduled cashflow behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_cash_node: Option<String>,

    /// Excess Cash Flow (ECF) sweep specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecf_sweep: Option<EcfSweepSpec>,

    /// PIK toggle specification for switching between cash and PIK interest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pik_toggle: Option<PikToggleSpec>,
}

fn default_priority_of_payments() -> Vec<PaymentPriority> {
    vec![
        PaymentPriority::Fees,
        PaymentPriority::Interest,
        PaymentPriority::Amortization,
        PaymentPriority::Sweep,
        PaymentPriority::Equity,
    ]
}

impl Default for WaterfallSpec {
    fn default() -> Self {
        Self {
            priority_of_payments: default_priority_of_payments(),
            available_cash_node: None,
            ecf_sweep: None,
            pik_toggle: None,
        }
    }
}

impl WaterfallSpec {
    /// Validate that the spec represents an economically consistent waterfall.
    ///
    /// Enforces:
    /// - `priority_of_payments` contains no duplicate entries.
    /// - All configured prepayment priorities appear before `Equity`.
    /// - PIK toggles explicitly identify target instruments.
    /// - `ecf_sweep.sweep_percentage` (when configured) lies in `[0.0, 1.0]`.
    /// - When an ECF sweep with a positive `sweep_percentage` is configured,
    ///   at least one prepayment priority (`Sweep`, `MandatoryPrepayment`, or
    ///   `VoluntaryPrepayment`) must be present. Any configured `Equity` entry
    ///   is terminal, so every such prepayment priority necessarily precedes
    ///   equity. Otherwise the waterfall engine silently zeros or never applies
    ///   the configured sweep.
    ///
    /// When `available_cash_node` is set, the fees, interest, and amortization
    /// priorities must all be listed so every cash-consuming category is capped
    /// against the same available-cash source. Equity, if included, must be
    /// terminal because the engine pays it from residual cash after the stack.
    ///
    /// # Errors
    ///
    /// Returns a build error for duplicate priorities, a non-terminal equity
    /// entry, incomplete cash-capping priorities, an empty PIK-toggle target
    /// set, a sweep percentage outside `[0, 1]`, or a positive ECF sweep with
    /// no prepayment priority. Validation does not confirm that referenced
    /// model nodes or instruments exist; that requires the enclosing model and
    /// evaluation context.
    pub fn validate(&self) -> Result<()> {
        for (idx, priority) in self.priority_of_payments.iter().enumerate() {
            if self.priority_of_payments[..idx].contains(priority) {
                return Err(Error::build(format!(
                    "WaterfallSpec: duplicate entry {priority:?} in `priority_of_payments`. \
                     Each payment priority may appear at most once.",
                )));
            }
        }

        // When available-cash capping is active, every cash-consuming category
        // must appear in the stack. A category omitted from
        // `priority_of_payments` is never capped against available cash, so its
        // full planned amount would still be reported as paid while the residual
        // flows to equity — creating cash out of nothing (uses > sources).
        if self.available_cash_node.is_some() {
            for required in [
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
            ] {
                if !self.priority_of_payments.contains(&required) {
                    return Err(Error::build(format!(
                        "WaterfallSpec: `available_cash_node` is set, so `{required:?}` must \
                         appear in `priority_of_payments`; otherwise its planned cash would be \
                         paid in full without consuming available cash, breaking cash \
                         conservation. List it explicitly (it caps to zero when there is no \
                         such flow)."
                    )));
                }
            }
        }

        // Equity, if present, must rank last: the engine distributes the
        // post-stack residual cash to equity after every other category, so a
        // non-terminal `Equity` position would be silently ignored.
        if let Some(equity_pos) = self
            .priority_of_payments
            .iter()
            .position(|p| *p == PaymentPriority::Equity)
        {
            if equity_pos != self.priority_of_payments.len() - 1 {
                return Err(Error::build(
                    "WaterfallSpec: `Equity` must be the last entry in `priority_of_payments`; \
                     the engine always distributes residual cash to equity after every other \
                     category, so a non-terminal position would be silently ignored.",
                ));
            }
        }

        if let Some(pik) = &self.pik_toggle {
            if pik
                .target_instrument_ids
                .as_ref()
                .is_none_or(|targets| targets.is_empty())
            {
                return Err(Error::build(
                    "WaterfallSpec: `pik_toggle.target_instrument_ids` must explicitly list \
                     the instruments that can PIK. Instrument-level PIK capability is not \
                     modeled yet, so implicit all-instrument PIK targets are rejected.",
                ));
            }
        }

        // (Prepayment-after-Equity is already rejected by the "Equity must be
        // last" rule above: if Equity is terminal, no prepayment can follow it.)

        let Some(ecf) = &self.ecf_sweep else {
            return Ok(());
        };
        if !(0.0..=1.0).contains(&ecf.sweep_percentage) {
            return Err(Error::build(format!(
                "WaterfallSpec: `ecf_sweep.sweep_percentage` must be in [0.0, 1.0], got {}",
                ecf.sweep_percentage
            )));
        }
        if ecf.sweep_percentage <= 0.0 {
            return Ok(());
        }
        let has_prepayment_priority = self.priority_of_payments.iter().any(|p| {
            matches!(
                p,
                PaymentPriority::Sweep
                    | PaymentPriority::MandatoryPrepayment
                    | PaymentPriority::VoluntaryPrepayment
            )
        });
        if !has_prepayment_priority {
            return Err(Error::build(
                "WaterfallSpec: `ecf_sweep.sweep_percentage > 0` requires at least one \
                 prepayment priority (`Sweep`, `MandatoryPrepayment`, or \
                 `VoluntaryPrepayment`) in `priority_of_payments`; otherwise the sweep \
                 can never be applied.",
            ));
        }
        Ok(())
    }
}

/// Payment priority levels in the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentPriority {
    /// Fees (commitment fees, facility fees, etc.)
    Fees,
    /// Cash interest payments
    Interest,
    /// Scheduled amortization
    Amortization,
    /// Mandatory prepayments
    MandatoryPrepayment,
    /// Voluntary prepayments
    VoluntaryPrepayment,
    /// Excess cash flow sweep
    Sweep,
    /// Equity distributions
    Equity,
}

/// Excess Cash Flow (ECF) sweep specification.
///
/// Defines how to calculate ECF and what percentage to sweep to pay down debt.
///
/// # ECF Calculation
///
/// The standard ECF formula deducts cash interest from EBITDA. Fees and
/// scheduled principal are also deducted when those payment categories rank
/// ahead of the prepayment priority:
///
/// ```text
/// ECF = EBITDA - Taxes - CapEx - ΔWC - Cash Interest Paid
///       - Fees Paid Ahead of Prepayment
///       - Scheduled Principal Paid Ahead of Prepayment
/// ```
///
/// Set `cash_interest_node` to override the cash-interest input. If omitted,
/// contractual cash interest is deducted automatically using the period's
/// debt-service magnitude.
///
/// # References
///
/// - Fixed-income and leverage context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EcfSweepSpec {
    /// Formula or node reference for EBITDA (e.g., "ebitda" or "revenue - cogs - opex")
    pub ebitda_node: String,

    /// Formula or node reference for taxes (e.g., "taxes")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taxes_node: Option<String>,

    /// Formula or node reference for capital expenditures (e.g., "capex")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capex_node: Option<String>,

    /// Formula or node reference for working capital change (e.g., "wc_change")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_capital_node: Option<String>,

    /// Formula or node reference for cash interest paid (e.g., "cs.interest_expense_cash.total").
    ///
    /// Per S&P LCD / standard LPA definitions, ECF should deduct cash interest paid.
    /// If omitted, contractual cash interest is deducted automatically.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cash_interest_node: Option<String>,

    /// Sweep percentage (e.g., 0.5 for 50%, 0.75 for 75%)
    pub sweep_percentage: f64,

    /// Target instrument ID for sweep payments (if None, applies to all term loans)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_instrument_id: Option<String>,
}

/// PIK toggle specification.
///
/// Defines conditions for switching between cash and PIK interest modes.
///
/// # Hysteresis
///
/// Set `min_periods_in_pik` to prevent oscillation when the liquidity metric
/// hovers near the threshold. Once PIK is triggered, it stays active for at
/// least that many periods before it can switch back.
///
/// Thresholds use the same scalar units as the referenced `liquidity_metric`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PikToggleSpec {
    /// Node reference or formula for liquidity metric (e.g., "cash_balance" or "ebitda / interest_expense")
    pub liquidity_metric: String,

    /// Threshold value: if metric < threshold, enable PIK; otherwise use cash
    pub threshold: f64,

    /// Target instrument IDs (if None, applies to all instruments with PIK capability)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_instrument_ids: Option<Vec<String>>,

    /// Minimum number of periods PIK must stay active once triggered (hysteresis).
    /// Prevents oscillation when the metric hovers near the threshold.
    /// Default: 0 (no hysteresis, PIK can toggle every period).
    #[serde(default)]
    pub min_periods_in_pik: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep_spec(percentage: f64) -> EcfSweepSpec {
        EcfSweepSpec {
            ebitda_node: "ebitda".into(),
            taxes_node: None,
            capex_node: None,
            working_capital_node: None,
            cash_interest_node: None,
            sweep_percentage: percentage,
            target_instrument_id: None,
        }
    }

    #[test]
    fn validate_rejects_duplicate_priorities() {
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Fees,
            ],
            ..WaterfallSpec::default()
        };
        let err = spec.validate().expect_err("duplicates must be rejected");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn validate_rejects_sweep_percentage_outside_unit_interval() {
        for pct in [-0.1, 1.5] {
            let spec = WaterfallSpec {
                ecf_sweep: Some(sweep_spec(pct)),
                ..WaterfallSpec::default()
            };
            let err = spec
                .validate()
                .expect_err("out-of-range sweep_percentage must be rejected");
            assert!(err.to_string().contains("sweep_percentage"));
        }
    }

    #[test]
    fn validate_requires_prepayment_priority_for_positive_sweep() {
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            ecf_sweep: Some(sweep_spec(0.5)),
            ..WaterfallSpec::default()
        };
        let err = spec
            .validate()
            .expect_err("positive sweep without a prepayment priority must be rejected");
        assert!(err.to_string().contains("prepayment priority"));
    }

    #[test]
    fn validate_rejects_prepayment_after_equity() {
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Equity,
                PaymentPriority::MandatoryPrepayment,
            ],
            ..WaterfallSpec::default()
        };
        // A prepayment after Equity means Equity is not last, which the
        // "Equity must be the last entry" rule rejects.
        let err = spec
            .validate()
            .expect_err("prepayment after equity must be rejected");
        assert!(err.to_string().contains("must be the last entry"));
    }

    #[test]
    fn validate_rejects_implicit_pik_targets() {
        let spec = WaterfallSpec {
            pik_toggle: Some(PikToggleSpec {
                liquidity_metric: "liquidity".into(),
                threshold: 100.0,
                target_instrument_ids: None,
                min_periods_in_pik: 0,
            }),
            ..WaterfallSpec::default()
        };
        let err = spec
            .validate()
            .expect_err("implicit PIK targets must be rejected");
        assert!(err.to_string().contains("target_instrument_ids"));
    }

    #[test]
    fn validate_accepts_default_spec_with_sweep() {
        let spec = WaterfallSpec {
            ecf_sweep: Some(sweep_spec(0.5)),
            ..WaterfallSpec::default()
        };
        assert!(spec.validate().is_ok());
    }
}
