"""Tests for correlation types: copulas, Bernoulli, factor models, bounds."""

import pytest

from finstack_quant.valuations.correlation import (
    CopulaSpec,
    CorrelatedBernoulli,
    LatentFactorSpec,
    LatentMultiFactor,
    cholesky_decompose,
    correlation_bounds,
    joint_probabilities,
    validate_correlation_matrix,
)


class TestCopulaSpec:
    """CopulaSpec construction and build round-trip."""

    def test_gaussian_builds(self) -> None:
        """Gaussian copula builds and produces a conditional default prob."""
        spec = CopulaSpec.gaussian()
        assert spec.is_gaussian
        copula = spec.build()
        assert "Gaussian" in copula.model_name
        assert copula.num_factors >= 1

    def test_gaussian_conditional_default_prob(self) -> None:
        """Conditional default prob is in [0, 1] for valid inputs."""
        copula = CopulaSpec.gaussian().build()
        p = copula.conditional_default_prob(-1.5, [0.0], 0.3)
        assert 0.0 <= p <= 1.0

    def test_gaussian_extreme_correlation(self) -> None:
        """At correlation=0 the factor drops out; P(default|Z) ≈ Φ(threshold)."""
        copula = CopulaSpec.gaussian().build()
        threshold = 0.0  # Φ(0) = 0.5
        p = copula.conditional_default_prob(threshold, [0.0], 0.0)
        assert p == pytest.approx(0.5, abs=0.01)

    def test_student_t_builds(self) -> None:
        """Student-t copula requires df > 2."""
        spec = CopulaSpec.student_t(5.0)
        assert spec.is_student_t
        copula = spec.build()
        # Shared-W t-copula integrates over [Z, W]: 2 quadrature factors.
        assert copula.num_factors == 2

    def test_student_t_invalid_df(self) -> None:
        """Student-t with df <= 2 should raise."""
        with pytest.raises(ValueError, match=r"(?i)degrees|freedom|df|greater"):
            CopulaSpec.student_t(2.0)

    def test_rfl_builds(self) -> None:
        """Random Factor Loading copula builds successfully."""
        spec = CopulaSpec.random_factor_loading(0.2)
        assert spec.is_rfl
        copula = spec.build()
        assert copula.num_factors >= 1

    def test_tail_dependence(self) -> None:
        """Gaussian tail dependence at correlation 0.5 is non-negative."""
        copula = CopulaSpec.gaussian().build()
        td = copula.tail_dependence(0.5)
        assert td >= 0.0


class TestCorrelationBounds:
    """Fréchet-Hoeffding bounds for correlated Bernoulli variables."""

    def test_equal_probabilities(self) -> None:
        """When p1 == p2, rho_max should be 1.0."""
        lo, hi = correlation_bounds(0.5, 0.5)
        assert hi == pytest.approx(1.0, abs=1e-6)
        assert lo <= 0.0

    def test_asymmetric_probabilities(self) -> None:
        """Bounds for (0.1, 0.9) are narrow and contain zero."""
        lo, hi = correlation_bounds(0.1, 0.9)
        assert lo < 0.0
        assert hi > 0.0

    def test_degenerate_zero(self) -> None:
        """When one probability is 0, bounds collapse."""
        lo, hi = correlation_bounds(0.0, 0.5)
        assert lo == pytest.approx(0.0, abs=1e-10)
        assert hi == pytest.approx(0.0, abs=1e-10)


