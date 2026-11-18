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

    /// The at-par mandatory prepayment: its amount and eligible facilities.
    ///
    /// Supplied together with `MandatoryPrepayment` in
    /// `priority_of_payments`: neither is valid without the other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory_prepayment: Option<PrepaymentSpec>,

    /// The at-par voluntary prepayment: its amount and eligible facilities.
    ///
    /// Supplied together with `VoluntaryPrepayment` in
    /// `priority_of_payments`: neither is valid without the other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voluntary_prepayment: Option<PrepaymentSpec>,
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
            mandatory_prepayment: None,
            voluntary_prepayment: None,
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
    ///   `mandatory_prepayment` / `voluntary_prepayment`, and each configured
    ///   prepayment requires its matching priority so every payment consumes
    ///   cash. Each prepayment names its eligible facilities.
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
        for (setting, expression) in self.sizing_expressions() {
            reject_pool_output_dependence(&indexmap::IndexMap::new(), setting, expression)?;
        }
        validate_payment_classes(&self.payment_classes)?;
        for (priority, name, prepayment) in [
            (
                PaymentPriority::MandatoryPrepayment,
                "mandatory_prepayment",
                &self.mandatory_prepayment,
            ),
            (
                PaymentPriority::VoluntaryPrepayment,
                "voluntary_prepayment",
                &self.voluntary_prepayment,
            ),
        ] {
            let has_priority = self.priority_of_payments.contains(&priority);
            match (prepayment, has_priority) {
                (Some(prepayment), true) => prepayment.validate(name)?,
                (None, true) => {
                    return Err(Error::build(format!(
                        "WaterfallSpec: `{priority:?}` in `priority_of_payments` requires \
                         `{name}`."
                    )));
                }
                (Some(_), false) => {
                    return Err(Error::build(format!(
                        "WaterfallSpec: `{name}` requires `{priority:?}` in \
                         `priority_of_payments` so its payment consumes available cash."
                    )));
                }
                (None, false) => {}
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
/// * `setting` - Name of the waterfall setting that holds the expression,
///   for the error.
/// * `expression` - The pool or sizing input: a node id or an inline formula.
///
/// # Errors
///
/// Returns a build error naming the `cs` component and the chain of nodes
/// that reaches it, or a parse error for an invalid formula.
pub(crate) fn reject_pool_output_dependence(
    nodes: &indexmap::IndexMap<crate::types::NodeId, crate::types::NodeSpec>,
    setting: &str,
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
                "WaterfallSpec: `{setting}` is an input measured before debt service, but it \
                 reads `{identifier}`{via} in the same period. Every same-period `cs` \
                 component is an output of the waterfall this input feeds; read a prior \
                 period with `lag(...)` or name an input that does not depend on it."
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

/// The facilities a prepayment may repay.
///
/// Eligibility is elected, never implied: either every eligible facility of
/// the structure (its term loans and revolvers), or the named ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FacilitySelection {
    /// Every term loan and revolver of the capital structure.
    AllEligible,
    /// The listed instrument ids only. Each must be a term loan or revolver
    /// of the structure.
    Named(Vec<String>),
}

impl FacilitySelection {
    /// Whether `instrument_id` may be repaid under this selection.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Exact debt instrument id of the capital structure.
    #[must_use]
    pub fn includes(&self, instrument_id: &str) -> bool {
        match self {
            Self::AllEligible => true,
            Self::Named(ids) => ids.iter().any(|id| id == instrument_id),
        }
    }

    fn validate(&self, owner: &str) -> Result<()> {
        let Self::Named(ids) = self else {
            return Ok(());
        };
        if ids.is_empty() {
            return Err(Error::build(format!(
                "WaterfallSpec: `{owner}.facilities` names no facility; list at least one \
                 instrument or elect `all_eligible`."
            )));
        }
        let mut seen = HashSet::new();
        for id in ids {
            if id.trim().is_empty() || !seen.insert(id.as_str()) {
                return Err(Error::build(format!(
                    "WaterfallSpec: `{owner}.facilities` lists an empty or repeated \
                     instrument id '{id}'."
                )));
            }
        }
        Ok(())
    }
}

/// An at-par mandatory or voluntary prepayment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrepaymentSpec {
    /// Formula or node for the amount requested in the period, in the
    /// waterfall currency. It must be finite and non-negative: a negative
    /// request is an error, not zero. It may not depend on a same-period
    /// `cs` component.
    pub amount: String,
    /// The facilities the prepayment may repay.
    pub facilities: FacilitySelection,
}

impl PrepaymentSpec {
    fn validate(&self, owner: &str) -> Result<()> {
        if self.amount.trim().is_empty() {
            return Err(Error::build(format!(
                "WaterfallSpec: `{owner}.amount` must name a node or formula."
            )));
        }
        self.facilities.validate(owner)
    }
}

/// One elected deduction from, or credit to, the excess-cash-flow measure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EcfTerm {
    /// A model amount: a node or formula in the waterfall currency, signed
    /// as the model states it. It may not depend on a same-period `cs`
    /// component.
    Input {
        /// Node id or formula.
        expression: String,
    },
    /// Cash interest actually paid by the priorities ahead of the sweep in
    /// this period, including carried arrears. PIK and unpaid interest are
    /// not cash and are not included.
    PaidCashInterest,
    /// Fees actually paid by the priorities ahead of the sweep.
    PaidFees,
    /// Scheduled principal actually paid by the priorities ahead of the
    /// sweep, including carried arrears. Other prepayments are not included.
    PaidScheduledPrincipal,
}

