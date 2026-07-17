//! Brinson-Fachler three-way performance attribution with Carino linking.
//!
//! # Single-period decomposition
//!
//! Given per-sector portfolio and benchmark weights and returns, the
//! classical Brinson-Fachler (Brinson, Hood & Beebower 1986; Brinson &
//! Fachler 1985) decomposition splits the portfolio's active return into
//! three components per sector `i`:
//!
//! ```text
//! Allocation_i  = (w_p,i − w_b,i) · (r_b,i − r_b)      // sector weighting bet
//! Selection_i   = w_b,i · (r_p,i − r_b,i)              // within-sector picks
//! Interaction_i = (w_p,i − w_b,i) · (r_p,i − r_b,i)    // joint effect
//! ```
//!
//! where:
//!
//! * `w_p,i`, `w_b,i` — portfolio / benchmark weight in sector `i`
//! * `r_p,i`, `r_b,i` — portfolio / benchmark return in sector `i`
//! * `r_b = Σ_i w_b,i · r_b,i` — benchmark total return
//!
//! Summed across sectors, the three effects reconstruct the active return
//! `r_p − r_b` exactly: `Σ_i (Allocation_i + Selection_i + Interaction_i)
//! = r_p − r_b`.
//!
//! # Multi-period Carino linking
//!
//! Arithmetic effects do *not* compound — a +1 % allocation effect in
//! period `t₁` and a +1 % allocation effect in period `t₂` do not
//! produce exactly a +2.01 % linked effect because portfolio and
//! benchmark compound at different rates. Carino (1999) gives a
//! smoothing coefficient
//!
//! ```text
//! k_t = ln(1 + r_{p,t}) − ln(1 + r_{b,t})
//!       ─────────────────────────────────────   if r_{p,t} ≠ r_{b,t}
//!              r_{p,t} − r_{b,t}
//!
//! k_t = 1 / (1 + r_{p,t})                        if r_{p,t} = r_{b,t}
//!
//! K   = (ln(1 + R_p) − ln(1 + R_b)) / (R_p − R_b)  if R_p ≠ R_b
//!       else 1 / (1 + R_p)
//! ```
//!
//! where `R_p = ∏_t (1 + r_{p,t}) − 1` is the geometrically compounded
//! portfolio return. The Carino-linked effect per sector is
//!
//! ```text
//! linked_effect_i = Σ_t (k_t / K) · period_effect_{t,i}
//! ```
//!
//! After summing sectors, `linked_allocation + linked_selection +
//! linked_interaction = R_p − R_b` exactly — the arithmetic effects are
//! rescaled so they reconstruct the *geometric* active return.
//!
//! # References
//!
//! * Brinson, G. P., & Fachler, N. (1985). "Measuring Non-US Equity
//!   Portfolio Performance." *Journal of Portfolio Management*, 11(3).
//! * Brinson, G. P., Hood, L. R., & Beebower, G. L. (1986). "Determinants
//!   of Portfolio Performance." *Financial Analysts Journal*, 42(4).
//! * Carino, D. (1999). "Combining Attribution Effects over Time."
//!   *Journal of Performance Measurement*, 3(4), 5–14.
//! * Grinold, R. C., & Kahn, R. N. (2000). *Active Portfolio Management*
//!   (2nd ed.), Chapter 17.
//!
//! Factor-based attribution already exists in [`crate::attribution`];
//! this module adds the classical Brinson-Fachler decomposition that
//! benchmark-relative reporting requires.

use crate::error::{Error, Result};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use serde::{Deserialize, Serialize};

/// Per-sector portfolio and benchmark weights and returns for a single
/// attribution period.
///
/// Weights should be expressed as fractions summing to 1.0 within each
/// of portfolio and benchmark. Returns are the period arithmetic returns
/// for each sector (e.g. `0.015` = +1.5 %).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorPeriod {
    /// Sector identifier (industry / country / any grouping).
    pub sector: String,
    /// Portfolio weight in the sector at period start.
    pub portfolio_weight: f64,
    /// Benchmark weight in the sector at period start.
    pub benchmark_weight: f64,
    /// Portfolio return for the sector over the period.
    pub portfolio_return: f64,
    /// Benchmark return for the sector over the period.
    pub benchmark_return: f64,
}

/// Per-sector attribution effects (Brinson-Fachler three-way split).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectorEffect {
    /// Sector identifier (mirrors [`SectorPeriod::sector`]).
    pub sector: String,
    /// Allocation effect `(w_p − w_b) · (r_b,i − r_b)`.
    pub allocation: f64,
    /// Selection effect `w_b · (r_p,i − r_b,i)`.
    pub selection: f64,
    /// Interaction effect `(w_p − w_b) · (r_p,i − r_b,i)`.
    pub interaction: f64,
    /// Sum of the three effects — equal to the sector's contribution to
    /// active return.
    pub total: f64,
}

