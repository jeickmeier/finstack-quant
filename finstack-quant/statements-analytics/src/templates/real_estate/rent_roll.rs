//! Lease-level rent roll specifications and buildup generator.

use super::buildup::{checked_growth_exponent, sum_expr};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements::{Error, Result};

fn default_occupancy() -> f64 {
    1.0
}

/// Rent step that resets the base rent starting at `start` (inclusive).
///
/// The lease `growth_rate` then applies from this step forward until the next step.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RentStepSpec {
    /// Period (inclusive) when this rent level becomes effective.
    pub start: PeriodId,
    /// Rent per model period starting at `start`.
    pub rent: f64,
}

/// Free rent (concession) window that zeros out rent for `periods` starting at `start`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FreeRentWindowSpec {
    /// Period (inclusive) when free rent starts.
    pub start: PeriodId,
    /// Number of model periods of free rent.
    pub periods: u32,
}

/// Renewal specification for a lease.
///
/// This is modeled in an **expected value** sense via `probability`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RenewalSpec {
    /// Downtime (no rent) after the initial term ends.
    #[serde(default)]
    pub downtime_periods: u32,
    /// Renewal term length in model periods.
    pub term_periods: u32,
    /// Probability of renewal in `0..=1`.
    pub probability: f64,
    /// Rent multiplier applied to the last contractual rent of the initial term.
    ///
    /// Example: `1.05` means renewal starts at +5% vs prior rent level.
    #[serde(default = "default_rent_factor")]
    pub rent_factor: f64,
    /// Free rent periods at renewal start.
    #[serde(default)]
    pub free_rent_periods: u32,
}

fn default_rent_factor() -> f64 {
    1.0
}

impl RenewalSpec {
    /// Validate renewal inputs that are independent of the model period grid.
    ///
    /// # Errors
    ///
    /// Returns an error if probability or rent factor are non-finite, if
    /// probability is outside `[0, 1]`, or if the renewal term/rent factor are
    /// not positive.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the renewal fields are structurally valid.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements_analytics::templates::real_estate::RenewalSpec;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let renewal = RenewalSpec {
    ///     downtime_periods: 1,
    ///     term_periods: 4,
    ///     probability: 0.75,
    ///     rent_factor: 1.05,
    ///     free_rent_periods: 1,
    /// };
    ///
    /// renewal.validate()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.term_periods == 0 {
            return Err(Error::build(
                "RenewalSpec: term_periods must be positive".to_string(),
            ));
        }
        if !self.probability.is_finite() || !(0.0..=1.0).contains(&self.probability) {
            return Err(Error::build(
                "RenewalSpec: probability must be in [0, 1]".to_string(),
            ));
        }
        if !self.rent_factor.is_finite() || self.rent_factor <= 0.0 {
            return Err(Error::build(
                "RenewalSpec: rent_factor must be positive".to_string(),
            ));
        }
        Ok(())
    }
}

/// Convention that determines how `growth_rate` compounds in a lease.
///
/// - `AnnualEscalator` (default): `growth_rate` is applied once per
///   **lease-start anniversary**, measured in model periods (every
///   `slots_per_year()` periods from the segment start; daily and weekly
///   models are rejected). Matches Argus /
///   NCREIF annual bumps: rent is flat within a lease year and steps on the
///   anniversary. The bump counter resets at each rent step.
/// - `PerPeriod`: `growth_rate` is applied every model period. Must be set
///   explicitly; omitted / defaulted values are never per-period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LeaseGrowthConvention {
    /// Growth rate compounds every model period.
    PerPeriod,
    /// Growth rate compounds annually on the lease-start anniversary.
    #[default]
    AnnualEscalator,
}