/// Excess Cash Flow (ECF) sweep specification.
///
/// The agreement's formula is elected in full: a cash measure, each
/// deduction and each credit. Nothing is deducted automatically.
///
/// ```text
/// ECF     = cash_measure - Σ deductions + Σ credits
/// request = sweep_percentage × ECF   when ECF is positive, otherwise zero
/// ```
///
/// The request is paid at par from the cash the earlier priorities left, up
/// to the eligible principal net of reserves. It is sized, paid and applied
/// to principal in the same reporting period.
///
/// # References
///
/// - Fixed-income and leverage context: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EcfSweepSpec {
    /// Formula or node for the agreement's cash measure before deductions
    /// (for example `ebitda`), in the waterfall currency. Signed and finite.
    /// It may not depend on a same-period `cs` component.
    pub cash_measure: String,

    /// Amounts subtracted from the cash measure, each elected explicitly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deductions: Vec<EcfTerm>,

    /// Amounts added to the cash measure, each elected explicitly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credits: Vec<EcfTerm>,

    /// Fraction of positive ECF to sweep: a decimal in `[0, 1]` (0.5 for 50%).
    pub sweep_percentage: f64,

    /// The facilities the sweep may repay.
    pub facilities: FacilitySelection,
}

impl EcfSweepSpec {
    /// Validate the sweep on its own.
    ///
    /// `sweep_percentage` must be a finite decimal fraction in `[0.0, 1.0]`
    /// (`0.5` sweeps half of excess cash flow), the cash measure and every
    /// input term must name an expression, and a named facility list must be
    /// non-empty and free of repeats. [`WaterfallSpec::validate`] calls this
    /// and additionally requires the `Sweep` priority.
    ///
    /// # Errors
    ///
    /// Returns a build error for a percentage outside `[0.0, 1.0]` or not
    /// finite, an empty expression, or an invalid facility list.
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.sweep_percentage) {
            return Err(Error::build(format!(
                "EcfSweepSpec: `sweep_percentage` must be in [0.0, 1.0], got {}",
                self.sweep_percentage
            )));
        }
        if self
            .expressions()
            .any(|expression| expression.trim().is_empty())
        {
            return Err(Error::build(
                "EcfSweepSpec: the cash measure and every input term must name a node or formula.",
            ));
        }
        self.facilities.validate("ecf_sweep")
    }

    /// The model expressions the sweep reads: the cash measure and every
    /// input deduction and credit.
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.cash_measure.as_str()).chain(
            self.deductions
                .iter()
                .chain(&self.credits)
                .filter_map(|term| match term {
                    EcfTerm::Input { expression } => Some(expression.as_str()),
                    _ => None,
                }),
        )
    }
}

impl WaterfallSpec {
    /// Every prepayment the waterfall configures, with its priority, its
    /// name in the specification and its facility selection.
    pub(crate) fn prepayments(&self) -> Vec<(PaymentPriority, &'static str, &FacilitySelection)> {
        let mut prepayments = Vec::new();
        if let Some(spec) = &self.mandatory_prepayment {
            prepayments.push((
                PaymentPriority::MandatoryPrepayment,
                "mandatory_prepayment",
                &spec.facilities,
            ));
        }
        if let Some(spec) = &self.voluntary_prepayment {
            prepayments.push((
                PaymentPriority::VoluntaryPrepayment,
                "voluntary_prepayment",
                &spec.facilities,
            ));
        }
        if let Some(spec) = &self.ecf_sweep {
            prepayments.push((PaymentPriority::Sweep, "ecf_sweep", &spec.facilities));
        }
        prepayments
    }