/// Single-period Brinson-Fachler result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrinsonPeriodResult {
    /// Per-sector effects, in the order supplied.
    pub sectors: Vec<SectorEffect>,
    /// Sum of allocation effects across sectors.
    pub total_allocation: f64,
    /// Sum of selection effects across sectors.
    pub total_selection: f64,
    /// Sum of interaction effects across sectors.
    pub total_interaction: f64,
    /// Portfolio total return for the period, `Σ_i w_p,i · r_p,i`.
    pub portfolio_return: f64,
    /// Benchmark total return for the period, `Σ_i w_b,i · r_b,i`.
    pub benchmark_return: f64,
    /// Active return, `portfolio_return − benchmark_return`.
    /// Equals `total_allocation + total_selection + total_interaction`.
    pub total_excess_return: f64,
}

/// Compute a single-period Brinson-Fachler attribution.
///
/// Portfolio and benchmark weights within each respective universe must
/// sum to 1.0 (within `1e-6` tolerance). A sector missing from either side
/// should still be supplied with a zero weight on that side so the
/// decomposition stays complete.
///
/// # Errors
///
/// * `Error::InvalidInput` if weights don't sum to 1.0 or any return is
///   non-finite.
///
/// # Arguments
///
/// * `sectors` - Sector-level portfolio and benchmark weights and returns;
///   weights on each side must sum to one, including zero-weight sectors.
pub fn brinson_fachler(sectors: &[SectorPeriod]) -> Result<BrinsonPeriodResult> {
    const WEIGHT_TOLERANCE: f64 = 1e-6;

    if sectors.is_empty() {
        return Err(Error::invalid_input(
            "Brinson-Fachler attribution requires at least one sector",
        ));
    }

    let mut sum_wp = NeumaierAccumulator::new();
    let mut sum_wb = NeumaierAccumulator::new();
    let mut sum_rp = NeumaierAccumulator::new();
    let mut sum_rb = NeumaierAccumulator::new();
    for s in sectors {
        for (name, value) in [
            ("portfolio_weight", s.portfolio_weight),
            ("benchmark_weight", s.benchmark_weight),
            ("portfolio_return", s.portfolio_return),
            ("benchmark_return", s.benchmark_return),
        ] {
            if !value.is_finite() {
                return Err(Error::invalid_input(format!(
                    "Brinson input '{name}' for sector '{}' must be finite (got {value})",
                    s.sector
                )));
            }
        }
        sum_wp.add(s.portfolio_weight);
        sum_wb.add(s.benchmark_weight);
        sum_rp.add(s.portfolio_weight * s.portfolio_return);
        sum_rb.add(s.benchmark_weight * s.benchmark_return);
    }

    let total_wp = sum_wp.total();
    let total_wb = sum_wb.total();
    if (total_wp - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "Portfolio weights must sum to 1.0 (got {total_wp})"
        )));
    }
    if (total_wb - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "Benchmark weights must sum to 1.0 (got {total_wb})"
        )));
    }

    let portfolio_return = sum_rp.total();
    let benchmark_return = sum_rb.total();

    let mut allocation = NeumaierAccumulator::new();
    let mut selection = NeumaierAccumulator::new();
    let mut interaction = NeumaierAccumulator::new();
    let mut sector_effects: Vec<SectorEffect> = Vec::with_capacity(sectors.len());

    for s in sectors {
        let alloc =
            (s.portfolio_weight - s.benchmark_weight) * (s.benchmark_return - benchmark_return);
        let sel = s.benchmark_weight * (s.portfolio_return - s.benchmark_return);
        let inter =
            (s.portfolio_weight - s.benchmark_weight) * (s.portfolio_return - s.benchmark_return);

        allocation.add(alloc);
        selection.add(sel);
        interaction.add(inter);

        sector_effects.push(SectorEffect {
            sector: s.sector.clone(),
            allocation: alloc,
            selection: sel,
            interaction: inter,
            total: alloc + sel + inter,
        });
    }

    let total_allocation = allocation.total();
    let total_selection = selection.total();
    let total_interaction = interaction.total();

    Ok(BrinsonPeriodResult {
        sectors: sector_effects,
        total_allocation,
        total_selection,
        total_interaction,
        portfolio_return,
        benchmark_return,
        total_excess_return: portfolio_return - benchmark_return,
    })
}

