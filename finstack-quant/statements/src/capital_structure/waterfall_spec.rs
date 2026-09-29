//! Waterfall configuration types for dynamic cash flow allocation.
//!
//! These are serializable specifications that define how payments are
//! prioritized and how excess cash flow sweeps and PIK toggles behave.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
/// - **Payment classes.** When `payment_classes` is empty, allocation within a
///   category is single-class pro-rata (today's behavior). When classes are
///   set, each category walks unique ranks and allocates pro-rata inside a
///   class before the next class sees remaining cash.
/// - **Prepayment penalties, call premiums, and original issue discount (OID)
///   are unsupported.** Prepayments (sweep, mandatory, voluntary) are applied
///   at par with no penalty or premium, and no OID accretion is modeled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WaterfallSpec {
    /// Priority order of payments (default: Fees > Interest > Amortization > Sweep > Equity)
    #[serde(default = "default_priority_of_payments")]
    pub priority_of_payments: Vec<PaymentPriority>,

    /// Formula or node reference for cash available to allocate in the waterfall.
    ///
    /// This is the **pre-waterfall** cash pool: cash before fees, interest,
    /// amortization, and prepays allocated by this waterfall. Point it at a
    /// standalone cash / FCF node (`cash`, `cash_available`, `free_cash_flow`).
    /// Do not deduct `cs.interest_expense`, `cs.interest_expense_cash`,
    /// `cs.principal_payment`, or `cs.fees` here — those are allocated by the
    /// waterfall, and subtracting them from the pool double-pays debt service.
    ///
    /// Required. Without a cash pool the waterfall reports every scheduled fee,
    /// coupon and amortization as paid in full regardless of whether the model
    /// generated the cash — uses exceed sources and no shortfall can ever be
    /// raised, so the structure cannot report insolvency.
    pub available_cash_node: String,

    /// Excess Cash Flow (ECF) sweep specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecf_sweep: Option<EcfSweepSpec>,

    /// PIK toggle specification for switching between cash and PIK interest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pik_toggle: Option<PikToggleSpec>,

    /// Payment classes for intra-category seniority (e.g. 1L then 2L).
    ///
    /// Empty means one implicit class: today's single-class pro-rata. When
    /// non-empty, every contractual instrument must appear in exactly one
    /// class, ranks and ids must be unique, and allocation walks rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payment_classes: Vec<PaymentClassSpec>,

    /// Formula or node for the `MandatoryPrepayment` rung.
    ///
    /// Required when `MandatoryPrepayment` appears in `priority_of_payments`.
    /// Sized independently of the ECF sweep and voluntary prepay buckets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory_prepay_node: Option<String>,

    /// Formula or node for the `VoluntaryPrepayment` rung.
    ///
    /// Required when `VoluntaryPrepayment` appears in `priority_of_payments`.
    /// Sized independently of the ECF sweep and mandatory prepay buckets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voluntary_prepay_node: Option<String>,
}

/// A seniority class for intra-category waterfall allocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PaymentClassSpec {
    /// Class identifier (e.g. `"1L"`).
    pub id: String,
    /// Seniority rank; `0` is most senior. Ranks must be unique.
    pub rank: u32,
    /// Instrument ids that belong to this class. Each instrument may appear
    /// in at most one class.
    pub instrument_ids: Vec<String>,
}

impl Default for WaterfallSpec {
    fn default() -> Self {
        Self {
            priority_of_payments: default_priority_of_payments(),
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            payment_classes: Vec::new(),
            mandatory_prepay_node: None,
            voluntary_prepay_node: None,
        }
    }
}

/// Canonical payment stack: fees, interest, amortization, sweep, equity.
pub fn default_priority_of_payments() -> Vec<PaymentPriority> {
    vec![
        PaymentPriority::Fees,
        PaymentPriority::Interest,
        PaymentPriority::Amortization,
        PaymentPriority::Sweep,
        PaymentPriority::Equity,
    ]
}

