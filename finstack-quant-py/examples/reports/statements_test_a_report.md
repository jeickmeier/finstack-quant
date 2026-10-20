# Test A — Financial Statements

Decision: **CONDITIONAL**

The model and normalization contracts are mechanically ready. Empirical
`AdjustmentValue` coverage remains pending Test B.

## A1 — Round-trip validity

- Status: **PASS**
- Model: `40` nodes across `3` periods
- Adjustments: `2`
- `FinancialModelSpec` and `NormalizationConfig` both round-trip structurally
- Both documents validate against their checked-in Draft 2020-12 schemas
- Rust evaluation and `adjustments::normalize` complete cleanly

| Period | Reported EBITDA | Fixed applied | Percentage applied | Final EBITDA |
|---|---:|---:|---:|---:|
| 2025Q1 | 170.0 | 17.0 | 8.5 | 195.5 |
| 2025Q2 | 200.0 | 20.0 | 10.0 | 230.0 |
| 2025Q3 | 200.0 | 20.0 | 10.0 | 230.0 |

## A2 — Adjustment schema

- Status: **PASS**
- Five input types derive `JsonSchema`
- Artifact: `finstack-quant/statements/schemas/statements/1/normalization_config.schema.json`
- Supported variants: `fixed`, `percentage_of_node`

## Reproducibility

- Model schema: `finstack-quant/statements/schemas/statements/1/financial_model_spec.schema.json`
- Normalization schema: `finstack-quant/statements/schemas/statements/1/normalization_config.schema.json`
- Report-generation command:

  ```
  uv run python finstack-quant-py/examples/scripts/statements_test_a.py --write-report --signer Jon --signed-on 2026-08-02
  ```

## A3 — Variant measurement

- Status: **CONDITIONAL**
- Evidence source: **synthetic dry run**
- Unsupported synthetic share: **4 / 10 (40.0%)**
- Threshold result: `provisional_trigger` because `40.0% > 30%`
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

Signer: **Jon**

Signed on: **2026-08-02**
