"""Validate the Test A financial-statements round-trip and normalization path.

Run the mechanical checks:

    uv run python finstack-quant-py/examples/scripts/statements_test_a.py

Write the named conditional readiness report:

    uv run python finstack-quant-py/examples/scripts/statements_test_a.py \
        --write-report --signer Jon --signed-on 2026-08-02
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import date
import json
import math
import os
from pathlib import Path
import secrets
import sys
from typing import Any, Final

from jsonschema import validators

from finstack_quant.statements import (
    Evaluator,
    FinancialModelSpec,
    ForecastSpec,
    ModelBuilder,
    NormalizationConfig,
    normalize,
)

WORKSPACE_ROOT: Final = Path(__file__).resolve().parents[3]
MODEL_SCHEMA_PATH: Final = (
    WORKSPACE_ROOT / "finstack-quant/statements/schemas/statements/1/financial_model_spec.schema.json"
)
NORMALIZATION_SCHEMA_PATH: Final = (
    WORKSPACE_ROOT / "finstack-quant/statements/schemas/statements/1/normalization_config.schema.json"
)
DEFAULT_REPORT_PATH: Final = WORKSPACE_ROOT / "finstack-quant-py/examples/reports/statements_test_a_report.md"
PERIODS: Final = ("2025Q1", "2025Q2", "2025Q3")
THRESHOLD: Final = 0.30

BASE_VALUES: Final[dict[str, tuple[float, float]]] = {
    "revenue": (1000.0, 1100.0),
    "cogs": (600.0, 650.0),
    "sga": (150.0, 160.0),
    "rnd": (80.0, 90.0),
    "da": (40.0, 42.0),
    "interest_expense": (20.0, 22.0),
    "tax_rate": (0.25, 0.25),
    "cash": (100.0, 120.0),
    "accounts_receivable": (150.0, 165.0),
    "inventory": (80.0, 85.0),
    "other_current_assets": (20.0, 22.0),
    "ppe_net": (500.0, 520.0),
    "other_assets": (50.0, 52.0),
    "accounts_payable": (110.0, 120.0),
    "accrued_expenses": (60.0, 65.0),
    "short_term_debt": (40.0, 35.0),
    "long_term_debt": (300.0, 295.0),
    "other_liabilities": (70.0, 74.0),
    "common_equity": (320.0, 375.0),
    "dividends": (10.0, 12.0),
}

FORMULAS: Final[dict[str, str]] = {
    "gross_profit": "revenue - cogs",
    "operating_expenses": "sga + rnd",
    "ebitda": "gross_profit - operating_expenses",
    "ebit": "ebitda - da",
    "ebt": "ebit - interest_expense",
    "taxes": "ebt * tax_rate",
    "net_income": "ebt - taxes",
    "current_assets": "cash + accounts_receivable + inventory + other_current_assets",
    "total_assets": "current_assets + ppe_net + other_assets",
    "current_liabilities": "accounts_payable + accrued_expenses + short_term_debt",
    "total_debt": "short_term_debt + long_term_debt",
    "total_liabilities": "current_liabilities + long_term_debt + other_liabilities",
    "total_liabilities_equity": "total_liabilities + common_equity",
    "balance_check": "total_assets - total_liabilities_equity",
    "cash_from_operations": "net_income + da",
    "capital_expenditures": "ppe_net * 0.05",
    "cash_from_investing": "0 - capital_expenditures",
    "cash_from_financing": "0 - dividends",
    "net_change_cash": "cash_from_operations + cash_from_investing + cash_from_financing",
    "ebitda_margin": "ebitda / revenue",
}

NORMALIZATION_DOCUMENT: Final[dict[str, Any]] = {
    "target_node": "ebitda",
    "adjustments": [
        {
            "id": "restructuring",
            "name": "Restructuring add-back",
            "value": {
                "type": "fixed",
                "amounts": {"2025Q1": 50.0, "2025Q2": 45.0, "2025Q3": 40.0},
            },
            "cap": {"base_node": "ebitda", "value": 0.10},
        },
        {
            "id": "management_fee",
            "name": "Management fee",
            "value": {
                "type": "percentage_of_node",
                "node_id": "revenue",
                "percentage": 0.01,
            },
            # Self-referential cap (base_node == target_node) with
            # base_mode omitted: exercises Rust's default
            # `CapBaseMode::Reported`, capping against the *reported*
            # (pre-adjustment) EBITDA rather than the running adjusted base.
            "cap": {"base_node": "ebitda", "value": 0.05},
        },
    ],
}

SYNTHETIC_CLASSIFICATIONS: Final = (
    "fixed",
    "fixed",
    "fixed",
    "percentage_of_node",
    "percentage_of_node",
    "fixed",
    "run_rate",
    "run_rate",
    "annualization",
    "annualization",
)
SUPPORTED_CLASSIFICATIONS: Final = frozenset({"fixed", "percentage_of_node"})

EXPECTED_NORMALIZATION: Final = {
    # The management-fee percentage (1% of revenue) is now self-referentially
    # capped at 5% of *reported* EBITDA (base_mode omitted -> default
    # `CapBaseMode::Reported`). Both quarters' 1%-of-revenue raw amounts
    # exceed 5% of reported EBITDA, so the cap binds every period:
    #   2025Q1: raw 10.0 > 170.0 * 0.05 = 8.5   -> capped 8.5
    #   2025Q2: raw 11.0 > 200.0 * 0.05 = 10.0  -> capped 10.0
    #   2025Q3: raw 11.0 > 200.0 * 0.05 = 10.0  -> capped 10.0
    # Had base_mode been explicit `Progressive` instead, the base would
    # widen by the prior restructuring add-back's running total
    # ((170.0 + 17.0) * 0.05 = 9.35 for Q1; (200.0 + 20.0) * 0.05 = 11.0 for
    # Q2/Q3), producing different capped amounts and final values.
    "2025Q1": {
        "base_value": 170.0,
        "raw_amounts": (50.0, 10.0),
        "capped_amounts": (17.0, 8.5),
        "is_capped": (True, True),
        "final_value": 195.5,
    },
    "2025Q2": {
        "base_value": 200.0,
        "raw_amounts": (45.0, 11.0),
        "capped_amounts": (20.0, 10.0),
        "is_capped": (True, True),
        "final_value": 230.0,
    },
    "2025Q3": {
        "base_value": 200.0,
        "raw_amounts": (40.0, 11.0),
        "capped_amounts": (20.0, 10.0),
        "is_capped": (True, True),
        "final_value": 230.0,
    },
}


class TestAFailure(RuntimeError):  # noqa: N818 - brief-defined stage failure type
    """Mechanical Test A failure with a stage-specific message."""


@dataclass(frozen=True)
class TestAEvidence:
    """Evidence retained after all mechanical Test A checks pass."""

    node_count: int
    period_count: int
    adjustment_count: int
    normalized_rows: tuple[dict[str, Any], ...]
    unsupported_count: int
    sample_count: int

    @property
    def unsupported_share(self) -> float:
        """Return the synthetic unsupported-classification share."""
        return self.unsupported_count / self.sample_count


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise TestAFailure(message)


def _validate_schema(document: dict[str, Any], schema_path: Path, stage: str) -> None:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    validator_cls = validators.validator_for(schema)
    validator_cls.check_schema(schema)
    validator = validator_cls(schema)
    errors = sorted(
        validator.iter_errors(document),
        key=lambda error: tuple(str(part) for part in error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"/{'/'.join(str(part) for part in error.absolute_path)}: {error.message}" for error in errors
        )
        raise TestAFailure(f"{stage} schema validation failed: {details}")


def _build_model() -> FinancialModelSpec:
    builder = ModelBuilder("test-a-financial-statements")
    builder.periods("2025Q1..Q3", "2025Q2")
    for node_id, values in BASE_VALUES.items():
        builder.value(node_id, list(zip(PERIODS[:2], values, strict=True)))
        builder.forecast(node_id, ForecastSpec.forward_fill())
    for node_id, formula in FORMULAS.items():
        builder.compute(node_id, formula)
    model = builder.build()
    _require(model.node_count == 40, f"A1 expected 40 nodes, got {model.node_count}")
    _require(model.period_count == 3, f"A1 expected 3 periods, got {model.period_count}")
    return model


def _assert_close(actual: float, expected: float, label: str) -> None:
    if not math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12):
        raise TestAFailure(f"A1 {label}: expected {expected}, got {actual}")


def _assert_normalization(rows: list[dict[str, Any]]) -> None:
    _require(len(rows) == len(PERIODS), f"A1 expected 3 normalization rows, got {len(rows)}")
    for row, period in zip(rows, PERIODS, strict=True):
        expected = EXPECTED_NORMALIZATION[period]
        _require(row["period"] == period, f"A1 expected period {period}, got {row['period']}")
        _assert_close(row["base_value"], expected["base_value"], f"{period} base_value")
        adjustments = row["adjustments"]
        _require(len(adjustments) == 2, f"A1 {period} expected 2 adjustments")
        for index, adjustment in enumerate(adjustments):
            _assert_close(
                adjustment["raw_amount"],
                expected["raw_amounts"][index],
                f"{period} adjustment {index} raw_amount",
            )
            _assert_close(
                adjustment["capped_amount"],
                expected["capped_amounts"][index],
                f"{period} adjustment {index} capped_amount",
            )
            _require(
                adjustment["is_capped"] is expected["is_capped"][index],
                f"A1 {period} adjustment {index} cap flag mismatch",
            )
        _assert_close(row["final_value"], expected["final_value"], f"{period} final_value")


def run_checks() -> TestAEvidence:
    """Run A1/A2 mechanics and the explicitly synthetic A3 dry run."""
    model = _build_model()
    model_document = json.loads(model.to_json())
    round_tripped_model = FinancialModelSpec.from_json(json.dumps(model_document))
    round_tripped_model_document = json.loads(round_tripped_model.to_json())
    _require(
        round_tripped_model_document == model_document,
        "A1 FinancialModelSpec JSON changed across the Rust round-trip",
    )
    _validate_schema(round_tripped_model_document, MODEL_SCHEMA_PATH, "A1 model")

    config = NormalizationConfig.from_json(json.dumps(NORMALIZATION_DOCUMENT))
    config_document = json.loads(config.to_json())
    _require(
        config_document == NORMALIZATION_DOCUMENT,
        "A1 NormalizationConfig JSON changed across the Rust round-trip",
    )
    _validate_schema(config_document, NORMALIZATION_SCHEMA_PATH, "A2 normalization")

    results = Evaluator().evaluate(round_tripped_model)
    normalized = [json.loads(row.to_json()) for row in normalize(results, config)]
    _require(isinstance(normalized, list), "A1 normalize did not return a JSON list")
    _assert_normalization(normalized)

    unsupported_count = sum(
        classification not in SUPPORTED_CLASSIFICATIONS for classification in SYNTHETIC_CLASSIFICATIONS
    )
    evidence = TestAEvidence(
        node_count=round_tripped_model.node_count,
        period_count=round_tripped_model.period_count,
        adjustment_count=config.adjustment_count,
        normalized_rows=tuple(normalized),
        unsupported_count=unsupported_count,
        sample_count=len(SYNTHETIC_CLASSIFICATIONS),
    )
    _require(
        evidence.unsupported_share > THRESHOLD,
        "A3 synthetic fixture must exercise the greater-than-30% branch",
    )
    return evidence


def render_report(evidence: TestAEvidence, signer: str, signed_on: date) -> str:
    """Render the deterministic named conditional readiness report."""
    bridge_rows = "\n".join(
        f"| {row['period']} | {row['base_value']:.1f} | "
        f"{row['adjustments'][0]['capped_amount']:.1f} | "
        f"{row['adjustments'][1]['capped_amount']:.1f} | {row['final_value']:.1f} |"
        for row in evidence.normalized_rows
    )
    return f"""# Test A — Financial Statements

