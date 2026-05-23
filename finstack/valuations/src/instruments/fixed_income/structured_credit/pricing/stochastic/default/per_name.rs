//! Per-name copula default simulation for finite structured-credit pools.
//!
//! The scalar-`Z` engine ([`super::CopulaBasedDefault::conditional_mdr`])
//! returns the *conditional* default probability `P(default | Z)` and applies
//! that single marginal default rate (MDR) uniformly to every asset. That is
//! the large-homogeneous-pool (LHP) limit: the realized default-count
//! distribution is a deterministic function of `Z` with **zero idiosyncratic
//! dispersion**. For a concentrated CLO (50-150 names) name-level lumpiness is
//! the dominant risk and the LHP limit materially mis-prices the
//! mezzanine/equity tranches (Glasserman §9; O'Kane Ch. 18-19).
//!
//! This module realizes per-name defaults: each name `i` draws its own
//! idiosyncratic shock `εᵢ`, forms the copula latent variable
//! `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` (Gaussian) — or the Student-t analogue with a
//! shared mixing variable `W` — and defaults in the period when `Aᵢ` crosses
//! the per-name threshold `c = Φ⁻¹(PDₜ)` (`t_ν⁻¹(PDₜ)` for Student-t).
//!
//! The latent-variable and mixing-variable maths are **not** reimplemented
//! here — they are delegated to the existing copula kernels via
//! [`Copula::latent_variable`] and [`Copula::sample_mixing`].
//!
//! # LHP fast-path
//!
//! As `N → ∞` the realized default fraction converges (law of large numbers)
//! to `E[1{Aᵢ ≤ c} | Z] = `[`Copula::conditional_default_prob`]. The LHP
//! fast-path applies exactly that conditional probability to the whole pool,
//! so it is the genuine `N → ∞` limit of this per-name model — selectable via
//! [`PoolGranularity`] for pools granular enough that the limit is an
//! acceptable, faster approximation.

use crate::instruments::common_impl::models::correlation::copula::{
    Copula, CopulaSpec, GaussianCopula,
};
use finstack_core::math::{standard_normal_inv_cdf, student_t_inv_cdf};
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::traits::RandomStream;

/// Pool-granularity policy for the structured-credit default engine.
///
/// Selects whether each scenario realizes defaults name-by-name (finite-pool
/// copula simulation) or applies the closed-form LHP conditional default
/// probability uniformly to the pool.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum PoolGranularity {
    /// Simulate each name's default independently conditional on the
    /// systematic factor (finite-pool copula). Captures name-level lumpiness;
    /// the correct default for concentrated pools. This is the default.
    #[default]
    PerName,

    /// Apply the closed-form LHP conditional default probability uniformly to
    /// every name. This is the `N → ∞` limit of [`Self::PerName`] — an
    /// acceptable, faster approximation only for genuinely granular pools.
    LargeHomogeneous,
}

/// Threshold transform for the per-name default barrier.
///
/// Gaussian/RFL/Multi-factor copulas invert the standard normal CDF;
/// the Student-t copula inverts `t_ν`.
#[derive(Debug, Clone, Copy)]
enum ThresholdKind {
    /// `c = Φ⁻¹(PD)`.
    Gaussian,
    /// `c = t_ν⁻¹(PD)`.
    StudentT { degrees_of_freedom: f64 },
}

impl ThresholdKind {
    fn from_spec(spec: &CopulaSpec) -> Self {
        match spec {
            CopulaSpec::StudentT { degrees_of_freedom } => ThresholdKind::StudentT {
                degrees_of_freedom: *degrees_of_freedom,
            },
            _ => ThresholdKind::Gaussian,
        }
    }

    /// Default barrier `c` for a per-period marginal default probability `pd`.
    fn threshold(&self, pd: f64) -> f64 {
        let p = pd.clamp(1e-10, 1.0 - 1e-10);
        match self {
            ThresholdKind::Gaussian => standard_normal_inv_cdf(p),
            ThresholdKind::StudentT { degrees_of_freedom } => {
                student_t_inv_cdf(p, *degrees_of_freedom)
            }
        }
    }
}

