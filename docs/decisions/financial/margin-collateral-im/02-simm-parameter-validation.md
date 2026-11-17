# MARGIN-COLLATERAL-IM-002 — SIMM parameter completeness and correlation validation

**Decision status:** Accepted 2026-11-16 for checked SIMM parameter profiles and correlation validation.

**Implementation status:** Existing behavior is described below. Registry, calculator, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Exact arithmetic confirmed positive definiteness of the four bundled fixed correlation matrices. Historical primary-source checks matched the qualifying-credit weights, concentration thresholds and non-residual pairs and the commodity inter-bucket table. Runtime, portfolio and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked SIMM parameter profiles and correlation validation

1. Bind one supported methodology edition and exact profile ID in the [immutable snapshot](01-registry-and-defaults.md#accepted-decision-for-explicit-margin-parameter-snapshots-and-contractual-terms). Require complete canonical tables for the supported dimensions, including all six risk classes, 12 IR tenors across three weight sets, four IR concentration groups, three FX delta categories, six FX vega category pairs, 17 commodity buckets and 13 qualifying-credit weight and threshold entries. Reject missing, extra and duplicate keys. Remove the redundant `other` commodity parameter key for bucket 16. Cache checked matrices in fixed semantic order after validation.
2. Require finite strictly positive concentration denominators and historical volatility ratios and a positive integer reference margin period of risk (MPOR). Require strictly positive IR and commodity delta weights and high-volatility FX delta weight. Admit explicit zero for other ordinary risk multipliers where the profile supports it. Preserve basis-point weights for rates and credit delta, percentage-point weights for equity, commodity and FX delta, and USD concentration denominators on the matching sensitivity basis. Keep the source profile's reference MPOR immutable. Check any separately elected target horizon as a calculation input under [record 011](11-simm-total-and-mpor.md).
3. Require 15 cross-class pairs, 66 IR tenor pairs, 66 non-residual qualifying-credit pairs and the full 17×17 commodity matrix. Qualifying-credit residual retains its own weight, threshold and within-bucket parameters. Remove the 12 unused residual cross-pairs from the required table. Preserve the published zero cross-links for commodity bucket 16. Missing required coefficients produce typed errors. [ISDA SIMM v2.6, §§E.2 and H.2](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
4. Require finite correlations in `[-1, 1]`, exact symmetry, exact unit diagonals and the read-only numerical PSD gate in the [accepted core correlation contract](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission). Use a fixed versioned pivot tolerance of `1e-12` in core's normalized units. Report this as numerical admission. Preserve the selected coefficients and reject failed matrices without repair, clipping or coefficient substitution.
5. Check every correlation scalar, including IR intercurrency correlation. For generated currency, issuer and concentration-adjusted blocks, require a structural PSD guarantee or checked admission of the effective matrix on the active labels. Retain valid signed coefficients, including historical FX high/high `−0.05`. Apply stable checked arithmetic and fail on a negative or nonfinite variance. The risk-class records own their formula-specific caps and charge floors. Three equally concentrated IR currencies with unit weighted exposures and common pair correlation `−0.75` give `3 + 6 × (−0.75) = −1.5`, even though every coefficient is within bounds. [ISDA SIMM v2.6, §§B and I](https://www.isda.org/a/b4ugE/ISDA-SIMM_v2.6_PUBLIC.pdf).
6. Carry the validated Rust profile through portfolio and the existing Python and WASM paths. Report typed failures with the profile and offending field, pair or calculation stage. Preserve the historical USD v2.6 approximation label and its coverage limits, including omitted product-class separation and incomplete factor dimensions. Keep validation of selected parameters distinct from portfolio output and host qualification.

## Choice and rationale

Observed: constructors validate the parameter set before caching an IR tenor matrix. Missing or invalid parameters can still change the computed dependence and margin through unchecked or fallback paths.

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

The accepted validation preserves the selected calibration. Numerical matrix repair changes effective dependence and has a separate [core decision](../core-numerics/nearest-correlation.md). Record 001 establishes the effective parameter identity used for replay.

## Source and validation evidence

Inspected registry tests reject missing IR pairs, missing CQ tables, out-of-range correlations and non-PSD cross-class, CQ and commodity matrices. `simm_schedule_parity.rs` contains independent literal transcriptions with recorded PDF provenance. Direct decimal comparisons matched all 13 CQ bucket weights, all 13 CQ delta thresholds, all 66 consumed CQ pairs and the 272 off-diagonal commodity cells with the historical publication. The 17 unit commodity diagonals follow the correlation convention. An independent exact-rational factorization found positive pivots for all four bundled fixed matrices. These checks cover the named data and mathematical properties. Production tests and host suites remain unrun for this decision review.

Implementation and data: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/registry/validation.rs](../../../../finstack-quant/margin/src/registry/validation.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs), [data/margin/simm.v1.json](../../../../finstack-quant/margin/data/margin/simm.v1.json), [tests/fixtures/simm_v26_provenance.md](../../../../finstack-quant/margin/tests/fixtures/simm_v26_provenance.md).

Existing evidence: [tests/simm_schedule_parity.rs](../../../../finstack-quant/margin/tests/simm_schedule_parity.rs), [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/calculators/im/simm.rs](../../../../finstack-quant/margin/src/calculators/im/simm.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