Decision: **CONDITIONAL**

The model and normalization contracts are mechanically ready. Empirical
`AdjustmentValue` coverage remains pending Test B.

## A1 — Round-trip validity

- Status: **PASS**
- Model: `{evidence.node_count}` nodes across `{evidence.period_count}` periods
- Adjustments: `{evidence.adjustment_count}`
- `FinancialModelSpec` and `NormalizationConfig` both round-trip structurally
- Both documents validate against their checked-in Draft 2020-12 schemas
- Rust evaluation and `adjustments::normalize` complete cleanly

| Period | Reported EBITDA | Fixed applied | Percentage applied | Final EBITDA |
|---|---:|---:|---:|---:|
{bridge_rows}

## A2 — Adjustment schema

- Status: **PASS**
- Five input types derive `JsonSchema`
- Artifact: `finstack-quant/statements/schemas/statements/1/normalization_config.schema.json`
- Supported variants: `fixed`, `percentage_of_node`

## Reproducibility

- Model schema: `{MODEL_SCHEMA_PATH.relative_to(WORKSPACE_ROOT)}`
- Normalization schema: `{NORMALIZATION_SCHEMA_PATH.relative_to(WORKSPACE_ROOT)}`
- Report-generation command:

  ```
  uv run python finstack-quant-py/examples/scripts/statements_test_a.py --write-report --signer {signer} --signed-on {signed_on.isoformat()}
  ```