    /// Every model expression that sizes a waterfall amount, with the name
    /// of the setting that holds it.
    pub(crate) fn sizing_expressions(&self) -> Vec<(&'static str, &str)> {
        let mut expressions = vec![("available_cash_node", self.available_cash_node.as_str())];
        if let Some(spec) = &self.mandatory_prepayment {
            expressions.push(("mandatory_prepayment.amount", spec.amount.as_str()));
        }
        if let Some(spec) = &self.voluntary_prepayment {
            expressions.push(("voluntary_prepayment.amount", spec.amount.as_str()));
        }
        if let Some(spec) = &self.ecf_sweep {
            expressions.extend(spec.expressions().map(|text| ("ecf_sweep", text)));
        }
        // The election is decided before the period's cash is allocated, so
        // its metric is an input of the stage like the pool and the sizing.
        if let Some(spec) = &self.pik_toggle {
            expressions.push((
                "pik_toggle.liquidity_metric",
                spec.liquidity_metric.as_str(),
            ));
        }
        expressions
    }
}

/// PIK toggle: the modeled exercise of a borrower's PIK election.
///
/// In each forecast period the liquidity metric is compared with the
/// threshold. A metric below the threshold requests PIK and a metric at or
/// above it requests the loan's base coupon terms. The request takes effect
/// at the start of each listed loan's next coupon period after the period
/// close, never on a coupon already running, and is applied through the
/// loan's own coupon terms: the elected share of each covered coupon is zero
/// cash and capitalizes once, on the coupon date. Overdue cash interest and
/// fees stay cash claims and are never capitalized by an election.
///
/// Actual periods are not exercised: record a historical election on the
/// loan itself (`covenants.pik_toggles`).
///
/// The threshold is in the units of `liquidity_metric`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PikToggleSpec {
    /// Node reference or formula for the liquidity metric (for example
    /// `"cash_balance"` or `"ebitda / interest_expense"`). It must evaluate to
    /// a finite number in every forecast period.
    pub liquidity_metric: String,

    /// Finite threshold in the metric's units: a metric below it requests
    /// PIK, a metric at or above it requests the base coupon terms.
    pub threshold: f64,

    /// The loans authorized to elect PIK, one entry each. Must not be empty.
    #[cfg_attr(feature = "json-schema", schemars(length(min = 1)))]
    pub elections: Vec<PikElectionSpec>,
}

/// One loan's PIK authorization under a [`PikToggleSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PikElectionSpec {
    /// Term loan authorized to elect PIK.
    pub instrument_id: String,

    /// Share of each elected coupon that capitalizes, a decimal in `(0, 1]`
    /// (`0.4` capitalizes 40% and pays 60% in cash; `1` is full PIK).
    #[serde(with = "finstack_quant_core::wire::decimal")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DecimalWire")
    )]
    pub pik_fraction: rust_decimal::Decimal,

    /// Minimum number of consecutive coupon periods a PIK election lasts,
    /// counting the first elected period. `0` and `1` let the election end
    /// after one coupon period.
    #[serde(default)]
    pub min_coupon_periods: usize,
}

