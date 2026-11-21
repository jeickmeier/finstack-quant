"""Empty result frames must carry real dtypes, not `object`.

`pd.DataFrame([], columns=[...])` types every column `object`. Concatenating
such a frame with a populated one downgrades EVERY column, so a numeric column
silently stops being numeric and `groupby().sum()`, arithmetic and `to_parquet`
break. Iterating a book and concatenating per-instrument frames is the normal
way to use these exports, and some instruments legitimately produce no rows —
so the empty case is on the common path, not an edge case.
"""

from __future__ import annotations

import json
from pathlib import Path

import pandas as pd
import pytest

from finstack_quant.attribution import PnlAttribution
from finstack_quant.statements import CheckReport
from finstack_quant.statements_analytics import CorkscrewReport, VarianceReport


def _variance_report(rows: list[dict[str, object]]) -> VarianceReport:
    return VarianceReport.from_json(json.dumps({"baseline_label": "base", "comparison_label": "comp", "rows": rows}))


VARIANCE_ROW = {
    "period": "2025Q1",
    "metric": "revenue",
    "baseline": 100.0,
    "comparison": 110.0,
    "abs_var": 10.0,
    "pct_var": 0.1,
}


class TestVarianceReport:
    """The reference case: mixed text and float columns."""

    def test_empty_and_populated_frames_have_identical_dtypes(self) -> None:
        """Identical dtypes are what makes a concat lossless.

        Asserting the empty frame is merely "not all object" would still allow
        a mismatch that downgrades on concat, so compare the two directly.
        """
        empty = _variance_report([]).to_pandas()
        populated = _variance_report([VARIANCE_ROW]).to_pandas()
        assert list(empty.columns) == list(populated.columns)
        assert empty.dtypes.astype(str).to_dict() == populated.dtypes.astype(str).to_dict()

    def test_concat_preserves_numeric_dtypes(self) -> None:
        empty = _variance_report([]).to_pandas()
        populated = _variance_report([VARIANCE_ROW]).to_pandas()
        combined = pd.concat([empty, populated], ignore_index=True)

        assert len(combined) == 1
        for column in ("baseline", "comparison", "abs_var", "pct_var"):
            assert pd.api.types.is_numeric_dtype(combined[column]), column

    def test_concat_result_supports_arithmetic_aggregation(self) -> None:
        """The end the dtypes are a means to: numbers still behave like numbers."""
        empty = _variance_report([]).to_pandas()
        populated = _variance_report([VARIANCE_ROW]).to_pandas()
        combined = pd.concat([empty, populated], ignore_index=True)

        assert combined.groupby("metric")["baseline"].sum().loc["revenue"] == pytest.approx(100.0)
        assert (combined["comparison"] - combined["baseline"]).iloc[0] == pytest.approx(10.0)

    def test_empty_frame_columns_are_not_object(self) -> None:
        """Pins the specific regression: `pd.DataFrame([], columns=...)`."""
        empty = _variance_report([]).to_pandas()
        assert len(empty) == 0
        numeric = ["baseline", "comparison", "abs_var", "pct_var"]
        assert [str(empty.dtypes[c]) for c in numeric] == ["float64"] * len(numeric)


class TestIntegerAndBooleanColumns:
    """`int64` and `bool` degrade to `object` just as readily as `float64`."""

    @staticmethod
    def _corkscrew(validations: list[dict[str, object]]) -> CorkscrewReport:
        return CorkscrewReport.from_json(
            json.dumps({
                "status": "success",
                "message": "ok",
                "data": {"validations": validations},
                "warnings": [],
                "errors": [],
            })
        )

    def test_empty_validations_frame_keeps_int_and_bool_dtypes(self) -> None:
        empty = self._corkscrew([]).to_validations_table().to_pandas()
        assert str(empty.dtypes["periods_validated"]) == "int64"
        assert str(empty.dtypes["is_valid"]) == "bool"
        assert str(empty.dtypes["max_error"]) == "float64"

    def test_populated_validations_frame_matches_the_empty_schema(self) -> None:
        row = {
            "account": "debt",
            "type": "balance",
            "periods_validated": 4,
            "max_error": 0.5,
            "is_valid": True,
        }
        empty = self._corkscrew([]).to_validations_table().to_pandas()
        populated = self._corkscrew([row]).to_validations_table().to_pandas()
        assert list(empty.columns) == list(populated.columns)
        combined = pd.concat([empty, populated], ignore_index=True)
        assert pd.api.types.is_integer_dtype(combined["periods_validated"])
        assert pd.api.types.is_bool_dtype(combined["is_valid"])


class TestOtherSchemaFrames:
    """The same guarantee across the other schema-carrying exports."""

    def test_check_report_frames_are_typed_when_empty(self) -> None:
        report = CheckReport.from_json(
            json.dumps({
                "outcome": "unevaluated",
                "results": [],
                "summary": {
                    "total_checks": 0,
                    "passed": 0,
                    "failed": 0,
                    "incomplete": 0,
                    "unevaluated": 0,
                    "errors": 0,
                    "warnings": 0,
                    "infos": 0,
                },
            })
        )
        checks = report.to_pandas()
        findings = report.to_findings_table().to_pandas()
        assert str(checks.dtypes["outcome"]) == "str"
        assert str(checks.dtypes["finding_count"]) == "int64"
        assert str(findings.dtypes["materiality_absolute"]) == "float64"

    def test_attribution_long_frame_is_typed_when_empty(self) -> None:
        fixture = Path(__file__).parent / "fixtures" / "attribution_baseline.json"
        baseline = json.loads(fixture.read_text())
        empty = PnlAttribution.from_json(json.dumps(baseline)).to_long_table().to_pandas()
        assert len(empty) == 0
        assert str(empty.dtypes["amount"]) == "float64"
