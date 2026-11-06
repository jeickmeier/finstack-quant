# FEATURES-002: Operation selection, strict parameters, missing values and failure policies

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and actual selection

The three operation enums select 23 time-series, 16 cross-sectional and three pairwise operations. `FromStr` deserializes their canonical snake_case serde names; `ALL`, `names` and `param_keys` describe that surface. Public entry points receive typed selectors. Parameters remain optional JSON values, parsed and validated before any entity or partition is evaluated, including empty input.

## Ordered rules

1. Check aligned lengths, reject a supplied parameter value unless it is an object, and reject every key outside the selected operation's allowlist. `None` means absent parameters; a direct `Some(Value::Null)` is not an empty object. Panel serde turns its optional `params: null` into `None`.
2. Parse integer controls as positive JSON unsigned integers representable as target `usize`. Zero, negatives, fractions and oversized values fail. Floating controls must be finite numbers; booleans must be actual JSON booleans. Missing keys use operation-specific defaults; a present null value fails the scalar parser.
3. `window` defaults to 1 and `min_periods` defaults to `window`; both are positive and `min_periods <= window`. Algorithmic sample floors may exceed the requested window and then produce all-missing outputs, without rejecting that otherwise valid request. Required `span` and `half_life` have no default.
4. `finite` maps `None`, NaN and either infinity to missing and canonicalizes either signed zero to positive zero. Algorithms exclude these inputs according to their own window/current-row policies. Thus nonfinite *input* and nonfinite *computed result* are treated differently.
5. Every public transform checks emitted `Some` values for finiteness. The first nonfinite output in original row order gives `Validation("non-finite feature result at row ...")`; earlier eager/domain/solver checks can fail first. No partial result is returned. Internal overflow that becomes a finite value later is not detected by this final scan.

Cross-sectional `fill_missing` emits the finite input or finite `value` (default 0). `is_finite` emits 1 for finite input, otherwise 0; `nan_mask` is its complement and covers None and infinities as well as NaN. These explicit selections fill all rows, unlike ordinary transforms. All three still require aligned partition keys and reject unused parameters.

## Rationale, alternatives and limits

Source comments explain strict parameters as protection against misspelled controls and output checks as preventing serialization from silently converting failures into missing values. The feature-specific exclusion of nonfinite inputs and zero-valued degeneracy conventions remain Draft. [ADR 0009](../conventions/0009-missing-and-invalid-data.md) requires explicit policies; its contiguous performance-panel rules are scoped to analytics and must not be silently imported here. Error, missing and zero are distinct outputs even where their numeric downstream effects may coincide.

Alternatives include rejecting nonfinite input, preserving its cause, or partition-local failure reporting. None is selected. Target `usize` width means the accepted maximum integer is not inherently cross-target identical. No general unit/rate/frequency validation exists because values are untyped f64 columns.

## Evidence

- Owning [parameter/missing/output helpers](../../../../finstack-quant/features/src/types.rs): `reject_unknown_params`, scalar parsers, `window_params`, `finite`, `validate_output`; enum and dispatch controls in [time-series](../../../../finstack-quant/features/src/timeseries.rs), [cross-sectional](../../../../finstack-quant/features/src/cross_sectional.rs) and [multi-input](../../../../finstack-quant/features/src/multi.rs).
- Inspected `invalid_parameters_fail_before_any_window_is_ready`, `signed_zero_never_creates_a_rank_signal`, `finite_results_survive_json_and_overflow_is_an_error` in [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs). The last covers an overflowing rolling sum, not every internal intermediate.
- Inspected `transform_cross_sectional_supports_weight_and_missing_ops`, typed dispatch and removed-alias cases in [transform tests](../../../../finstack-quant/features/tests/transforms.rs).

Evidence inspected here does not establish every host error mapping.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