/// Richer lease spec for rent roll generation:
/// - rent steps (explicit bumps)
/// - arbitrary free-rent windows
/// - optional renewal with downtime + probability
///
/// All amounts are per-model-period (quarterly model => per quarter).
/// Renewal probability is interpreted in expected-value terms, so vacancy and
/// effective-rent outputs include the weighting implicitly.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LeaseSpec {
    /// Base id used to derive node ids:
    /// `{node_id}.pgi`, `{node_id}.free_rent`, `{node_id}.vacancy_loss`, `{node_id}.effective_rent`.
    pub node_id: String,
    /// First period (inclusive) when the lease is active.
    pub start: PeriodId,
    /// Last period (inclusive) of the initial term. `None` means through model end (no renewal modeling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<PeriodId>,
    /// Base rent per period at `start`.
    pub base_rent: f64,
    /// Growth rate applied per period or annually (depending on `growth_convention`)
    /// within a rent segment (between steps).
    #[serde(default)]
    pub growth_rate: f64,
    /// Convention for compounding `growth_rate`.
    ///
    /// Defaults to [`LeaseGrowthConvention::AnnualEscalator`]. Set
    /// [`LeaseGrowthConvention::PerPeriod`] explicitly for per-period
    /// compounding.
    #[serde(default)]
    pub growth_convention: LeaseGrowthConvention,
    /// Rent steps that reset rent levels at their start periods.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rent_steps: Vec<RentStepSpec>,
    /// Number of free rent periods from `start`.
    #[serde(default)]
    pub free_rent_periods: u32,
    /// Additional free rent windows (beyond the initial `free_rent_periods`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub free_rent_windows: Vec<FreeRentWindowSpec>,
    /// Occupancy factor in `0..=1` applied to non-free contractual rent.
    #[serde(default = "default_occupancy")]
    pub occupancy: f64,
    /// Optional renewal modeling after `end`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renewal: Option<RenewalSpec>,
}

impl LeaseSpec {
    /// Validate lease inputs that are independent of the model period grid.
    ///
    /// # Errors
    ///
    /// Returns an error if numeric fields are non-finite, if occupancy is outside
    /// `[0, 1]`, if the node id is empty, or if nested renewal data is invalid.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the lease fields are structurally valid. Period-grid
    /// checks are performed by [`add_rent_roll`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::dates::PeriodId;
    /// use finstack_quant_statements_analytics::templates::real_estate::{
    ///     LeaseGrowthConvention, LeaseSpec,
    /// };
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let lease = LeaseSpec {
    ///     node_id: "tenant_a".to_string(),
    ///     start: PeriodId::quarter(2025, 1).expect("valid period fixture"),
    ///     end: Some(PeriodId::quarter(2025, 4).expect("valid period fixture")),
    ///     base_rent: 25_000.0,
    ///     growth_rate: 0.0,
    ///     growth_convention: LeaseGrowthConvention::PerPeriod,
    ///     rent_steps: vec![],
    ///     free_rent_periods: 0,
    ///     free_rent_windows: vec![],
    ///     occupancy: 1.0,
    ///     renewal: None,
    /// };
    ///
    /// lease.validate()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.node_id.trim().is_empty() {
            return Err(Error::build(
                "LeaseSpec: node_id cannot be empty".to_string(),
            ));
        }
        if !self.base_rent.is_finite() {
            return Err(Error::build(
                "LeaseSpec: base_rent must be finite".to_string(),
            ));
        }
        if !self.growth_rate.is_finite() {
            return Err(Error::build(
                "LeaseSpec: growth_rate must be finite".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.occupancy) {
            return Err(Error::build(
                "LeaseSpec: occupancy must be in [0, 1]".to_string(),
            ));
        }
        for step in &self.rent_steps {
            if !step.rent.is_finite() {
                return Err(Error::build(
                    "LeaseSpec: rent_steps rents must be finite".to_string(),
                ));
            }
        }
        if let Some(renewal) = &self.renewal {
            renewal.validate()?;
        }
        Ok(())
    }
}

/// Standard output node ids for a rent roll.
///
/// The richer rent-roll helper also emits fixed transparency nodes
/// `vacancy_loss_physical` and `renewal_prob_loss` in addition to these
/// configurable aggregate outputs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RentRollOutputNodes {
    /// Total contractual rent (PGI) from all leases.
    pub rent_pgi_node: String,
    /// Total free rent concessions.
    pub free_rent_node: String,
    /// Total vacancy loss (includes occupancy and renewal probability effects).
    pub vacancy_loss_node: String,
    /// Total effective rent (EGI rent component): `rent_pgi - free_rent - vacancy_loss`.
    pub rent_effective_node: String,
}

impl Default for RentRollOutputNodes {
    fn default() -> Self {
        Self {
            rent_pgi_node: "rent_pgi".into(),
            free_rent_node: "free_rent".into(),
            vacancy_loss_node: "vacancy_loss".into(),
            rent_effective_node: "rent_effective".into(),
        }
    }
}

