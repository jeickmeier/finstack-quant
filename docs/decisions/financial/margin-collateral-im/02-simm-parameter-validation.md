# MARGIN-COLLATERAL-IM-002 — SIMM parameter completeness and correlation validation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: constructors validate the parameter set before caching an IR tenor matrix. Proposed rationale: missing weights or invalid dependence matrices must not silently remove risk. This is validation of the supplied mathematical objects, not certification of the historical parameter transcription.

## Input domain and conventions

Only `SimmVersion::V2_6` / `v2_6` is selectable. Required dimensions include 12 IR tenors, four IR currency groups, three FX categories, six FX pair categories, 17 commodity buckets, six risk classes and 13 qualifying-credit sectors including residual.

## Ordered algorithm

1. Select a registry record by matching its parsed version. The version comes from the first ID; aliases store clones. Reject missing versions.
2. Require positive MPOR, complete strictly positive IR weight/group-threshold maps, FX thresholds, commodity weights/thresholds, HVRs and high-volatility FX weight. Require complete positive generic delta/vega concentration maps.
3. Check nonnegative ordinary weights and finite nonnegative CQ weights and CQ concentration thresholds. Require each IR tenor pair and every CQ sector pair. Scalar correlation checks cover FX, CQ, subcurve, same-issuer and residual controls.
4. Build symmetric matrices with unit diagonal for six risk classes, IR tenors and CQ sectors; delegate diagonal/range/symmetry/PSD validation to core. Missing cross-risk-class pairs use 1.0; missing CQ correlations would use 0.0 but required-pair checks precede use. Validate the explicit 17×17 commodity matrix, including its 289-entry shape.
5. Cache the IR tenor matrix. An unexpected post-validation missing pair logs an error and uses 0.5. Other defensive lookups have full-correlation, zero-weight or zero-correlation fallbacks, normally blocked by construction checks.

## Selection, controls and limitations

The embedded data provide all expected pairs. No nearest-correlation repair or eigenvalue clipping is performed here. The imported quadratic kernels floor negative accumulated variance at zero; that is not a substitute for parameter validation. Direct `with_mpor(0)` is accepted by the setter but rejected at calculation.

The admitted CQ concentration-threshold domain includes zero despite later division by that threshold. IR intercurrency correlation and concentration-adjusted within-bucket matrices are outside these validators’ range/PSD checks.

## Alternatives and tradeoffs

Strict rejection of every missing pair and strictly positive denominator would give a narrower contract than the current fallbacks. A documented repair algorithm could accept noisy estimated matrices, but changes economic dependence. Versioning each complete parameter set by content hash would distinguish a custom override from the shipped historical model.

## Source and validation evidence

Inspected registry tests reject missing IR pairs, missing CQ tables, out-of-range correlations and non-PSD cross-class/CQ/commodity matrices. `simm_schedule_parity.rs` contains independent-literal transcriptions with local PDF provenance; this audit did not retrieve the PDF or execute those checks.

Implementation and data: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/registry/validation.rs](../../../../finstack-quant/margin/src/registry/validation.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json), [tests/fixtures/simm_v26_provenance.md](../../../../finstack-quant/margin/tests/fixtures/simm_v26_provenance.md).

Existing evidence: [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
