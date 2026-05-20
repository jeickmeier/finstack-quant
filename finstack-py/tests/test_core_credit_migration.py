"""Runtime coverage for ``finstack.core.credit.migration`` bindings."""

import math

from finstack.core.credit import migration
import pytest


def test_project_two_state_generator() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.01, 0.01, 0.0, 0.0])

    projected = migration.project(gen, 5.0)

    assert projected.horizon() == pytest.approx(5.0)
    assert projected.probability("AAA", "D") == pytest.approx(1.0 - math.exp(-0.05), rel=1e-4)
    assert projected.probability("D", "D") == pytest.approx(1.0)
    assert projected.default_probabilities() is not None


def test_scale_and_matrix_validation_errors() -> None:
    with pytest.raises(Exception, match=r"Insufficient|states|State"):
        migration.RatingScale.custom(["AAA"])

    scale = migration.RatingScale.custom(["AAA", "D"])
    with pytest.raises(Exception, match=r"Dimension|dimension|expected"):
        migration.TransitionMatrix(scale, [1.0, 0.0, 0.0], 1.0)


def test_seeded_simulation_is_deterministic() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.25, 0.25, 0.0, 0.0])
    sim = migration.MigrationSimulator(gen, 3.0)

    paths_a = sim.simulate(0, 8, 42)
    paths_b = sim.simulate(0, 8, 42)

    assert [p.transitions() for p in paths_a] == [p.transitions() for p in paths_b]
    assert all(p.label_at(0.0) == "AAA" for p in paths_a)
    assert all(p.horizon() == pytest.approx(3.0) for p in paths_a)


def test_empirical_matrix_shape() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.05, 0.05, 0.0, 0.0])
    sim = migration.MigrationSimulator(gen, 1.0)

    matrix = sim.empirical_matrix(20, 7)

    assert matrix.n_states() == 2
    assert len(matrix.to_matrix()) == 2
    assert all(len(row) == 2 for row in matrix.to_matrix())
