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
    /// Priority order of payments (default: Fees > Interest > Amortization > Equity).
    /// Include each prepayment priority explicitly alongside its sizing configuration.
    #[serde(default = "default_priority_of_payments")]
    pub priority_of_payments: Vec<PaymentPriority>,

    /// Formula or node reference for the cash available to allocate.
    ///
    /// This is the cash pool **before debt service**: cash before the fees,
    /// interest, amortization and prepayments this waterfall allocates. Point
    /// it at a standalone cash or free-cash-flow node (`cash`,
    /// `cash_available`, `free_cash_flow`). It may not depend on any
    /// same-period `cs` component, directly or through the formulas and masks
    /// of the nodes it names: every such component is an output of this
    /// waterfall. A prior period read with `lag(...)` is allowed.
    ///
    /// A negative pool is a funding shortfall: it is reported and nothing is
    /// allocated. The cash left after the payment stack is an equity
    /// distribution when `Equity` is the terminal priority and retained cash
    /// when `Equity` is absent.
    pub available_cash_node: String,

    /// Excess Cash Flow (ECF) sweep specification.
    /// This specification and the `Sweep` payment priority must be supplied together.
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
    /// When configured, that priority must be present so payment consumes cash.
    /// Sized independently of the ECF sweep and voluntary prepay buckets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory_prepay_node: Option<String>,

    /// Formula or node for the `VoluntaryPrepayment` rung.
    ///
    /// Required when `VoluntaryPrepayment` appears in `priority_of_payments`.
    /// When configured, that priority must be present so payment consumes cash.
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

/// Default payment stack for scheduled debt service and equity distributions.
pub fn default_priority_of_payments() -> Vec<PaymentPriority> {
    vec![
        PaymentPriority::Fees,
        PaymentPriority::Interest,
        PaymentPriority::Amortization,
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
    /// - `Sweep` and `ecf_sweep` must be supplied together, including when the
    ///   sweep percentage is zero.
    /// - `payment_classes` ids and ranks are unique, each class lists at least
    ///   one instrument, and no instrument appears in more than one class.
    /// - `MandatoryPrepayment` / `VoluntaryPrepayment` require the matching
    ///   `mandatory_prepay_node` / `voluntary_prepay_node`, and configured sizing
    ///   nodes require their matching priority so every payment consumes cash.
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
    /// set, a sweep percentage outside `[0, 1]`, invalid payment classes, or a
    /// prepayment priority and its sizing configuration supplied separately.
    /// The enclosing model and evaluation context check referenced model nodes
    /// and instruments.
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
        reject_pool_output_dependence(&indexmap::IndexMap::new(), &self.available_cash_node)?;
        validate_payment_classes(&self.payment_classes)?;
        for (priority, node_name, node) in [
            (
                PaymentPriority::MandatoryPrepayment,
                "mandatory_prepay_node",
                &self.mandatory_prepay_node,
            ),
            (
                PaymentPriority::VoluntaryPrepayment,
                "voluntary_prepay_node",
                &self.voluntary_prepay_node,
            ),
        ] {
            let has_priority = self.priority_of_payments.contains(&priority);
            if has_priority && node.as_ref().is_none_or(|n| n.trim().is_empty()) {
                return Err(Error::build(format!(
                    "WaterfallSpec: `{priority:?}` in `priority_of_payments` requires \
                     `{node_name}`."
                )));
            }
            if node.is_some() && !has_priority {
                return Err(Error::build(format!(
                    "WaterfallSpec: `{node_name}` requires `{priority:?}` in \
                     `priority_of_payments` so its payment consumes available cash."
                )));
            }
        }

        // Equity, if present, must rank last: the engine distributes the
        // post-stack residual cash to equity after every other category, so a
        // non-terminal `Equity` position would be silently ignored.
        if let Some(equity_pos) = self
            .priority_of_payments
            .iter()
            .position(|p| *p == PaymentPriority::Equity)
            && equity_pos != self.priority_of_payments.len() - 1
        {
            return Err(Error::build(
                "WaterfallSpec: `Equity` must be the last entry in `priority_of_payments`; \
                     the engine always distributes residual cash to equity after every other \
                     category, so a non-terminal position would be silently ignored.",
            ));
        }

        if let Some(pik) = &self.pik_toggle {
            pik.validate()?;
        }

        // (Prepayment-after-Equity is already rejected by the "Equity must be
        // last" rule above: if Equity is terminal, no prepayment can follow it.)

        let has_sweep = self.priority_of_payments.contains(&PaymentPriority::Sweep);
        match (&self.ecf_sweep, has_sweep) {
            (Some(ecf), true) => ecf.validate(),
            (Some(_), false) => Err(Error::build(
                "WaterfallSpec: `ecf_sweep` requires the `Sweep` priority in `priority_of_payments`.",
            )),
            (None, true) => Err(Error::build(
                "WaterfallSpec: `Sweep` in `priority_of_payments` requires `ecf_sweep`.",
            )),
            (None, false) => Ok(()),
        }
    }
}

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

