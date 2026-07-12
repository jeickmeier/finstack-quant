"""Credit APIs reject non-finite inputs without panicking."""

import math

import pytest

from finstack_quant.core import credit


@pytest.mark.parametrize("invalid", [float("nan"), float("inf"), -float("inf")])
def test_credit_non_finite_inputs_raise_value_error(invalid: float) -> None:
    with pytest.raises(ValueError, match=r"(?i)quantile.*finite"):
        credit.lgd.beta_recovery_quantile(0.4, 0.2, invalid)
    with pytest.raises(ValueError, match=r"(?i)WARF.*finite"):
        credit.migration.RatingScale.standard().rating_from_warf(invalid)
    with pytest.raises(ValueError, match=r"(?i)non-finite"):
        credit.pd.ttc_to_pit(0.02, 0.2, invalid)
    with pytest.raises(ValueError, match=r"(?i)non-finite"):
        credit.pd.pit_to_ttc(0.02, 0.2, invalid)


def test_generator_matrix_exposes_extraction_diagnostics() -> None:
    scale = credit.migration.RatingScale.custom_with_default(["A", "D"], "D")
    direct = credit.migration.GeneratorMatrix(scale, [-0.1, 0.1, 0.0, 0.0])
    assert direct.regularization_l1 == 0.0
    assert direct.round_trip_error == 0.0

    transition = credit.migration.TransitionMatrix(scale, [0.9, 0.1, 0.0, 1.0], 1.0)
    extracted = credit.migration.GeneratorMatrix.from_transition_matrix(transition)
    assert math.isfinite(extracted.regularization_l1)
    assert math.isfinite(extracted.round_trip_error)
    assert extracted.regularization_l1 >= 0.0
    assert extracted.round_trip_error >= 0.0