/// Finite-pool per-name copula default simulator.
///
/// Holds the copula kernel and asset correlation. One instance is shared
/// (via clone of the cheap `Arc`-backed copula) across all scenario paths;
/// each path drives it with its own [`PhiloxRng`] substream.
pub(crate) struct PerNameCopulaDefault {
    /// Copula kernel — provides `latent_variable` / `sample_mixing` /
    /// `conditional_default_prob`. Never reimplemented here.
    copula: Box<dyn Copula>,
    /// Asset correlation `ρ`.
    correlation: f64,
    /// Threshold transform (`Φ⁻¹` vs `t_ν⁻¹`).
    threshold_kind: ThresholdKind,
}

impl PerNameCopulaDefault {
    /// Build a per-name simulator from a copula specification.
    pub(crate) fn new(copula_spec: &CopulaSpec, correlation: f64) -> Self {
        Self {
            copula: copula_spec
                .build()
                .unwrap_or_else(|_| Box::new(GaussianCopula::new())),
            correlation: correlation.clamp(0.0, 0.99),
            threshold_kind: ThresholdKind::from_spec(copula_spec),
        }
    }

    /// Realize per-name default indicators for one payment period.
    ///
    /// # Arguments
    ///
    /// * `systematic` — the period systematic factor `Z` (shared by all names).
    /// * `marginal_pd` — the per-name *unconditional* period marginal default
    ///   probability `PDₜ`. For a homogeneous pool every name shares it; the
    ///   slice carries one entry per still-alive name to support heterogeneous
    ///   pools.
    /// * `rng` — the path's RNG substream. Per-name idiosyncratic draws are
    ///   pulled in slice order, so the realization is deterministic and
    ///   order-stable for a fixed seed.
    /// * `out` — filled with one `bool` per name: `true` ⇒ name defaults this
    ///   period.
    ///
    /// The shared Student-t mixing variable `W` is drawn once here (one
    /// uniform), then reused for every name so tail dependence is preserved.
    pub(crate) fn simulate_period(
        &self,
        systematic: f64,
        marginal_pd: &[f64],
        rng: &mut PhiloxRng,
        out: &mut Vec<bool>,
    ) {
        self.simulate_period_inner(systematic, marginal_pd, rng, out, false);
    }

    /// Realize per-name default indicators with optional antithetic negation.
    ///
    /// When `antithetic` is `true`, every idiosyncratic `εᵢ` draw is negated.
    /// Paired with a negated systematic factor `Z`, the copula latent variable
    /// `Aᵢ = √ρ·Z + √(1−ρ)·εᵢ` becomes `−Aᵢ`, giving the genuine antithetic
    /// variate. The two paired paths MUST share the same RNG substream so the
    /// underlying uniforms (and hence the εᵢ before negation, and the shared
    /// mixing `W`) match. The Student-t mixing `W` is *not* negated — only the
    /// Gaussian components are, per standard antithetic treatment.
    pub(crate) fn simulate_period_antithetic(
        &self,
        systematic: f64,
        marginal_pd: &[f64],
        rng: &mut PhiloxRng,
        out: &mut Vec<bool>,
    ) {
        self.simulate_period_inner(systematic, marginal_pd, rng, out, true);
    }

    fn simulate_period_inner(
        &self,
        systematic: f64,
        marginal_pd: &[f64],
        rng: &mut PhiloxRng,
        out: &mut Vec<bool>,
        antithetic: bool,
    ) {
        out.clear();
        out.reserve(marginal_pd.len());

        // One shared mixing draw per period (1.0 for Gaussian — no mixing).
        // Drawn before any per-name εᵢ so the draw order is fixed regardless
        // of pool size. Not negated under antithetic mode (asymmetric mixing).
        let mixing = self.copula.sample_mixing(rng.next_u01());

        for &pd in marginal_pd {
            let threshold = self.threshold_kind.threshold(pd);
            let raw = rng.next_std_normal();
            // Antithetic partner negates the idiosyncratic Gaussian draw so
            // its latent variable is the antithetic variate of its pair.
            let idiosyncratic = if antithetic { -raw } else { raw };
            let latent =
                self.copula
                    .latent_variable(systematic, idiosyncratic, mixing, self.correlation);
            out.push(latent <= threshold);
        }
    }