## A3 — Variant measurement

- Status: **CONDITIONAL**
- Evidence source: **synthetic dry run**
- Unsupported synthetic share: **{evidence.unsupported_count} / {evidence.sample_count} ({evidence.unsupported_share:.1%})**
- Threshold result: `provisional_trigger` because `{evidence.unsupported_share:.1%} > {THRESHOLD:.0%}`
- Production decision: `pending_test_b`

The synthetic percentage is not an observed corpus result. If Test B measures
the real unsupported share above 30%, add canonical Rust run-rate or
annualization semantics before M4 freezes the contract.

## Root-cause actions

- Published the missing Rust-owned normalization schema.
- Preserved adjustments as a sidecar rather than changing `FinancialModelSpec`.
- Kept financial logic in Rust.
- Corrected stale notebook claims about variants and cap defaults.

## A4 — Quant-contract readiness attestation

Decision: **CONDITIONAL**

Signer: **{signer}**

Signed on: **{signed_on.isoformat()}**
"""


def write_report(path: Path, content: str) -> None:
    """Atomically replace the report after all checks have passed."""
    path.parent.mkdir(parents=True, exist_ok=True)
    # An exclusive create takes the process umask, so the published report has
    # the same permissions as any other file the user writes.
    temporary_path = path.with_name(f".{path.name}.{secrets.token_hex(8)}")
    output = temporary_path.open("x", encoding="utf-8")
    try:
        with output:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        temporary_path.replace(path)
    except Exception:
        temporary_path.unlink(missing_ok=True)
        raise


def _iso_date(value: str) -> date:
    try:
        return date.fromisoformat(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected YYYY-MM-DD") from error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict-readiness", action="store_true")
    parser.add_argument("--write-report", action="store_true")
    parser.add_argument("--report-path", type=Path, default=DEFAULT_REPORT_PATH)
    parser.add_argument("--signer")
    parser.add_argument("--signed-on", type=_iso_date)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    if args.write_report and (not args.signer or args.signed_on is None):
        parser.error("--write-report requires --signer and --signed-on")

    try:
        evidence = run_checks()
    except Exception as error:
        print(f"Test A mechanical failure: {error}", file=sys.stderr)
        return 1

    if args.write_report:
        try:
            report = render_report(evidence, args.signer, args.signed_on)
            write_report(args.report_path, report)
        except Exception as error:
            print(f"Test A report write failure: {error}", file=sys.stderr)
            return 1

    print("A1 PASS — 40 nodes, 3 periods, 2 adjustments")
    print("A2 PASS — normalization schema published and validated")
    print(f"A3 CONDITIONAL — synthetic unsupported share {evidence.unsupported_share:.1%}")
    print("A4 CONDITIONAL — production evidence pending Test B")
    if args.write_report:
        print(f"Wrote {args.report_path}")
    return 2 if args.strict_readiness else 0


if __name__ == "__main__":
    raise SystemExit(main())