impl WaterfallSpec {
    /// Validate that the spec represents an economically consistent waterfall.
    ///
    /// Enforces:
    /// - `priority_of_payments` contains no duplicate entries.
    /// - All configured prepayment priorities appear before `Equity`.
    /// - PIK toggles explicitly identify target instruments
    ///   ([`PikToggleSpec::validate`]).
    /// - `ecf_sweep.sweep_percentage` (when configured) lies in `[0.0, 1.0]`
    ///   ([`EcfSweepSpec::validate`]).
    /// - When an ECF sweep with a positive `sweep_percentage` is configured,
    ///   at least one prepayment priority (`Sweep`, `MandatoryPrepayment`, or
    ///   `VoluntaryPrepayment`) must be present. Any configured `Equity` entry
    ///   is terminal, so every such prepayment priority necessarily precedes
    ///   equity. Otherwise the waterfall engine silently zeros or never applies
    ///   the configured sweep.
    /// - `payment_classes` ids and ranks are unique, each class lists at least
    ///   one instrument, and no instrument appears in more than one class.
    /// - `MandatoryPrepayment` / `VoluntaryPrepayment` require the matching
    ///   `mandatory_prepay_node` / `voluntary_prepay_node`.
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
    /// set, a sweep percentage outside `[0, 1]`, a positive ECF sweep with
    /// no prepayment priority, invalid payment classes, or a prepayment rung
    /// without its sizing node. Validation does not confirm that referenced
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

        // Every cash-consuming category must appear in the stack. A category
        // omitted from `priority_of_payments` is never capped against available
        // cash, so its full planned amount would still be reported as paid
        // while the residual flows to equity — creating cash out of nothing
        // (uses > sources).
        for required in [
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
        ] {
            if !self.priority_of_payments.contains(&required) {
                return Err(Error::build(format!(
                    "WaterfallSpec: `{required:?}` must appear in `priority_of_payments`; \
                     otherwise its planned cash would be paid in full without consuming \
                     available cash, breaking cash conservation. List it explicitly (it \
                     caps to zero when there is no such flow)."
                )));
            }
        }

        if self.available_cash_node.trim().is_empty() {
            return Err(Error::build(
                "WaterfallSpec: `available_cash_node` must name a value or formula node \
                 supplying the period's available cash.",
            ));
        }
        reject_available_cash_debt_service(&self.available_cash_node)?;
        validate_payment_classes(&self.payment_classes)?;
        if self
            .priority_of_payments
            .contains(&PaymentPriority::MandatoryPrepayment)
            && self
                .mandatory_prepay_node
                .as_ref()
                .is_none_or(|n| n.trim().is_empty())
        {
            return Err(Error::build(
                "WaterfallSpec: `MandatoryPrepayment` in `priority_of_payments` requires \
                 `mandatory_prepay_node`.",
            ));
        }
        if self
            .priority_of_payments
            .contains(&PaymentPriority::VoluntaryPrepayment)
            && self
                .voluntary_prepay_node
                .as_ref()
                .is_none_or(|n| n.trim().is_empty())
        {
            return Err(Error::build(
                "WaterfallSpec: `VoluntaryPrepayment` in `priority_of_payments` requires \
                 `voluntary_prepay_node`.",
            ));
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
            pik.validate()?;
        }

        // (Prepayment-after-Equity is already rejected by the "Equity must be
        // last" rule above: if Equity is terminal, no prepayment can follow it.)