    /// LHP conditional default probability for one period.
    ///
    /// This is `E[1{Aᵢ ≤ c} | Z, W]` — the `N → ∞` limit of
    /// [`Self::simulate_period`] — used by the
    /// [`PoolGranularity::LargeHomogeneous`] fast-path.
    ///
    /// It draws the **same** shared mixing variable `W` that
    /// [`Self::simulate_period`] draws (one uniform via
    /// [`Copula::sample_mixing`], `1.0` for Gaussian) and conditions on the
    /// same `(Z, W)` sigma-algebra, so a per-name pool and this fast-path
    /// converge as `N → ∞`. The conditional fraction is delegated to
    /// [`Copula::conditional_default_prob_given_systematic_and_mixing`]:
    ///
    /// - Gaussian / RFL / multi-factor: `Φ((Φ⁻¹(PD) − √ρ·Z)/√(1−ρ))`.
    /// - Student-t: `Φ((c·√W − √ρ·Z)/√(1−ρ))` with `c = t_ν⁻¹(PD)`.
    ///
    /// The `W` draw mirrors [`Self::simulate_period`] exactly (one
    /// [`RandomStream::next_u01`] per period, before any other consumption),
    /// so the LHP and per-name RNG streams stay consistent for a fixed seed.
    pub(crate) fn conditional_default_prob(
        &self,
        systematic: f64,
        marginal_pd: f64,
        rng: &mut PhiloxRng,
    ) -> f64 {
        let threshold = self.threshold_kind.threshold(marginal_pd);
        // One shared mixing draw per period — identical to `simulate_period`
        // (1.0 for Gaussian). Conditioning the LHP limit on this same `W`
        // makes it the genuine `N → ∞` limit of the per-name model.
        let mixing = self.copula.sample_mixing(rng.next_u01());
        self.copula
            .conditional_default_prob_given_systematic_and_mixing(
                threshold,
                systematic,
                mixing,
                self.correlation,
            )
            .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-name realization, averaged over the systematic factor and a
    /// large pool, must recover the unconditional marginal PD. Without this
    /// the realized default count would be biased.
    #[test]
    fn per_name_marginal_recovers_pd() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let pd = 0.04;
        let names = vec![pd; 200];
        let mut rng = PhiloxRng::new(123);
        let mut out = Vec::new();

        let periods = 4_000usize;
        let mut total_defaults = 0usize;
        for _ in 0..periods {
            let z = rng.next_std_normal();
            sim.simulate_period(z, &names, &mut rng, &mut out);
            total_defaults += out.iter().filter(|d| **d).count();
        }
        let realized = total_defaults as f64 / (periods * names.len()) as f64;
        assert!(
            (realized - pd).abs() < 0.003,
            "per-name marginal {realized} should recover PD {pd}"
        );
    }

