"""Runtime coverage for ``finstack_quant.models.credit.migration`` bindings."""

import math

import pytest

from finstack_quant.models.credit import migration


def test_project_two_state_generator() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.01, 0.01, 0.0, 0.0])

    projected = migration.project(gen, 5.0)

    assert projected.horizon == pytest.approx(5.0)
    assert projected.probability("AAA", "D") == pytest.approx(1.0 - math.exp(-0.05), rel=1e-4)
    assert projected.probability("D", "D") == pytest.approx(1.0)
    assert projected.default_probabilities() is not None


def test_scale_and_matrix_validation_errors() -> None:
    with pytest.raises(Exception, match=r"Insufficient|states|State"):
        migration.RatingScale.custom(["AAA"])

    scale = migration.RatingScale.custom(["AAA", "D"])
    with pytest.raises(ValueError, match="data must contain 2 x 2 values"):
        migration.TransitionMatrix(scale, [1.0, 0.0, 0.0], 1.0)


def test_seeded_simulation_is_deterministic() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.25, 0.25, 0.0, 0.0])
    sim = migration.MigrationSimulator(gen, 3.0)

    paths_a = sim.simulate(0, 8, 42)
    paths_b = sim.simulate(0, 8, 42)

    assert [p.transitions() for p in paths_a] == [p.transitions() for p in paths_b]
    assert all(p.label_at(0.0) == "AAA" for p in paths_a)
    assert all(p.horizon == pytest.approx(3.0) for p in paths_a)
    assert isinstance(paths_a, migration.RatingPaths)
    assert len(paths_a) == 8
    assert paths_a[-1].scale == scale
    frame = paths_a.to_dataframe()
    assert list(frame.columns) == ["path", "time", "state", "label"]
    assert frame["path"].nunique() == 8
    assert set(frame["label"]) <= {"AAA", "D"}
    assert migration.RatingPaths.from_json(paths_a.to_json()).to_dataframe().equals(frame)


def test_empirical_matrix_shape() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.05, 0.05, 0.0, 0.0])
    sim = migration.MigrationSimulator(gen, 1.0)

    matrix = sim.empirical_matrix(20, 7)

    assert matrix.n_states == 2
    assert len(matrix.to_matrix()) == 2
    assert all(len(row) == 2 for row in matrix.to_matrix())


def test_matrices_accept_nested_rows_and_export_labelled_frames() -> None:
    import pickle

    scale = migration.RatingScale.custom(["A", "D"])
    nested = migration.TransitionMatrix(scale, [[0.9, 0.1], [0.0, 1.0]], 1.0)
    flat = migration.TransitionMatrix(scale, [0.9, 0.1, 0.0, 1.0], 1.0)
    assert nested.to_matrix() == flat.to_matrix()
    assert nested.probability_by_index(0, 1) == pytest.approx(0.1)
    assert nested.probability(from_="A", to="D") == pytest.approx(0.1)
    assert nested.scale == scale

    frame = nested.to_dataframe()
    assert list(frame.index) == ["A", "D"] == list(frame.columns)
    assert frame.loc["A", "D"] == pytest.approx(0.1)
    rebuilt = migration.TransitionMatrix.from_dataframe(frame, 1.0)
    assert rebuilt.to_matrix() == nested.to_matrix()

    two_year = nested.compose(nested)
    assert two_year.horizon == pytest.approx(2.0)
    assert two_year.probability("A", "A") == pytest.approx(0.81)

    assert migration.TransitionMatrix.from_json(nested.to_json()).to_matrix() == nested.to_matrix()
    assert pickle.loads(pickle.dumps(nested)).to_matrix() == nested.to_matrix()  # noqa: S301 - trusted in-process round trip

    gen = migration.GeneratorMatrix(scale, [[-0.1, 0.1], [0.0, 0.0]])
    assert gen.intensity(from_="A", to="D") == pytest.approx(0.1)
    assert gen.scale == scale
    assert list(gen.to_dataframe().columns) == ["A", "D"]
    assert pickle.loads(pickle.dumps(gen)).to_matrix() == gen.to_matrix()  # noqa: S301 - trusted in-process round trip
    tol = migration.GeneratorMatrix.from_transition_matrix_with_tol(nested, 1e-6)
    assert tol.round_trip_error <= 1e-6

    sim = migration.MigrationSimulator(gen, 2.0)
    assert sim.generator.to_matrix() == gen.to_matrix()
    assert migration.MigrationSimulator.from_json(sim.to_json()).horizon == pytest.approx(2.0)


def test_rating_scale_helpers_and_round_trip() -> None:
    scale = migration.RatingScale.custom_with_default(["A", "B", "D"], "D")
    assert scale.n_states == 3 == len(scale)
    assert scale.label_of(1) == "B"
    assert scale.label_of(9) is None
    assert scale.index_of_required("B") == 1
    with pytest.raises(KeyError):
        scale.index_of_required("Z")
    assert migration.RatingScale.from_json(scale.to_json()) == scale
    assert repr(scale) == 'RatingScale(labels=["A", "B", "D"], default_state=2)'


def test_simulation_validation_maps_to_python_errors() -> None:
    scale = migration.RatingScale.custom(["AAA", "D"])
    gen = migration.GeneratorMatrix(scale, [-0.05, 0.05, 0.0, 0.0])
    sim = migration.MigrationSimulator(gen, 1.0)

    with pytest.raises(Exception, match=r"state index|out of range"):
        sim.simulate(2, 1, 7)
    with pytest.raises(Exception, match=r"paths per state|positive"):
        sim.empirical_matrix(0, 7)