impl PikToggleSpec {
    /// Validate the toggle on its own.
    ///
    /// [`WaterfallSpec::validate`] calls this for a configured `pik_toggle`.
    ///
    /// # Errors
    ///
    /// Returns a build error when the threshold is not finite, the metric is
    /// empty, no election is listed, a loan is listed twice or with an empty
    /// id, or a `pik_fraction` is outside `(0, 1]`.
    pub fn validate(&self) -> Result<()> {
        if !self.threshold.is_finite() {
            return Err(Error::build(format!(
                "PikToggleSpec: `threshold` must be finite, got {}",
                self.threshold
            )));
        }
        if self.liquidity_metric.trim().is_empty() {
            return Err(Error::build(
                "PikToggleSpec: `liquidity_metric` must name a node or formula",
            ));
        }
        if self.elections.is_empty() {
            return Err(Error::build(
                "PikToggleSpec: `elections` must list each loan authorized to elect PIK; an \
                 election is never implied for every instrument.",
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for election in &self.elections {
            if election.instrument_id.trim().is_empty() {
                return Err(Error::build(
                    "PikToggleSpec: an election has an empty `instrument_id`",
                ));
            }
            if !seen.insert(election.instrument_id.as_str()) {
                return Err(Error::build(format!(
                    "PikToggleSpec: `elections` lists '{}' more than once",
                    election.instrument_id
                )));
            }
            if election.pik_fraction <= rust_decimal::Decimal::ZERO
                || election.pik_fraction > rust_decimal::Decimal::ONE
            {
                return Err(Error::build(format!(
                    "PikToggleSpec: `pik_fraction` for '{}' must be in (0, 1], got {}",
                    election.instrument_id, election.pik_fraction
                )));
            }
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
            cash_measure: "ebitda".into(),
            deductions: vec![
                crate::capital_structure::EcfTerm::PaidCashInterest,
                crate::capital_structure::EcfTerm::PaidFees,
                crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
            ],
            credits: Vec::new(),
            sweep_percentage: percentage,
            facilities: crate::capital_structure::FacilitySelection::AllEligible,
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
            mandatory_prepayment: Some(crate::capital_structure::PrepaymentSpec {
                amount: "mandatory".into(),
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
            }),
            ..spec_with(default_priority_of_payments())
        };
        // A prepayment after Equity means Equity is not last, which the
        // "Equity must be the last entry" rule rejects.
        let err = spec
            .validate()
            .expect_err("prepayment after equity must be rejected");
        assert!(err.to_string().contains("must be the last entry"));
    }

    fn pik(elections: Vec<PikElectionSpec>) -> PikToggleSpec {
        PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 100.0,
            elections,
        }
    }

    fn election(id: &str, fraction: rust_decimal::Decimal) -> PikElectionSpec {
        PikElectionSpec {
            instrument_id: id.into(),
            pik_fraction: fraction,
            min_coupon_periods: 0,
        }
    }

    #[test]
    fn validate_rejects_a_toggle_without_elections() {
        let spec = WaterfallSpec {
            pik_toggle: Some(pik(Vec::new())),
            ..spec_with(default_priority_of_payments())
        };
        let err = spec
            .validate()
            .expect_err("an implied election must be rejected");
        assert!(err.to_string().contains("elections"));
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

        use rust_decimal::Decimal;
        let err = pik(Vec::new())
            .validate()
            .expect_err("no election is rejected");
        assert!(err.to_string().contains("elections"), "{err}");
        pik(vec![election("TL-PIK", Decimal::new(4, 1))])
            .validate()
            .expect("an explicit election is valid");
        // The fraction is in (0, 1], a loan is listed once and the threshold
        // is finite.
        for fraction in [Decimal::ZERO, Decimal::new(-1, 1), Decimal::new(11, 1)] {
            let err = pik(vec![election("TL-PIK", fraction)])
                .validate()
                .expect_err("fraction outside (0, 1]");
            assert!(err.to_string().contains("pik_fraction"), "{err}");
        }
        let err = pik(vec![
            election("TL-PIK", Decimal::ONE),
            election("TL-PIK", Decimal::ONE),
        ])
        .validate()
        .expect_err("a loan listed twice");
        assert!(err.to_string().contains("more than once"), "{err}");
        for threshold in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut spec = pik(vec![election("TL-PIK", Decimal::ONE)]);
            spec.threshold = threshold;
            let err = spec.validate().expect_err("non-finite threshold");
            assert!(err.to_string().contains("threshold"), "{err}");
        }
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
            reject_pool_output_dependence(&nodes, "available_cash_node", pool).expect(pool);
        }
        // Direct, aliased, masked and inline dependence, on any component.
        for (pool, component) in [
            ("direct", "cs.principal_payment.total"),
            ("alias", "cs.interest_expense_cash.total"),
            ("accrual", "cs.accrued_interest.total"),
            ("masked", "cs.debt_balance.total"),
            ("ebitda - cs.fees.total", "cs.fees.total"),
        ] {
            let error = reject_pool_output_dependence(&nodes, "available_cash_node", pool)
                .expect_err(pool)
                .to_string();
            assert!(error.contains(component), "{pool}: {error}");
        }
        // The chain that reaches the output is named.
        let error = reject_pool_output_dependence(&nodes, "available_cash_node", "alias")
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
            .expect_err("MandatoryPrepayment requires mandatory_prepayment");
        assert!(err.to_string().contains("mandatory_prepayment"));
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
    fn pik_elections_are_required_on_the_wire_and_checked_when_validated() {
        for value in [
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0}),
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0,"elections":null}),
            // The retired shape is not read.
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0,"target_instrument_ids":["TL"]}),
            // A threshold the wire cannot state as a finite number.
            serde_json::json!({"liquidity_metric":"cash","threshold":"nan","elections":[]}),
            serde_json::json!({"liquidity_metric":"cash","threshold":null,"elections":[]}),
        ] {
            assert!(serde_json::from_value::<PikToggleSpec>(value).is_err());
        }
        let mut value =
            serde_json::json!({"liquidity_metric":"cash","threshold":1.0,"elections":[]});
        assert!(
            serde_json::from_value::<PikToggleSpec>(value.clone())
                .expect("vector parses")
                .validate()
                .is_err()
        );
        value["elections"] = serde_json::json!([{"instrument_id":"TL","pik_fraction":"0.4"}]);
        let spec: PikToggleSpec = serde_json::from_value(value.clone()).expect("explicit election");
        spec.validate().expect("valid election");
        assert_eq!(spec.elections[0].min_coupon_periods, 0);
        assert_eq!(
            serde_json::to_value(&spec).expect("serialize")["elections"][0]["pik_fraction"],
            serde_json::json!("0.4")
        );
        // An unknown field of an election is rejected.
        value["elections"][0]["min_periods_in_pik"] = serde_json::json!(2);
        assert!(serde_json::from_value::<PikToggleSpec>(value).is_err());
    }
}