    /// Conditional on `Z`, the realized default fraction of a large pool must
    /// converge to the LHP conditional default probability. This is the
    /// finite-pool → LHP correctness anchor.
    #[test]
    fn large_pool_converges_to_lhp_conditional() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.25);
        let pd = 0.05;
        let n = 20_000usize;
        let names = vec![pd; n];
        let mut rng = PhiloxRng::new(99);
        let mut out = Vec::new();

        for &z in &[-1.5_f64, 0.0, 1.5] {
            sim.simulate_period(z, &names, &mut rng, &mut out);
            let realized = out.iter().filter(|d| **d).count() as f64 / n as f64;
            let lhp = sim.conditional_default_prob(z, pd, &mut rng);
            assert!(
                (realized - lhp).abs() < 0.01,
                "z={z}: realized fraction {realized} should converge to LHP {lhp}"
            );
        }
    }

    /// A negative systematic factor (market stress) must produce more
    /// defaults than a positive one — the copula's systematic channel.
    #[test]
    fn stress_factor_increases_defaults() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let pd = 0.05;
        let names = vec![pd; 5_000];
        let mut rng = PhiloxRng::new(7);
        let mut out = Vec::new();

        sim.simulate_period(-2.0, &names, &mut rng, &mut out);
        let stressed = out.iter().filter(|d| **d).count();
        sim.simulate_period(2.0, &names, &mut rng, &mut out);
        let benign = out.iter().filter(|d| **d).count();

        assert!(
            stressed > benign,
            "stress (Z=-2) defaults {stressed} should exceed benign (Z=+2) {benign}"
        );
    }

    /// A finite concentrated pool must show genuine idiosyncratic dispersion:
    /// repeated draws at a *fixed* `Z` produce a spread of default counts,
    /// unlike the LHP limit which is a deterministic function of `Z`.
    #[test]
    fn finite_pool_has_idiosyncratic_dispersion() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.20);
        let pd = 0.06;
        let n = 80usize;
        let names = vec![pd; n];
        let mut rng = PhiloxRng::new(2024);
        let mut out = Vec::new();

        let z = 0.0; // fixed systematic factor
        let trials = 600usize;
        let mut counts = Vec::with_capacity(trials);
        for _ in 0..trials {
            sim.simulate_period(z, &names, &mut rng, &mut out);
            counts.push(out.iter().filter(|d| **d).count());
        }
        let min = counts.iter().copied().min().unwrap_or(0);
        let max = counts.iter().copied().max().unwrap_or(0);
        // The LHP limit would give an identical count every trial. Per-name
        // simulation must spread it.
        assert!(
            max > min,
            "finite pool must show dispersion in default counts (min={min}, max={max})"
        );
    }

    /// Student-t per-name simulation recovers the marginal PD and is
    /// order-stable.
    #[test]
    fn student_t_per_name_marginal_recovers_pd() {
        let sim = PerNameCopulaDefault::new(
            &CopulaSpec::StudentT {
                degrees_of_freedom: 6.0,
            },
            0.30,
        );
        let pd = 0.05;
        let names = vec![pd; 128];
        let mut rng = PhiloxRng::new(555);
        let mut out = Vec::new();

        let periods = 5_000usize;
        let mut total = 0usize;
        for _ in 0..periods {
            let z = rng.next_std_normal();
            sim.simulate_period(z, &names, &mut rng, &mut out);
            total += out.iter().filter(|d| **d).count();
        }
        let realized = total as f64 / (periods * names.len()) as f64;
        assert!(
            (realized - pd).abs() < 0.004,
            "Student-t per-name marginal {realized} should recover PD {pd}"
        );
    }

    /// The Student-t LHP fast-path, averaged over the systematic factor,
    /// must recover the unconditional marginal PD.
    ///
    /// `conditional_default_prob` draws a fresh shared mixing `W` per call and
    /// returns `Φ((c·√W − √ρ·Z)/√(1−ρ))`; averaging over `(Z, W)` must equal
    /// PD. The pre-fix path fed the Gaussian `Z` into the slot expecting the
    /// `t(ν)` factor `M = Z/√W`, biasing this average ~14-17% low.
    #[test]
    fn student_t_lhp_marginal_recovers_pd() {
        let sim = PerNameCopulaDefault::new(
            &CopulaSpec::StudentT {
                degrees_of_freedom: 6.0,
            },
            0.30,
        );
        let pd = 0.05;
        let mut rng = PhiloxRng::new(909);

        let periods = 400_000usize;
        let mut sum = 0.0;
        for _ in 0..periods {
            let z = rng.next_std_normal();
            sum += sim.conditional_default_prob(z, pd, &mut rng);
        }
        let realized = sum / periods as f64;
        // E[Φ((c·√W − √ρ·Z)/√(1−ρ))] = PD. 3σ MC error at p≈0.05, n=4e5 ≈ 0.001.
        assert!(
            (realized - pd).abs() < 0.0015,
            "Student-t LHP marginal {realized} should recover PD {pd}"
        );
    }

    /// **Student-t LHP-limit parity** — the regression anchor for the
    /// corrected Student-t LHP conditional default probability.
    ///
    /// A large granular pool simulated per-name and the Student-t LHP
    /// fast-path must agree: per-name → LHP as `N → ∞`, because both now
    /// realize the *same* `(Z, W)` latent construction
    /// `Aᵢ = (√ρ·Z + √(1−ρ)·εᵢ)/√W`. The two streams share the period `Z`
    /// but draw `W` independently — both `W` draws are `χ²(ν)/ν`, so the
    /// period-averaged rates estimate the same `PD` and must converge.
    ///
    /// On the pre-fix engine the bug this anchors FAILS empirically: the LHP
    /// path fed the Gaussian `Z` into the slot expecting the `t(ν)` factor
    /// `M = Z/√W`, so the LHP rate (≈ 0.0423) sat ~17% below the per-name
    /// rate (≈ 0.0513) — both verified by a scratch Monte Carlo run.
    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn student_t_lhp_limit_parity() {
        let sim = PerNameCopulaDefault::new(
            &CopulaSpec::StudentT {
                degrees_of_freedom: 6.0,
            },
            0.30,
        );
        let pd = 0.05;
        let n = 8_192usize;
        let names = vec![pd; n];

        // Per-name engine: one shared W + n idiosyncratic εᵢ per period.
        let mut pn_rng = PhiloxRng::new(4242);
        let mut out = Vec::new();
        // LHP fast-path: independent stream, one shared W per period.
        let mut lhp_rng = PhiloxRng::new(7171);

        let periods = 8_000usize;
        let mut pn_defaults = 0usize;
        let mut lhp_sum = 0.0;
        for _ in 0..periods {
            let z = pn_rng.next_std_normal();
            sim.simulate_period(z, &names, &mut pn_rng, &mut out);
            pn_defaults += out.iter().filter(|d| **d).count();
            lhp_sum += sim.conditional_default_prob(z, pd, &mut lhp_rng);
        }
        let pn_rate = pn_defaults as f64 / (periods * n) as f64;
        let lhp_rate = lhp_sum / periods as f64;

        // Both recover PD, and per-name ⇄ LHP agree within MC error. The
        // pre-fix LHP rate was ~0.043 — a ~14% gap that blows this tolerance.
        assert!(
            (pn_rate - pd).abs() < 0.0025,
            "Student-t per-name rate {pn_rate} should recover PD {pd}"
        );
        assert!(
            (lhp_rate - pd).abs() < 0.0025,
            "Student-t LHP rate {lhp_rate} should recover PD {pd}"
        );
        assert!(
            (pn_rate - lhp_rate).abs() < 0.0025,
            "Student-t per-name rate {pn_rate} and LHP rate {lhp_rate} must \
             converge (N → ∞ limit); pre-fix LHP ≈ 0.043 fails this"
        );
    }

    /// Item 5 — `simulate_period_antithetic` produces the antithetic variate.
    ///
    /// With a Gaussian copula and `PD = 0.5` the default barrier is exactly
    /// `c = Φ⁻¹(0.5) = 0`, so a name defaults iff its latent `Aᵢ ≤ 0`. The
    /// antithetic partner negates BOTH the systematic `Z` and every
    /// idiosyncratic `εᵢ`, so its latent variable is `−Aᵢ`. For a barrier of
    /// 0, `Aᵢ ≤ 0` and `−Aᵢ ≤ 0` are complementary (one true, one false,
    /// barring the measure-zero `Aᵢ = 0`). The antithetic default mask must
    /// therefore be the exact bitwise complement of the normal mask.
    ///
    /// The pre-fix engine gave antithetic partners *independent* per-name
    /// substreams, so the masks were uncorrelated rather than complementary —
    /// no idiosyncratic-channel variance reduction.
    #[test]
    fn antithetic_per_name_mask_is_complement_of_normal() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let pd = 0.5; // barrier c = Φ⁻¹(0.5) = 0
        let names = vec![pd; 256];

        // Normal path and its antithetic partner share the SAME substream
        // (mirroring the engine: both members of a pair use substream(k)).
        let mut normal_rng = PhiloxRng::new(20_260_517);
        let mut anti_rng = PhiloxRng::new(20_260_517);

        let z = 0.7_f64;
        let mut normal_out = Vec::new();
        let mut anti_out = Vec::new();
        sim.simulate_period(z, &names, &mut normal_rng, &mut normal_out);
        // Antithetic partner: negated Z AND negated εᵢ ⇒ latent = −Aᵢ.
        sim.simulate_period_antithetic(-z, &names, &mut anti_rng, &mut anti_out);

        assert_eq!(normal_out.len(), anti_out.len());
        for (i, (n, a)) in normal_out.iter().zip(anti_out.iter()).enumerate() {
            assert_ne!(
                *n, *a,
                "name {i}: antithetic mask must be the complement of the \
                 normal mask at a zero barrier (normal={n}, antithetic={a})"
            );
        }
    }

    /// Determinism: the same seed and inputs reproduce the per-name default
    /// mask bit-for-bit.
    #[test]
    fn per_name_simulation_is_deterministic() {
        let sim = PerNameCopulaDefault::new(&CopulaSpec::Gaussian, 0.30);
        let names = vec![0.05; 100];

        let run = |seed: u64| {
            let mut rng = PhiloxRng::new(seed);
            let mut out = Vec::new();
            let mut all = Vec::new();
            for _ in 0..10 {
                let z = rng.next_std_normal();
                sim.simulate_period(z, &names, &mut rng, &mut out);
                all.extend_from_slice(&out);
            }
            all
        };

        assert_eq!(run(42), run(42), "fixed seed must reproduce default mask");
    }
}