/// Compounded portfolio vs. benchmark return and Carino-linked effects
/// across multiple periods.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CarinoLinkedAttribution {
    /// Per-period decompositions in chronological order.
    pub periods: Vec<BrinsonPeriodResult>,
    /// Geometrically compounded portfolio return,
    /// `∏_t (1 + r_p,t) − 1`.
    pub portfolio_return_compounded: f64,
    /// Geometrically compounded benchmark return.
    pub benchmark_return_compounded: f64,
    /// Per-sector Carino-smoothed effects summed across periods.
    /// `sum(linked_allocation + linked_selection + linked_interaction)`
    /// reconstructs the active compounded return exactly.
    pub linked_sectors: Vec<SectorEffect>,
    /// Sum of per-sector linked allocation effects.
    pub linked_allocation: f64,
    /// Sum of per-sector linked selection effects.
    pub linked_selection: f64,
    /// Sum of per-sector linked interaction effects.
    pub linked_interaction: f64,
}

/// Apply Carino smoothing to a sequence of per-period Brinson-Fachler
/// decompositions so the arithmetic effects reconstruct the *geometrically
/// compounded* active return exactly.
///
/// # Errors
///
/// * `Error::InvalidInput` if `periods` is empty, has inconsistent sector
///   ordering across periods, or any per-period return is non-finite.
///
/// # Arguments
///
/// * `periods` - Chronologically ordered Brinson-Fachler period results with
///   identical sector ordering for every observation.
pub fn carino_link(periods: &[BrinsonPeriodResult]) -> Result<CarinoLinkedAttribution> {
    if periods.is_empty() {
        return Err(Error::invalid_input(
            "Carino linking requires at least one period",
        ));
    }

    // Enforce consistent sector ordering so summing per-sector across
    // periods is well-defined.
    let sector_names: Vec<String> = periods[0]
        .sectors
        .iter()
        .map(|e| e.sector.clone())
        .collect();
    for (idx, p) in periods.iter().enumerate().skip(1) {
        if p.sectors.len() != sector_names.len()
            || !p
                .sectors
                .iter()
                .zip(sector_names.iter())
                .all(|(e, n)| e.sector == *n)
        {
            return Err(Error::invalid_input(format!(
                "Carino linking requires identical sector ordering across all periods (period {idx} differs from period 0)"
            )));
        }
    }

    // Compounded portfolio/benchmark returns.
    let mut compounded_p = 1.0_f64;
    let mut compounded_b = 1.0_f64;
    for p in periods {
        if !p.portfolio_return.is_finite() || !p.benchmark_return.is_finite() {
            return Err(Error::invalid_input(format!(
                "Carino linking requires finite period returns, got portfolio_return = {}, benchmark_return = {}",
                p.portfolio_return, p.benchmark_return
            )));
        }
        compounded_p *= 1.0 + p.portfolio_return;
        compounded_b *= 1.0 + p.benchmark_return;
    }
    let r_p_total = compounded_p - 1.0;
    let r_b_total = compounded_b - 1.0;
    let big_k = carino_coefficient(r_p_total, r_b_total)?;

    let mut linked_alloc = vec![NeumaierAccumulator::new(); sector_names.len()];
    let mut linked_sel = vec![NeumaierAccumulator::new(); sector_names.len()];
    let mut linked_inter = vec![NeumaierAccumulator::new(); sector_names.len()];

    for period in periods {
        let k_t = carino_coefficient(period.portfolio_return, period.benchmark_return)?;
        let scale = k_t / big_k;
        for (i, e) in period.sectors.iter().enumerate() {
            linked_alloc[i].add(scale * e.allocation);
            linked_sel[i].add(scale * e.selection);
            linked_inter[i].add(scale * e.interaction);
        }
    }

    let mut linked_sectors = Vec::with_capacity(sector_names.len());
    let mut sum_alloc = NeumaierAccumulator::new();
    let mut sum_sel = NeumaierAccumulator::new();
    let mut sum_inter = NeumaierAccumulator::new();
    for (i, name) in sector_names.into_iter().enumerate() {
        let alloc = linked_alloc[i].total();
        let sel = linked_sel[i].total();
        let inter = linked_inter[i].total();
        sum_alloc.add(alloc);
        sum_sel.add(sel);
        sum_inter.add(inter);
        linked_sectors.push(SectorEffect {
            sector: name,
            allocation: alloc,
            selection: sel,
            interaction: inter,
            total: alloc + sel + inter,
        });
    }

    Ok(CarinoLinkedAttribution {
        periods: periods.to_vec(),
        portfolio_return_compounded: r_p_total,
        benchmark_return_compounded: r_b_total,
        linked_sectors,
        linked_allocation: sum_alloc.total(),
        linked_selection: sum_sel.total(),
        linked_interaction: sum_inter.total(),
    })
}