/// Reject a cash pool that depends, in the same period, on an output of the
/// waterfall it funds.
///
/// The pool is cash before debt service. Every same-period capital-structure
/// component (`cs.*`: interest, fees, principal, balances, accruals, arrears)
/// is an output of the allocation, so the pool expression may not read one,
/// directly or through the formulas and masks of the nodes it names. A lagged
/// reference (`lag(cs.debt_balance.total, 1)`) reads a committed prior period
/// and is allowed.
///
/// # Arguments
///
/// * `nodes` - The model's nodes, searched transitively through their
///   formulas and `where` masks. Empty checks the expression alone.
/// * `expression` - The pool: a node id or an inline formula.
///
/// # Errors
///
/// Returns a build error naming the `cs` component and the chain of nodes
/// that reaches it, or a parse error for an invalid formula.
pub(crate) fn reject_pool_output_dependence(
    nodes: &indexmap::IndexMap<crate::types::NodeId, crate::types::NodeSpec>,
    expression: &str,
) -> Result<()> {
    use crate::utils::formula::extract_direct_dependencies;
    let same_period = |text: &str| -> Result<Vec<String>> {
        Ok(
            extract_direct_dependencies(&crate::dsl::parse_formula(text)?)
                .into_iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        )
    };
    let mut pending: Vec<(String, Vec<String>)> = if nodes.contains_key(expression) {
        vec![(expression.to_string(), Vec::new())]
    } else {
        same_period(expression)?
            .into_iter()
            .map(|id| (id, Vec::new()))
            .collect()
    };
    let mut visited = HashSet::new();
    while let Some((identifier, path)) = pending.pop() {
        if identifier.starts_with("cs.") {
            let via = if path.is_empty() {
                String::new()
            } else {
                format!(" through {}", path.join(" -> "))
            };
            return Err(Error::build(format!(
                "WaterfallSpec: `available_cash_node` is the cash pool before debt service, \
                 but it reads `{identifier}`{via} in the same period. Every same-period \
                 `cs` component is an output of the waterfall the pool funds; read a prior \
                 period with `lag(...)` or name a pool that does not depend on it."
            )));
        }
        if !visited.insert(identifier.clone()) {
            continue;
        }
        let Some(node) = nodes.get(identifier.as_str()) else {
            continue;
        };
        let mut chain = path;
        chain.push(identifier);
        for text in [node.formula_text.as_deref(), node.where_text.as_deref()]
            .into_iter()
            .flatten()
        {
            for dependency in same_period(text)? {
                pending.push((dependency, chain.clone()));
            }
        }
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
/// The ECF formula deducts cash interest, fees and scheduled principal paid
/// by payment categories ahead of the `Sweep` priority, including carried
/// arrears. Unpaid claims, PIK and later payment priorities are not deducted:
///
/// ```text
/// ECF = EBITDA - Taxes - CapEx - ΔWC - Cash Interest Paid
///       - Fees Paid Ahead of Prepayment
///       - Scheduled Principal Paid Ahead of Prepayment
///   ```
///
/// Set `cash_interest_node` to override the cash-interest input. If omitted,
/// the interest actually paid ahead of `Sweep` is deducted automatically.
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
    /// If omitted, deducts cash interest actually paid by the `Interest` priority
    /// ahead of `Sweep`, including carried coupon arrears. If `Interest` follows
    /// `Sweep`, this automatic deduction is zero. An explicit node overrides
    /// that amount in the model's cash currency units.
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

    /// Borrowing debt IDs whose cash coupons capitalize when the toggle triggers.
    ///
    /// Must be a non-empty list: instrument-level PIK capability is not
    /// modeled, so an empty list is rejected by [`PikToggleSpec::validate`]
    /// rather than meaning "every instrument".
    /// Swaps and options cannot be PIK targets.
    #[cfg_attr(feature = "json-schema", schemars(length(min = 1)))]
    pub target_instrument_ids: Vec<String>,

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
    /// Returns a build error when `target_instrument_ids` is empty.
    pub fn validate(&self) -> Result<()> {
        if self.target_instrument_ids.is_empty() {
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

    fn sweep_priorities() -> Vec<PaymentPriority> {
        vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Sweep,
            PaymentPriority::Equity,
        ]
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
            let spec = spec_with_sweep(sweep_priorities(), sweep_spec(pct));
            let err = spec
                .validate()
                .expect_err("out-of-range sweep_percentage must be rejected");
            assert!(err.to_string().contains("sweep_percentage"));
        }
    }

    #[test]
    fn validate_requires_sweep_priority_when_ecf_is_configured() {
        for pct in [0.0, 0.5] {
            let spec = spec_with_sweep(default_priority_of_payments(), sweep_spec(pct));
            let err = spec
                .validate()
                .expect_err("configured ECF without the Sweep priority must be rejected");
            assert!(err.to_string().contains("requires the `Sweep`"));
        }
    }

    #[test]
    fn validate_requires_ecf_config_for_sweep_priority() {
        let spec = spec_with(sweep_priorities());
        let err = spec
            .validate()
            .expect_err("the Sweep priority without ECF config must be rejected");
        assert!(err.to_string().contains("requires `ecf_sweep`"));
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
                target_instrument_ids: Vec::new(),
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
            target_instrument_ids: Vec::new(),
            min_periods_in_pik: 0,
        };
        let err = pik.validate().expect_err("empty PIK targets are rejected");
        assert!(err.to_string().contains("target_instrument_ids"), "{err}");
        pik.target_instrument_ids = vec!["TL-PIK".into()];
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
            err.to_string().contains("before debt service"),
            "error must name the pre-debt-service contract: {err}"
        );
    }

    #[test]
    fn the_pool_may_not_reach_a_same_period_cs_output_through_any_chain() {
        use crate::types::{NodeId, NodeSpec, NodeType};
        let node = |id: &str, formula: &str| {
            (
                NodeId::new(id),
                NodeSpec::new(id, NodeType::Calculated).with_formula(formula),
            )
        };
        let mut masked = NodeSpec::new("masked", NodeType::Calculated).with_formula("ebitda");
        masked.where_text = Some("cs.debt_balance.total > 0 * cs.debt_balance.total".into());
        let nodes: indexmap::IndexMap<NodeId, NodeSpec> = [
            node("cash", "ebitda - capex"),
            node("direct", "ebitda - cs.principal_payment.total"),
            node("alias", "net_cash"),
            node("net_cash", "ebitda - interest_paid"),
            node("interest_paid", "cs.interest_expense_cash.total"),
            node("accrual", "ebitda - cs.accrued_interest.total"),
            node("lagged", "ebitda - lag(cs.debt_balance.total, 1)"),
            (NodeId::new("masked"), masked),
        ]
        .into_iter()
        .collect();

        // A pool with no capital dependence, and one that reads a prior period.
        for pool in ["cash", "lagged", "ebitda - capex"] {
            reject_pool_output_dependence(&nodes, pool).expect(pool);
        }
        // Direct, aliased, masked and inline dependence, on any component.
        for (pool, component) in [
            ("direct", "cs.principal_payment.total"),
            ("alias", "cs.interest_expense_cash.total"),
            ("accrual", "cs.accrued_interest.total"),
            ("masked", "cs.debt_balance.total"),
            ("ebitda - cs.fees.total", "cs.fees.total"),
        ] {
            let error = reject_pool_output_dependence(&nodes, pool)
                .expect_err(pool)
                .to_string();
            assert!(error.contains(component), "{pool}: {error}");
        }
        // The chain that reaches the output is named.
        let error = reject_pool_output_dependence(&nodes, "alias")
            .expect_err("alias")
            .to_string();
        assert!(
            error.contains("alias -> net_cash -> interest_paid"),
            "{error}"
        );
    }

    #[test]
    fn validate_accepts_explicit_sweep_with_ecf_config() {
        for pct in [0.0, 0.5] {
            assert!(
                spec_with_sweep(sweep_priorities(), sweep_spec(pct))
                    .validate()
                    .is_ok()
            );
        }
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
    #[test]
    fn pik_targets_are_required_on_the_wire_and_nonempty_when_validated() {
        for value in [
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0}),
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0,"target_instrument_ids":null}),
        ] {
            assert!(serde_json::from_value::<PikToggleSpec>(value).is_err());
        }
        let mut value = serde_json::json!({"liquidity_metric":"cash","threshold":1.0,"target_instrument_ids":[]});
        assert!(
            serde_json::from_value::<PikToggleSpec>(value.clone())
                .expect("vector parses")
                .validate()
                .is_err()
        );
        value["target_instrument_ids"] = serde_json::json!(["TL"]);
        let spec: PikToggleSpec = serde_json::from_value(value.clone()).expect("explicit targets");
        spec.validate().expect("valid targets");
        assert_eq!(
            serde_json::to_value(spec).expect("serialize")["target_instrument_ids"],
            value["target_instrument_ids"]
        );
    }
}
