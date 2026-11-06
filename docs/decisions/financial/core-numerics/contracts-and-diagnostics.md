# CORE-NUMERICS-25: Bounded contract loading, validation and deterministic diagnostics

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`contract/*`, `validation.rs`, `serde_guard.rs`, `table.rs`, `error/*`, `explain.rs`, `parallel.rs` and lock recovery in `lib.rs`: validation order, limits, lookup diagnostics, trace caps and ordered execution. These rules can change whether a financial calculation runs or how failure is reported; they do not supply model assumptions.

## Choice and rationale

Strict versioned loading rejects missing/unsupported versions. Resource limits bound initial parsing and retained diagnostics. Generic validation functions expose explicit financial domains rather than applying blanket silent correction. Ordered parallel collection preserves input-result/error order across execution scheduling.

## Algorithm

1. Default load limits are 64 MiB bytes, depth 96, 100,000 artifacts, 1,000,000 positions, 256 diagnostics. Generic byte/depth precheck scans braces/brackets outside escaped JSON strings; it is not a JSON syntax parser and does not count artifacts/positions. Then serde_json parses, contract version is checked, then typed deserialization runs. Domain semantic validation must be called separately.
2. ContractDescriptor accepts exactly version 1 via `id/version` or `schema_version`; missing/invalid/unsupported versions become located diagnostics. Reports retain up to the cap, set truncated after overflow, and replace a retained warning with the first later error if necessary. A zero-cap truncated report conservatively counts as one error.
3. Generic helpers check finite/positive/nonnegative/unit-interval f64, strict/non-strict date ranges, currency equality/distinctness and amount greater than a supplied minimum. Recovery requires finite [0,1), explicitly excluding full recovery. Registry IDs must be nonempty and unique after trimming across all records; source/version metadata must be nonblank. These checks validate text presence, not provenance authority.
4. UnknownFieldGuard errors on the first leftover field. TableEnvelope construction sets row count from first column and rejects unequal lengths/duplicate names; direct derived deserialization and public mutation do not invoke that constructor.
5. Suggestions lowercase Unicode text, keep substring matches or Levenshtein distance≤2, stable-sort by distance and retain at most three. Distance uses insertion/deletion/substitution dynamic programming. Suggestions never change the requested ID. Error classification delegates through wrapped metric failures; missing resources map NotFound, solver/calibration/internal/cycle failures Computation, others Validation.
6. Explanation traces append up to a caller cap and then flag truncation; enabled convenience options default to 1,000 entries. Native ordered parallel map computes a Vec of per-item results with Rayon then serially returns the first input-order error; WASM serial collection stops when the first error occurs. Lock recovery keeps the contained value of a poisoned lock.

## Selection and controls

Load limits, trace cap and validation calls are explicit. Native/WASM execution is compile-target-selected. No financial domain validator is automatically applied by a generic schema parse, table constructor or result stamp.

## Limitations, failure and fallback

Artifact/position limits need higher-level enforcement. Table wire shape can bypass constructor invariants. Parallel native tasks can finish side effects after an earlier logical error, while serial execution stops earlier; same error order is not identical execution history. Poison recovery is a deliberate continuation policy, not proof of intact state.

## Alternatives and tradeoffs

Strict loading and bounded diagnostics are the implemented path. Automatic version migration, fuzzy ID substitution, and transactional parallel cancellation are absent. Limits trade diagnostic detail/resource consumption; they should not be mistaken for a financial validation certificate.

## Evidence

- [Contract loading](../../../../finstack-quant/core/src/contract/load.rs), [descriptor tests](../../../../finstack-quant/core/tests/contract/descriptor.rs), [diagnostic tests](../../../../finstack-quant/core/tests/contract/diagnostics.rs), [validation](../../../../finstack-quant/core/src/validation.rs), [table](../../../../finstack-quant/core/src/table.rs), [suggestions](../../../../finstack-quant/core/src/error/suggestions.rs), [parallel tests](../../../../finstack-quant/core/src/parallel.rs).
- `lexical_depth_scan_ignores_delimiters_inside_strings_and_escapes` checks parser precheck boundaries; `zero_diagnostic_limit_fails_closed_across_json_roundtrip` checks conservative failure; `several_failures_always_report_the_serial_first_error` checks ordered native error selection. These are deterministic contract checks. These do not demonstrate every financial object's constructor/serde invariant parity. [Wagner–Fischer](../../../REFERENCES.md#wagner-fischer-1974) supplies edit-distance context.

Truncated traces are not complete calculation evidence.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