/// Compute Brinson-Fachler attribution for each period and Carino-link the
/// resulting effects across the full horizon.
///
/// This is the canonical entry point for bindings that receive raw sector
/// period inputs rather than precomputed [`BrinsonPeriodResult`] values.
///
/// # Arguments
///
/// * `periods` - Chronologically ordered period slices of raw sector inputs;
///   each slice must satisfy [`brinson_fachler`] weight and finiteness rules.
///
/// # Errors
///
/// Returns any validation error from [`brinson_fachler`] or [`carino_link`].
pub fn carino_link_from_sector_periods(
    periods: &[Vec<SectorPeriod>],
) -> Result<CarinoLinkedAttribution> {
    let period_results = periods
        .iter()
        .map(|sectors| brinson_fachler(sectors))
        .collect::<Result<Vec<_>>>()?;
    carino_link(&period_results)
}

/// Carino smoothing coefficient for a single period or the horizon.
///
/// Formula:
///
/// ```text
/// k = (ln(1 + r_p) − ln(1 + r_b)) / (r_p − r_b)   if r_p ≠ r_b
/// k = 1 / (1 + r_p)                                if r_p = r_b
/// ```
///
/// This is the limit-preserving form from Carino (1999) §4, which keeps
/// the smoothing continuous as `r_p` and `r_b` converge.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when either return is at or below
/// −100 % (`1 + r ≤ 0`). The Carino formula uses `ln(1 + r)`, which is
/// undefined in that regime, and silently substituting unity would mask a
/// genuine modeling error in the upstream period returns.
fn carino_coefficient(r_p: f64, r_b: f64) -> Result<f64> {
    if !r_p.is_finite() || !r_b.is_finite() {
        return Err(Error::invalid_input(format!(
            "Carino coefficient requires finite returns (got r_p = {r_p}, r_b = {r_b})"
        )));
    }
    let one_plus_rp = 1.0 + r_p;
    let one_plus_rb = 1.0 + r_b;
    if one_plus_rp <= 0.0 || one_plus_rb <= 0.0 {
        return Err(Error::invalid_input(format!(
            "Carino coefficient is undefined for returns ≤ -100% \
             (got r_p = {r_p}, r_b = {r_b}); upstream period returns \
             likely contain a modeling error"
        )));
    }
    let diff = r_p - r_b;
    if diff.abs() < 1e-12 {
        Ok(1.0 / one_plus_rp)
    } else {
        Ok((diff / one_plus_rb).ln_1p() / diff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period_two_sector(
        wp_a: f64,
        wb_a: f64,
        rp_a: f64,
        rb_a: f64,
        rp_b: f64,
        rb_b: f64,
    ) -> Vec<SectorPeriod> {
        vec![
            SectorPeriod {
                sector: "A".into(),
                portfolio_weight: wp_a,
                benchmark_weight: wb_a,
                portfolio_return: rp_a,
                benchmark_return: rb_a,
            },
            SectorPeriod {
                sector: "B".into(),
                portfolio_weight: 1.0 - wp_a,
                benchmark_weight: 1.0 - wb_a,
                portfolio_return: rp_b,
                benchmark_return: rb_b,
            },
        ]
    }

    /// The three Brinson-Fachler effects must sum to the active return
    /// exactly — this is the definitional invariant that separates a
    /// correct BF implementation from a drift-prone one.
    #[test]
    fn brinson_effects_reconstruct_active_return() {
        let sectors = period_two_sector(0.60, 0.40, 0.08, 0.06, 0.01, 0.03);
        let r = brinson_fachler(&sectors).expect("valid BF inputs");

        let reconstructed = r.total_allocation + r.total_selection + r.total_interaction;
        assert!(
            (reconstructed - r.total_excess_return).abs() < 1e-12,
            "A + S + I = {reconstructed} must equal active return {}",
            r.total_excess_return
        );
    }

    /// Rejecting malformed weights is part of the production-robustness
    /// contract — Brinson attribution is meaningless if the weights
    /// don't form a convex combination.
    #[test]
    fn brinson_rejects_weights_that_do_not_sum_to_one() {
        let mut sectors = period_two_sector(0.60, 0.40, 0.08, 0.06, 0.01, 0.03);
        sectors[1].portfolio_weight = 0.30; // portfolio totals 0.90, not 1.0

        let err = brinson_fachler(&sectors).expect_err("weights must sum to 1");
        assert!(
            err.to_string().contains("Portfolio weights"),
            "error should name the malformed side: {err}"
        );
    }

    /// Active-return reconstruction still holds with zero benchmark
    /// exposure in a sector — the manager's overweight bet must flow
    /// through Allocation and Interaction without artifacts.
    #[test]
    fn brinson_handles_zero_benchmark_weight_sector() {
        let sectors = vec![
            SectorPeriod {
                sector: "CORE".into(),
                portfolio_weight: 0.80,
                benchmark_weight: 1.00,
                portfolio_return: 0.05,
                benchmark_return: 0.05,
            },
            SectorPeriod {
                sector: "EXTRA".into(),
                portfolio_weight: 0.20,
                benchmark_weight: 0.00,
                portfolio_return: 0.12,
                benchmark_return: 0.00, // irrelevant when benchmark weight is zero
            },
        ];
        let r = brinson_fachler(&sectors).expect("valid BF inputs");
        let reconstructed = r.total_allocation + r.total_selection + r.total_interaction;
        assert!(
            (reconstructed - r.total_excess_return).abs() < 1e-12,
            "A + S + I must reconstruct active return even with zero-weight sectors"
        );
    }

    /// Carino linking must rescale the arithmetic effects so the linked
    /// totals reconstruct the *geometric* compounded active return, not
    /// the arithmetic sum of period active returns.
    #[test]
    fn carino_linking_matches_compounded_active_return() {
        // Two periods, simple two-sector portfolios.
        let p1 = brinson_fachler(&period_two_sector(0.70, 0.50, 0.10, 0.06, 0.04, 0.05))
            .expect("period 1");
        let p2 = brinson_fachler(&period_two_sector(0.60, 0.50, 0.02, 0.03, -0.01, 0.00))
            .expect("period 2");

        let linked = carino_link(&[p1.clone(), p2.clone()]).expect("carino");

        let geometric_active =
            linked.portfolio_return_compounded - linked.benchmark_return_compounded;
        let reconstructed =
            linked.linked_allocation + linked.linked_selection + linked.linked_interaction;
        assert!(
            (reconstructed - geometric_active).abs() < 1e-10,
            "Carino-linked A+S+I = {reconstructed} must equal geometric active return {geometric_active}"
        );

        // Additionally, the arithmetic sum of period active returns does
        // NOT match the geometric active — otherwise Carino smoothing
        // would be a no-op and the test would not be catching anything.
        let arithmetic_active = p1.total_excess_return + p2.total_excess_return;
        assert!(
            (arithmetic_active - geometric_active).abs() > 1e-6,
            "test setup should produce distinct arithmetic vs geometric active returns"
        );
    }

    /// The Carino coefficient is numerically well-defined even when
    /// portfolio and benchmark periods have the same return — the
    /// limit `k = 1 / (1 + r)` must be used instead of the `0/0` ratio
    /// form.
    #[test]
    fn carino_coefficient_handles_equal_returns() {
        let k = carino_coefficient(0.05, 0.05).expect("equal returns must yield 1 / (1 + r)");
        assert!(
            (k - 1.0 / 1.05).abs() < 1e-12,
            "k(0.05, 0.05) must be 1/1.05, got {k}"
        );
    }

    /// Returns at or below −100 % violate the domain of `ln(1 + r)`. The
    /// coefficient must surface this as an error rather than silently
    /// returning unity, which would produce meaningless attribution weights.
    #[test]
    fn carino_coefficient_rejects_negative_unity_or_below() {
        assert!(carino_coefficient(-1.0, 0.05).is_err());
        assert!(carino_coefficient(0.05, -1.5).is_err());
    }

    #[test]
    fn carino_linking_rejects_non_finite_period_returns() {
        let mut p = brinson_fachler(&period_two_sector(0.6, 0.5, 0.03, 0.02, 0.01, 0.01))
            .expect("valid period");
        p.portfolio_return = f64::NAN;

        let err = carino_link(&[p]).expect_err("M-16 non-finite returns must fail");
        assert!(err.to_string().contains("finite"));
    }

    #[test]
    fn carino_linking_rejects_inconsistent_sector_ordering() {
        let p1 = brinson_fachler(&period_two_sector(0.60, 0.50, 0.08, 0.06, 0.02, 0.03))
            .expect("period 1");
        let mut p2 = brinson_fachler(&period_two_sector(0.60, 0.50, 0.08, 0.06, 0.02, 0.03))
            .expect("period 2");
        // Rename the first sector so the sector ordering differs.
        p2.sectors[0].sector = "DIFFERENT".into();

        let err = carino_link(&[p1, p2]).expect_err("must reject inconsistent sectors");
        assert!(
            err.to_string().contains("sector ordering"),
            "expected sector-ordering error: {err}"
        );
    }
}