class TestCorrelatedBernoulli:
    """Correlated Bernoulli joint probability computations."""

    def test_joint_probabilities_sum_to_one(self) -> None:
        """Four joint probabilities must sum to exactly 1."""
        cb = CorrelatedBernoulli(0.3, 0.5, 0.2)
        p11, p10, p01, p00 = cb.joint_probabilities()
        assert p11 + p10 + p01 + p00 == pytest.approx(1.0, abs=1e-10)

    def test_marginals_preserved(self) -> None:
        """Joint probabilities preserve the marginal probabilities."""
        cb = CorrelatedBernoulli(0.3, 0.5, 0.2)
        p11, p10, p01, _p00 = cb.joint_probabilities()
        assert p11 + p10 == pytest.approx(0.3, abs=1e-8)
        assert p11 + p01 == pytest.approx(0.5, abs=1e-8)

    def test_zero_correlation(self) -> None:
        """At zero correlation, p11 = p1 * p2."""
        cb = CorrelatedBernoulli(0.4, 0.6, 0.0)
        assert cb.joint_p11 == pytest.approx(0.4 * 0.6, abs=1e-8)

    def test_property_accessors(self) -> None:
        """Getters return stored marginals and correlation."""
        cb = CorrelatedBernoulli(0.2, 0.8, 0.1)
        assert cb.p1 == pytest.approx(0.2)
        assert cb.p2 == pytest.approx(0.8)
        assert cb.requested_correlation == pytest.approx(0.1)
        assert cb.correlation == pytest.approx(0.1, abs=0.05)

    def test_requested_and_effective_correlation_expose_clamping(self) -> None:
        cb = CorrelatedBernoulli(0.05, 0.95, 0.9)
        assert cb.requested_correlation == pytest.approx(0.9)
        assert cb.correlation < cb.requested_correlation

    def test_conditional_probabilities(self) -> None:
        """Conditional P(X2=1|X1=1) = p11 / p1 when p1 > 0."""
        cb = CorrelatedBernoulli(0.5, 0.5, 0.5)
        p_cond = cb.conditional_p2_given_x1()
        assert 0.0 <= p_cond <= 1.0

    @pytest.mark.parametrize("invalid", [float("nan"), float("inf"), -float("inf")])
    def test_non_finite_inputs_raise_value_error(self, invalid: float) -> None:
        with pytest.raises(ValueError, match=r"(?i)marginal p1.*finite"):
            CorrelatedBernoulli(invalid, 0.5, 0.0)
        with pytest.raises(ValueError, match=r"(?i)correlation.*finite"):
            CorrelatedBernoulli(0.5, 0.5, invalid)

    def test_invalid_uniform_raises_value_error(self) -> None:
        cb = CorrelatedBernoulli(0.5, 0.5, 0.0)
        for invalid in (-0.1, 1.1, float("nan")):
            with pytest.raises(ValueError, match=r"(?i)uniform.*finite"):
                cb.sample_from_uniform(invalid)


class TestJointProbabilities:
    """Module-level joint_probabilities function."""

    def test_sum_to_one(self) -> None:
        """Four-tuple sums to 1.0."""
        p11, p10, p01, p00 = joint_probabilities(0.3, 0.4, 0.1)
        assert p11 + p10 + p01 + p00 == pytest.approx(1.0, abs=1e-10)


class TestValidateCorrelationMatrix:
    """Validation of flattened correlation matrices."""

    def test_identity_valid(self) -> None:
        """2x2 identity is a valid correlation matrix."""
        validate_correlation_matrix([1.0, 0.0, 0.0, 1.0], 2)

    def test_invalid_diagonal(self) -> None:
        """Non-unity diagonal should fail."""
        with pytest.raises(ValueError, match=r"(?i)diagonal|correlation|invalid"):
            validate_correlation_matrix([2.0, 0.0, 0.0, 1.0], 2)


class TestLatentMultiFactor:
    """Multi-factor latent model: correlated factor generation."""

    def test_generate_correlated_factors_roundtrip(self) -> None:
        """Valid-length input returns one correlated value per factor."""
        model = LatentMultiFactor.uncorrelated(2, [1.0, 1.0])
        out = model.generate_correlated_factors([0.5, -0.25])
        assert len(out) == 2

    def test_generate_correlated_factors_bad_length_raises_value_error(self) -> None:
        """Wrong-length input raises ValueError, not a Rust panic."""
        model = LatentMultiFactor.uncorrelated(2, [1.0, 1.0])
        with pytest.raises(ValueError, match=r"(?i)exactly 2 draws"):
            model.generate_correlated_factors([0.5])
        with pytest.raises(ValueError, match=r"(?i)exactly 2 draws"):
            model.generate_correlated_factors([0.5, 0.1, -0.7])

    def test_latent_factor_kind_is_rust_canonical_name(self) -> None:
        """LatentFactorSpec.build() returns the Rust-canonical LatentFactorKind."""
        built = LatentFactorSpec.single_factor(0.2, 0.1).build()
        assert type(built).__name__ == "LatentFactorKind"
        assert built.num_factors == 1


class TestCholeskyDecompose:
    """Cholesky decomposition of flattened correlation matrices."""

    def test_identity(self) -> None:
        """Cholesky of 2x2 identity is identity."""
        lower = cholesky_decompose([1.0, 0.0, 0.0, 1.0], 2)
        assert lower[0] == pytest.approx(1.0)
        assert lower[1] == pytest.approx(0.0)
        assert lower[2] == pytest.approx(0.0)
        assert lower[3] == pytest.approx(1.0)

    def test_correlated(self) -> None:
        """Factor matrix for [[1, 0.5], [0.5, 1]] satisfies A A^T = R."""
        lower = cholesky_decompose([1.0, 0.5, 0.5, 1.0], 2)
        assert len(lower) == 4
        a00, a01, a10, a11 = lower
        # AA^T should recover the original correlation matrix
        r00 = a00 * a00 + a01 * a01
        r01 = a00 * a10 + a01 * a11
        r11 = a10 * a10 + a11 * a11
        assert r00 == pytest.approx(1.0, abs=1e-8)
        assert r01 == pytest.approx(0.5, abs=1e-8)
        assert r11 == pytest.approx(1.0, abs=1e-8)