fn find_period_idx(periods: &[finstack_quant_core::dates::Period], id: PeriodId) -> Result<usize> {
    periods
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| Error::build(format!("Unknown period id '{id}'")))
}

fn apply_free_window(is_free: &mut [bool], start_idx: usize, len: u32) -> Result<()> {
    if len == 0 {
        return Ok(());
    }
    let end = start_idx
        .checked_add(len as usize)
        .ok_or_else(|| Error::build("free rent window overflow".to_string()))?;
    for i in start_idx..end.min(is_free.len()) {
        is_free[i] = true;
    }
    Ok(())
}

/// Add a richer rent roll that outputs standard PGI/EGI nodes and per-lease detail nodes.
///
/// Canonical entry point for rent roll generation.
///
/// Creates per-lease value nodes (`{id}.pgi`, `{id}.free_rent`, `{id}.vacancy_loss`,
/// `{id}.effective_rent`) and aggregated totals via `nodes`.
///
/// # Arguments
///
/// * `builder` - Ready builder whose periods define the lease timeline
/// * `leases` - Rich lease specs with optional steps, free-rent windows, and
///   renewals
/// * `nodes` - Aggregate output node ids
///
/// # Errors
///
/// Returns an error if leases reference unknown periods, contain invalid
/// numeric values, or if any generated node conflicts with the builder state.
///
/// # References
///
/// - Property cashflow and fixed-income style discounting context: `docs/REFERENCES.md#hull-options-futures`
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::dates::PeriodId;
/// use finstack_quant_statements::builder::ModelBuilder;
/// use finstack_quant_statements_analytics::templates::real_estate::{
///     add_rent_roll, LeaseGrowthConvention, LeaseSpec, RentRollOutputNodes,
/// };
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut builder = {
///     let mut model_builder = ModelBuilder::new("property");
///     model_builder.periods("2025Q1..Q4", None)?;
///     model_builder
/// };
/// let lease = LeaseSpec {
///     node_id: "tenant_a".to_string(),
///     start: PeriodId::quarter(2025, 1).expect("valid period fixture"),
///     end: Some(PeriodId::quarter(2025, 4).expect("valid period fixture")),
///     base_rent: 25_000.0,
///     growth_rate: 0.0,
///     growth_convention: LeaseGrowthConvention::PerPeriod,
///     rent_steps: vec![],
///     free_rent_periods: 1,
///     free_rent_windows: vec![],
///     occupancy: 1.0,
///     renewal: None,
/// };
///
/// add_rent_roll(&mut builder, &[lease], &RentRollOutputNodes::default())?;
/// let model = builder.build()?;
/// assert!(model.nodes.contains_key("rent_effective"));
/// # Ok(())
/// # }
/// ```
pub fn add_rent_roll(
    builder: &mut ModelBuilder,
    leases: &[LeaseSpec],
    nodes: &RentRollOutputNodes,
) -> Result<()> {
    let mut draft = builder.clone();
    populate_rent_roll(&mut draft, leases, nodes)?;
    *builder = draft;
    Ok(())
}

pub(super) fn populate_rent_roll(
    builder: &mut ModelBuilder,
    leases: &[LeaseSpec],
    nodes: &RentRollOutputNodes,
) -> Result<()> {
    if leases.is_empty() {
        return Err(Error::build("add_rent_roll: expected at least one lease"));
    }

    // Periods in a lease year for the annual escalator: only month-block
    // periods partition a year into a fixed count.
    let slots_per_year = builder
        .periods_slice()
        .first()
        .and_then(|p| p.id.kind().slots_per_year())
        .map(usize::from);

    for lease in leases {
        lease.validate()?;

        let start_idx = find_period_idx(builder.periods_slice(), lease.start)?;
        let end_idx = if let Some(e) = lease.end {
            find_period_idx(builder.periods_slice(), e)?
        } else {
            builder.periods_slice().len().saturating_sub(1)
        };
        if end_idx < start_idx {
            return Err(Error::build("add_rent_roll: end must be >= start"));
        }

        let mut is_free = vec![false; builder.periods_slice().len()];
        apply_free_window(&mut is_free, start_idx, lease.free_rent_periods)?;
        for w in &lease.free_rent_windows {
            if w.periods == 0 {
                continue;
            }
            let w_start = find_period_idx(builder.periods_slice(), w.start)?;
            apply_free_window(&mut is_free, w_start, w.periods)?;
        }

        let (renewal_start_idx, renewal_end_idx, renewal_prob, renewal_free_periods) =
            if let (Some(_end_pid), Some(r)) = (lease.end, lease.renewal.as_ref()) {
                let start = end_idx + 1 + r.downtime_periods as usize;
                let end = start + r.term_periods as usize;
                (
                    Some(start),
                    Some(end.min(builder.periods_slice().len())),
                    r.probability,
                    r.free_rent_periods,
                )
            } else {
                (None, None, 1.0, 0)
            };

        let mut step_points: Vec<(usize, f64)> = Vec::new();
        step_points.push((start_idx, lease.base_rent));
        for s in &lease.rent_steps {
            let idx = find_period_idx(builder.periods_slice(), s.start)?;
            step_points.push((idx, s.rent));
        }
        step_points.sort_by_key(|(i, _)| *i);

        let rent_at = |idx: usize, phase_start: usize, phase_base_rent: f64| -> Result<f64> {
            let mut base_idx = phase_start;
            let mut base_rent = phase_base_rent;
            for (si, sr) in &step_points {
                if *si < phase_start {
                    continue;
                }
                if *si <= idx {
                    base_idx = *si;
                    base_rent = *sr;
                } else {
                    break;
                }
            }
            let periods_elapsed = idx.saturating_sub(base_idx);
            let raw_n = match lease.growth_convention {
                LeaseGrowthConvention::PerPeriod => periods_elapsed,
                LeaseGrowthConvention::AnnualEscalator => {
                    let slots = slots_per_year.ok_or_else(|| {
                        Error::build(
                            "add_rent_roll: an annual escalator needs monthly, quarterly, \
                             semi-annual or annual periods",
                        )
                    })?;
                    periods_elapsed / slots
                }
            };
            let n = checked_growth_exponent(raw_n as u32).map_err(|e| {
                Error::build(format!(
                    "add_rent_roll: {} (period index {}, base_idx {})",
                    e, idx, base_idx
                ))
            })?;
            let contractual = base_rent * (1.0 + lease.growth_rate).powi(n);
            if !contractual.is_finite() {
                return Err(Error::build(
                    "add_rent_roll: rent growth overflow (base_rent * (1+g)^n is not finite)",
                ));
            }
            Ok(contractual)
        };

        // Compute contractual rent at end of initial phase (for renewal base).
        let last_initial_contractual = rent_at(end_idx, start_idx, lease.base_rent)?;
        let renewal_base_rent = lease
            .renewal
            .as_ref()
            .map(|r| last_initial_contractual * r.rent_factor);

        if let Some(r_start) = renewal_start_idx {
            apply_free_window(&mut is_free, r_start, renewal_free_periods)?;
        }

        let mut pgi_vals = Vec::with_capacity(builder.periods_slice().len());
        let mut free_vals = Vec::with_capacity(builder.periods_slice().len());
        let mut vac_vals = Vec::with_capacity(builder.periods_slice().len());
        let mut vac_physical_vals = Vec::with_capacity(builder.periods_slice().len());
        let mut renewal_loss_vals = Vec::with_capacity(builder.periods_slice().len());
        let mut eff_vals = Vec::with_capacity(builder.periods_slice().len());

        for (i, p) in builder.periods_slice().iter().enumerate() {
            let pid = p.id;

            let (contractual, occupancy, renewal_p) = if i >= start_idx && i <= end_idx {
                (
                    rent_at(i, start_idx, lease.base_rent)?,
                    lease.occupancy,
                    1.0,
                )
            } else if let (Some(r_start), Some(r_end), Some(r_base)) =
                (renewal_start_idx, renewal_end_idx, renewal_base_rent)
            {
                if i >= r_start && i < r_end {
                    let contractual = rent_at(i, r_start, r_base)?;
                    (contractual, lease.occupancy, renewal_prob)
                } else {
                    (0.0, 0.0, 1.0)
                }
            } else {
                (0.0, 0.0, 1.0)
            };
            // Attribution convention for renewal free-rent periods: the FULL
            // contractual rent is booked to `free_rent`, not the
            // probability-weighted (`p × rent`) expected concession. The
            // renewal-probability discount is then applied to the remaining
            // (zero) net rent, so no `renewal_loss` is booked in a free
            // period. The decomposition identity
            // `pgi − free_rent − vacancy = effective rent` holds exactly
            // under this convention; only the split between `free_rent` and
            // `renewal_loss` differs from a probability-weighted attribution.
            let is_free_here = contractual != 0.0 && is_free.get(i).copied().unwrap_or(false);
            let free = if is_free_here { contractual } else { 0.0 };
            let net_after_free = contractual - free;

            // Expected-value decomposition:
            // - physical vacancy loss is conditional on renewal happening: net * p * (1 - occ)
            // - renewal probability loss is net * (1 - p)
            // Total loss matches the existing behavior: net - net * (p * occ)
            let eff = net_after_free * occupancy * renewal_p;
            let vac_physical = net_after_free * renewal_p * (1.0 - occupancy);
            let renewal_loss = net_after_free * (1.0 - renewal_p);
            let vac = vac_physical + renewal_loss;

            pgi_vals.push((pid, AmountOrScalar::scalar(contractual)));
            free_vals.push((pid, AmountOrScalar::scalar(free)));
            vac_vals.push((pid, AmountOrScalar::scalar(vac)));
            vac_physical_vals.push((pid, AmountOrScalar::scalar(vac_physical)));
            renewal_loss_vals.push((pid, AmountOrScalar::scalar(renewal_loss)));
            eff_vals.push((pid, AmountOrScalar::scalar(eff)));
        }

        let base = lease.node_id.as_str();
        builder
            .value(format!("{base}.pgi"), &pgi_vals)
            .value(format!("{base}.free_rent"), &free_vals)
            .value(format!("{base}.vacancy_loss"), &vac_vals)
            // Additional transparency nodes (do not change totals behavior):
            .value(format!("{base}.vacancy_loss_physical"), &vac_physical_vals)
            .value(format!("{base}.renewal_prob_loss"), &renewal_loss_vals)
            .value(format!("{base}.effective_rent"), &eff_vals);
    }

    let pgi_nodes = leases
        .iter()
        .map(|l| format!("{}.pgi", l.node_id))
        .collect::<Vec<_>>();
    let free_nodes = leases
        .iter()
        .map(|l| format!("{}.free_rent", l.node_id))
        .collect::<Vec<_>>();
    let vac_nodes = leases
        .iter()
        .map(|l| format!("{}.vacancy_loss", l.node_id))
        .collect::<Vec<_>>();
    let vac_physical_nodes = leases
        .iter()
        .map(|l| format!("{}.vacancy_loss_physical", l.node_id))
        .collect::<Vec<_>>();
    let renewal_loss_nodes = leases
        .iter()
        .map(|l| format!("{}.renewal_prob_loss", l.node_id))
        .collect::<Vec<_>>();

    let pgi_refs = pgi_nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let free_refs = free_nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let vac_refs = vac_nodes.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let vac_physical_refs = vac_physical_nodes
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();
    let renewal_loss_refs = renewal_loss_nodes
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    let pgi_expr = sum_expr(&pgi_refs)?;
    let free_expr = sum_expr(&free_refs)?;
    let vac_expr = sum_expr(&vac_refs)?;
    let vac_physical_expr = sum_expr(&vac_physical_refs)?;
    let renewal_loss_expr = sum_expr(&renewal_loss_refs)?;

    builder
        .compute(nodes.rent_pgi_node.as_str(), &pgi_expr)?
        .compute(nodes.free_rent_node.as_str(), &free_expr)?
        .compute(nodes.vacancy_loss_node.as_str(), &vac_expr)?
        // Extra totals (fixed ids) for more underwriting transparency.
        .compute("vacancy_loss_physical", &vac_physical_expr)?
        .compute("renewal_prob_loss", &renewal_loss_expr)?
        .compute(
            nodes.rent_effective_node.as_str(),
            format!(
                "{} - {} - {}",
                nodes.rent_pgi_node, nodes.free_rent_node, nodes.vacancy_loss_node
            ),
        )?;

    Ok(())
}