        let Some(ecf) = &self.ecf_sweep else {
            return Ok(());
        };
        ecf.validate()?;
        // A positive sweep also needs somewhere to land, which only the
        // enclosing waterfall's priority stack can tell.
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

/// Tokens that mean the cash pool has already deducted waterfall debt service.
const AVAILABLE_CASH_DEBT_SERVICE_TOKENS: &[&str] = &[
    "cs.interest_expense",
    "cs.interest_expense_cash",
    "cs.principal_payment",
    "cs.fees",
];

/// Reject an `available_cash_node` expression (or a named node's formula) that
/// deducts capital-structure debt service from the pre-waterfall cash pool.
///
/// # Arguments
///
/// * `text` - Inline DSL formula or node `formula_text` to scan for
///   `cs.interest_expense`, `cs.interest_expense_cash`, `cs.principal_payment`,
///   or `cs.fees`. Those buckets are allocated by the waterfall; subtracting
///   them here double-pays debt service.
///
/// # Errors
///
/// Returns a build error naming the pre-waterfall contract when `text`
/// contains any of those `cs.*` debt-service identifiers.
fn validate_payment_classes(classes: &[PaymentClassSpec]) -> Result<()> {
    if classes.is_empty() {
        return Ok(());
    }
    let mut ids = HashSet::new();
    let mut ranks = HashSet::new();
    let mut instruments = HashSet::new();
    for class in classes {
        if class.id.trim().is_empty() {
            return Err(Error::build(
                "WaterfallSpec: `payment_classes` entries must have a non-empty `id`.",
            ));
        }
        if !ids.insert(class.id.as_str()) {
            return Err(Error::build(format!(
                "WaterfallSpec: duplicate payment class id '{}'.",
                class.id
            )));
        }
        if !ranks.insert(class.rank) {
            return Err(Error::build(format!(
                "WaterfallSpec: duplicate payment class rank {}.",
                class.rank
            )));
        }
        if class.instrument_ids.is_empty() {
            return Err(Error::build(format!(
                "WaterfallSpec: payment class '{}' must list at least one instrument.",
                class.id
            )));
        }
        for instrument_id in &class.instrument_ids {
            if !instruments.insert(instrument_id.as_str()) {
                return Err(Error::build(format!(
                    "WaterfallSpec: instrument '{instrument_id}' appears in more than one \
                     payment class."
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn reject_available_cash_debt_service(text: &str) -> Result<()> {
    if let Some(token) = AVAILABLE_CASH_DEBT_SERVICE_TOKENS
        .iter()
        .copied()
        .find(|token| text.contains(token))
    {
        return Err(Error::build(format!(
            "WaterfallSpec: `available_cash_node` is the pre-waterfall cash pool \
             (cash before fees, interest, amortization, and prepays allocated by \
             this waterfall). The formula must not deduct `{token}` or other \
             `cs.interest_expense` / `cs.interest_expense_cash` / \
             `cs.principal_payment` / `cs.fees` terms; those are allocated by \
             the waterfall and deducting them here double-pays debt service."
        )));
    }
    Ok(())
}

/// Payment priority levels in the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
///   ```
///
/// Set `cash_interest_node` to override the cash-interest input. If omitted,
/// contractual cash interest is deducted automatically using the period's
/// debt-service magnitude.
///
/// # References
///
/// - Fixed-income and leverage context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

impl EcfSweepSpec {
    /// Validate the sweep on its own.
    ///
    /// `sweep_percentage` must be a finite decimal fraction in `[0.0, 1.0]`
    /// (`0.5` sweeps half of excess cash flow). [`WaterfallSpec::validate`]
    /// calls this and additionally requires a prepayment priority for a
    /// positive sweep, which needs the enclosing priority stack.
    ///
    /// # Errors
    ///
    /// Returns a build error when `sweep_percentage` is outside `[0.0, 1.0]`
    /// or not finite.
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.sweep_percentage) {
            return Err(Error::build(format!(
                "EcfSweepSpec: `sweep_percentage` must be in [0.0, 1.0], got {}",
                self.sweep_percentage
            )));
        }
        Ok(())
    }
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PikToggleSpec {
    /// Node reference or formula for liquidity metric (e.g., "cash_balance" or "ebitda / interest_expense")
    pub liquidity_metric: String,

    /// Threshold value: if metric < threshold, enable PIK; otherwise use cash
    pub threshold: f64,

    /// Instruments that switch to PIK when the toggle triggers.
    ///
    /// Must be a non-empty list: instrument-level PIK capability is not
    /// modeled, so `None` or an empty list is rejected by
    /// [`PikToggleSpec::validate`] rather than meaning "every instrument".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_instrument_ids: Option<Vec<String>>,

    /// Minimum number of periods PIK must stay active once triggered (hysteresis).
    /// Prevents oscillation when the metric hovers near the threshold.
    /// Default: 0 (no hysteresis, PIK can toggle every period).
    #[serde(default)]
    pub min_periods_in_pik: usize,
}

impl PikToggleSpec {
    /// Validate the toggle on its own.
    ///
    /// `target_instrument_ids` must list at least one instrument explicitly.
    /// [`WaterfallSpec::validate`] calls this for a configured `pik_toggle`.
    ///
    /// # Errors
    ///
    /// Returns a build error when `target_instrument_ids` is `None` or empty.
    pub fn validate(&self) -> Result<()> {
        if self
            .target_instrument_ids
            .as_ref()
            .is_none_or(|targets| targets.is_empty())
        {
            return Err(Error::build(
                "PikToggleSpec: `target_instrument_ids` must explicitly list the \
                 instruments that can PIK. Instrument-level PIK capability is not \
                 modeled yet, so implicit all-instrument PIK targets are rejected.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    /// Minimal valid spec for tests: a priority stack and a cash node.
    fn spec_with(priority: Vec<PaymentPriority>) -> WaterfallSpec {
        WaterfallSpec {
            priority_of_payments: priority,
            available_cash_node: "cash".into(),
            ecf_sweep: None,
            pik_toggle: None,
            ..WaterfallSpec::default()
        }
    }

    /// Minimal valid spec carrying an ECF sweep.
    fn spec_with_sweep(priority: Vec<PaymentPriority>, sweep: EcfSweepSpec) -> WaterfallSpec {
        WaterfallSpec {
            ecf_sweep: Some(sweep),
            ..spec_with(priority)
        }
    }
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
        let spec = spec_with(vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Fees,
        ]);
        let err = spec.validate().expect_err("duplicates must be rejected");
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn validate_rejects_sweep_percentage_outside_unit_interval() {
        for pct in [-0.1, 1.5] {
            let spec = spec_with_sweep(default_priority_of_payments(), sweep_spec(pct));
            let err = spec
                .validate()
                .expect_err("out-of-range sweep_percentage must be rejected");
            assert!(err.to_string().contains("sweep_percentage"));
        }
    }

    #[test]
    fn validate_requires_prepayment_priority_for_positive_sweep() {
        let spec = spec_with_sweep(
            vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Equity,
            ],
            sweep_spec(0.5),
        );
        let err = spec
            .validate()
            .expect_err("positive sweep without a prepayment priority must be rejected");
        assert!(err.to_string().contains("prepayment priority"));
    }

    #[test]
    fn validate_rejects_prepayment_after_equity() {
        let spec = WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Amortization,
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Equity,
                PaymentPriority::MandatoryPrepayment,
            ],
            mandatory_prepay_node: Some("mandatory".into()),
            ..spec_with(default_priority_of_payments())
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
            ..spec_with(default_priority_of_payments())
        };
        let err = spec
            .validate()
            .expect_err("implicit PIK targets must be rejected");
        assert!(err.to_string().contains("target_instrument_ids"));
    }

    /// The sweep and PIK rules run on the sub-specs alone, so a host can
    /// validate an `EcfSweepSpec` or `PikToggleSpec` without a waterfall.
    #[test]
    fn sub_specs_validate_standalone() {
        for pct in [-0.1, 1.5, f64::NAN] {
            let err = sweep_spec(pct)
                .validate()
                .expect_err("out-of-range sweep_percentage must be rejected");
            assert!(err.to_string().contains("sweep_percentage"), "{err}");
        }
        sweep_spec(0.0).validate().expect("zero sweep is valid");
        sweep_spec(1.0).validate().expect("full sweep is valid");

        let mut pik = PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 100.0,
            target_instrument_ids: Some(Vec::new()),
            min_periods_in_pik: 0,
        };
        let err = pik.validate().expect_err("empty PIK targets are rejected");
        assert!(err.to_string().contains("target_instrument_ids"), "{err}");
        pik.target_instrument_ids = Some(vec!["TL-PIK".into()]);
        pik.validate().expect("explicit PIK targets are valid");
    }

    #[test]
    fn validate_rejects_available_cash_that_deducts_cs_debt_service() {
        let spec = WaterfallSpec {
            available_cash_node: "ebitda - cs.interest_expense_cash.total".into(),
            ..spec_with(default_priority_of_payments())
        };
        let err = spec
            .validate()
            .expect_err("deducting cs debt service from available cash must be rejected");
        assert!(
            err.to_string().contains("pre-waterfall"),
            "error must name the pre-waterfall contract: {err}"
        );
    }

    #[test]
    fn reject_available_cash_debt_service_scans_named_node_formula() {
        reject_available_cash_debt_service("cash_available")
            .expect("a standalone cash node name is the pre-waterfall pool");
        let err = reject_available_cash_debt_service("ebitda - cs.principal_payment.total")
            .expect_err("a named node's formula that deducts cs principal must be rejected");
        assert!(err.to_string().contains("pre-waterfall"));
    }

    #[test]
    fn validate_accepts_default_spec_with_sweep() {
        let spec = WaterfallSpec {
            ecf_sweep: Some(sweep_spec(0.5)),
            ..spec_with(default_priority_of_payments())
        };
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validate_rejects_mandatory_prepayment_without_node() {
        let spec = spec_with(vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::MandatoryPrepayment,
            PaymentPriority::Equity,
        ]);
        let err = spec
            .validate()
            .expect_err("MandatoryPrepayment requires mandatory_prepay_node");
        assert!(err.to_string().contains("mandatory_prepay_node"));
    }

    #[test]
    fn validate_rejects_duplicate_payment_class_id() {
        let spec = WaterfallSpec {
            payment_classes: vec![
                PaymentClassSpec {
                    id: "1L".into(),
                    rank: 0,
                    instrument_ids: vec!["A".into()],
                },
                PaymentClassSpec {
                    id: "1L".into(),
                    rank: 1,
                    instrument_ids: vec!["B".into()],
                },
            ],
            ..spec_with(default_priority_of_payments())
        };
        let err = spec
            .validate()
            .expect_err("duplicate class ids must be rejected");
        assert!(err.to_string().contains("duplicate payment class id"));
    }
}
