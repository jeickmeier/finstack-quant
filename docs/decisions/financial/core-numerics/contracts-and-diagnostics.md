# CORE-NUMERICS-25: Bounded contract loading, validation and deterministic diagnostics

- Decision status: **Accepted — complete contract/validation/diagnostic package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded reference examples and read-only loader/state/table/trace/error/caller reviews are reported below; these do not certify production admission, scheduling or financial correctness.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`contract/*`, `validation.rs`, `serde_guard.rs`, `table.rs`, `error/*`, `explain.rs`, `parallel.rs` and lock recovery in `lib.rs`: validation order, limits, lookup diagnostics, trace caps and ordered execution. These rules can change whether a financial calculation runs or how failure is reported; they do not supply model assumptions.

## Accepted decision: bounded contract loading, validation and diagnostics

Accepted on **2026-11-08** after whole-record review. Inherit accepted
[012's exact monetary comparisons](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[023's producer/config authority](rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata),
[024's strict wire/lossless ingress](numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity),
[0025's evidence boundaries](../../project/0025-auditable-results-and-replay.md)
and [0049's explicit error classification](../../standards/0049-explicit-public-error-matching.md).
Runtime, existing caller/host/schema reconciliation and independent qualification
remain pending. The observed source behavior is recorded separately below.

1. **Use one coherent strict loading path through owner validation.** Validate
   controls; gate the complete encoded input; apply accepted024 lossless JSON
   admission before duplicate names/nonfinite/oversized integer tokens disappear;
   check the owner's required version marker; validate typed shape/wire/unknown
   fields; then run owner semantic/reference/build validation before publishing
   any usable artifact or mutating authoritative state. Generic decode remains
   explicitly structure-only working machinery, not a financial validation
   certificate. Preserve streaming/direct-typed paths where useful, with the
   same gates before collection allocation; do not require every large payload
   to clone a complete generic tree or add parallel parser APIs. A prebuilt Value
   can validate only its current tree, not original bytes or lost information.

2. **Retain explicit technical load defaults and a real depth ceiling.** Keep
   the small LoadLimits parameter value and defaults: 64 MiB encoded bytes,
   depth96, 100,000 artifacts, 1,000,000 positions per artifact and 256 retained
   diagnostics. These are operational profiles, not financial defaults or a
   total-memory/time guarantee. Validate controls at every consuming boundary;
   depth may be tightened in 0..=96, not relaxed past the accepted024 writer
   ceiling. Zero byte/cardinality/diagnostic limits are valid deny/retain-none
   policies; depth0 admits only scalar JSON roots before contract shape checks.
   Check counters/capacity arithmetic and do not trust collection length hints.
   No universal resource manager or unbounded sentinel value is selected.

3. **Bound the entire artifact and actual owner cardinalities.** The lexical
   depth pass remains a cheap precheck, not a syntax parser; root containers
   count1, scalar roots0 and ignored/extension/nested fields count too. Actual
   JSON parsing must reject malformed syntax. Remove calibration's parse-with-
   usize::MAX/clone-and-null-final_market depth exception: nested budgets cannot
   erase ancestor depth or bypass the outer byte/depth gate. Owners count every
   supplied artifact/position, including invalid/duplicate entries, before full
   collection allocation, resolution or build. Stop at the first limit+1;
   overflow is failure, not a saturated successful count. Generic load helpers
   must not claim to enforce domain counts they never inspect. Enclosing owners
   retain explicit aggregate/batch budgets beyond the per-artifact limits.

4. **Keep the existing descriptor explicitly v1-only and exact.** Correct its
   universal-version claim: ContractDescriptor governs only owners selecting
   this v1 contract, independently of RoundingContext2/canonicalc2/other artifact
   versions. Require a nonblank exact descriptor ID with no surrounding
   whitespace or slash. Require exactly `<id>/1`, rejecting /01, /+1, extra
   components and case/whitespace aliases; the numeric version form requires
   integer token1. Each owner selects schema or schema_version deliberately;
   there is no autodetection, alternate-marker fallback, missing-version default
   or migration. Validate a root object before marker lookup, locate wrong-type/
   missing/unsupported marker errors truthfully at /schema or /schema_version,
   and do not invent an actual numeric version for malformed text.

5. **Keep strict shape separate from duplicates and semantic validation.**
   Closed typed objects reject unknown fields. Retain UnknownFieldGuard as the
   final flattened guard where native deny_unknown_fields is incompatible;
   its first leftover field is an error and it writes no keys. Keys intentionally
   consumed by an opaque extension map remain with that owner's schema/content
   policy. This guard does not detect already overwritten duplicates or validate
   economic meaning. Reconcile actual serde behavior and generated additional-
   property rules rather than claiming a schema flag enforces every live path.

6. **Separate report fatal status from retained diagnostic detail.** Give each
   report private guarded state, one selected immutable retention cap, ordered
   diagnostics, monotone truncation and an explicit persisted has_errors fact.
   Update has_errors for every finding before applying the cap; retain the first
   N findings in encounter order without replacing warnings, sorting or dedup.
   With cap0, an omitted warning is nonfatal and an omitted error remains fatal.
   Persist required max_diagnostics, has_errors and truncated fields alongside
   diagnostics, using accepted024's checked count wire for the cap. Checked
   restoration requires the stored cap itself to fit the enclosing caller's
   bound, not merely its current list length; reject rather than rebind/trim.
   Reject oversized, unknown-field
   or inconsistent restored reports: a retained error requires has_errors=true;
   without truncation fatal status must equal the retained severities. A report-
   carrying failure must be fatal; warning-only reports are not failed loads.
   Existing merges OR child has_errors/truncated into the target before applying
   its fixed cap, even when the child retained no diagnostics; preserve ordered
   available detail and correctly escaped pointer prefixes. Never infer a child's
   fatal status from its retained list. Summaries distinguish retained error counts from omitted detail and never
   fabricate one error or imply a known total. Counts do not bound message bytes
   or validation work. No secondary report/diagnostic framework or old-wire alias.

7. **Make diagnostic location, context and determinism truthful.** Stable codes,
   phase, severity, owner/expected version and actual available context carry
   machine meaning; human dependency/OS message text need not be byte-stable.
   Preserve producer encounter order and array index order; semantic owners use
   deterministic traversal. Optional context stays absent when unknown. JSON
   Pointer uses RFC6901 string form: empty string is root, / is an empty-key
   member, ~0/~1 escape tilde/slash; reject malformed pointer syntax, do not
   concatenate raw identifiers or accept a URI fragment as a pointer. Syntax
   does not prove a target exists. Actual version0 may describe rejected input;
   an expected version must be positive. A supplied digest/revision label is not
   authenticated provenance or evidence that hashing/building succeeded.

8. **Retain small explicit generic predicates without financial inference.**
   Keep finite/positive/nonnegative/closed-unit f64, strict/non-strict date
   ordering, nonblank text, exact currency equality/distinctness and lazy
   require helpers. Reject invalid inputs without epsilon, correction, rounding,
   lookup substitution or convention defaults. Dates use their actual supported
   type and accepted024 boundaries. A helper validates only its stated predicate;
   the product chooses sign, clock, currencies and which bounds apply. Successful
   source/version text checks prove presence, not authority/currentness. User
   callbacks can fail/panic independently; no universal validation sandbox.

9. **Make Money threshold checks exact.** Change validate_money_gt's min from
   f64 to Decimal in the supplied Money's major currency units and compare its
   stored amount_decimal directly under accepted012. Equality fails the strict
   greater-than predicate; negative thresholds remain mathematically valid.
   Keep separate expected-currency checks where the owner requires them. Replace
   actual zero/other threshold callers with exact values; no compatibility float
   overload or amount() projection. A model view, display or rounded output
   cannot determine monetary admissibility or silently suppress small cash.

10. **Remove universal recovery-rate economics from generic validation.**
    Delete validate_recovery_rate and unsupported claims that full recovery is
    universally invalid/ISDA-forbidden. Existing closed-unit validation expresses
    a finite decimal recovery fraction [0,1]. Positive-LGD or spread-to-hazard
    inversion/calibration restrictions belong to the actual operation and need
    a local explicit R<1 check when mathematically required. Reconcile accepted
    hazard-source full-recovery admission; retain separately reviewed/unreviewed
    CDS/protection-leg/model constraints with their owners rather than widening
    every caller automatically. No recovery default, inferred LGD or new global
    product-domain validator is selected.

11. **Align registry admission, uniqueness, lookup and restoration.** Require
    nonempty alias lists; reject blank or surrounding-whitespace IDs instead of
    silently trimming comparison only. Compare exact case/Unicode strings and
    reject repeated aliases, including within one record, in the lookup namespace
    represented by each helper invocation. This is not global uniqueness across
    unrelated registries. Owners with accepted normalization apply it explicitly
    before both lookup and uniqueness. Resolve raw registry documents through
    their semantic builder on every restoration path; private fields with derived
    deserialization alone do not establish that. Preserve exact source/version
    labels without asserting authenticity. Fixed builders and retired generic
    overrides remain inherited023 decisions, not reopened choices.

12. **Keep one validated immutable table envelope and guard actual exports.**
    Retain unique exact nonblank column names, producer column/row order, equal
    lengths, matching row_count, zero rows for no columns and valid named empty
    columns. Names with surrounding whitespace remain literal if nonblank;
    table labels are not registry identifiers. Derive row count at construction,
    require restored row_count agreement and reject unknown structural fields.
    Make state private with immutable get_* views/checked whole-value creation;
    no mutable aliases or row-count/column patches that create partial states.
    Preserve current validate-on-serialization/restoration, and validate actual
    Arrow/Python/pandas/WASM export entry points. Roles/opaque metadata are hints,
    not financial units, row keys, uniqueness or accounting certification.

13. **Preserve missing, undefined and exact integer table values.** Keep Float64
    explicit finite/nonfinite result storage; NullableFloat64 must distinguish
    None→null from Some(NaN)→"nan" and Some(±infinity)→their accepted024 tokens.
    Reuse the existing Rust sentinel authority in the optional sequence path;
    never reinterpret string columns. Actual product owners decide valid result
    availability and must not hide calculation errors in a table. Arrow retains
    validity separately from present NaN; disclose any pandas convenience view
    that collapses that distinction. Keep full i64/optional-i64 columns exact;
    native WASM exports use BigInt for all present Int64 values, while UInt32 and
    checked row counts use Number. JSON text keeps exact integer tokens; ordinary
    JS JSON.parse is not a lossless large-integer path. Reconcile declared-column
    sentinel revival and native-to-wire conversion, preserving null, signed zero
    and accepted024 finite numeric arms. No host arithmetic or widened core domain.

14. **Keep suggestions advisory, deterministic and bounded.** Exact requested
    IDs still control lookup and failure. Retain Unicode lowercase comparison,
    substring-or-Levenshtein≤2 eligibility and the existing two-row edit-distance
    method for hints only. Rank by (distance, original UTF-8 ID), deduplicate exact
    candidates and return at most3. Blank requests yield no hints. Select a fixed
    private 1,000,000-cell aggregate DP-work budget per lookup; if exceeded, omit
    all suggestions rather than returning an iteration-dependent partial top3 or
    changing NotFound. Input/iteration/string-size controls remain with owners;
    this cell budget is not a callback/allocation/time sandbox. No fuzzy automatic
    substitution, financial alias, configurable matching framework or new package.

15. **Preserve owned error meaning and bounded cause rendering.** Inherit
    accepted0049: explicitly classify every owned variant/nested variant in its
    owning Rust crate; remove catch-all assignment of Validation meaning. Exact
    misses remain NotFound, invalid inputs Validation, and post-admission
    numerical/nonconvergence/poison failures Computation through appropriate
    owned variants. Metric wrappers delegate to their cause without string
    parsing. Preserve stable codes/context and source causes in central host
    conversion. Bound format_chain to64 source links and16 KiB rendered UTF-8,
    marking omitted detail; cycles/deep chains cannot loop forever. Arbitrary
    Display/source callbacks themselves are not sandboxed. Host exceptions may
    carry bounded cause text rather than native exception chaining; document the
    actual mapping instead of claiming a richer chain. No duplicate host tables.

16. **Bind finite trace capacity once and make truncation monotone.** Default
    tracing remains disabled; enabled() selects the explicit technical default
    1,000 entries. Replace optional/unlimited max_entries by a checked integer
    cap in accepted024's checked count wire domain; wire options explicitly carry
    enabled and cap. Each trace has private
    validated type/cap/entries and required bool truncated state, with that cap
    fixed at creation/restoration. Remove per-push changing limits, public list
    mutation and unlimited None. Cap0 records no entries and marks any attempted
    omission; overflow makes truncation true permanently, including restored
    false and merged truncated children. Preserve ordered admitted entries and
    reject inconsistent/oversized/unknown-field restoration without trimming.
    All push/merge callers respect the enclosing cap. Avoid constructing unused
    detail when disabled/full; entry count is not a metadata-byte/work budget,
    and no unmeasured zero-overhead claim or complete-evidence claim survives.

17. **Keep trace amounts honest diagnostic projections.** CashflowPV retains
    finite f64 cashflow/PV views in stated major currency units with validated
    currency identities, nonblank curve IDs, a finite positive DF, supported date
    strings and an optional valid owner-qualified conditional survival probability.
    Validate entry construction/restoration/serialization; NaN/infinity cannot
    silently become null in these finite fields. Computation-step name/description
    are nonblank; optional metadata remains opaque admitted JSON. These f64
    projections do not reconstruct original Decimal cash, certify conservation
    or replace immutable owner evidence. Do not wrap already projected floats
    in Money and imply recovered precision. Credit PV can include recovery;
    impose no generic cashflow×DF×survival equality. Trace caps cannot suppress
    underlying computation failures, and trace capture is not replay proof.

18. **Retain ordered fallible mapping with explicit execution limits.** Native
    indexed Rayon mapping evaluates all items and collects Result values in
    input order before returning the lowest-index error; WASM serial mapping
    stops at that error. Successful output order is preserved; callbacks must
    be deterministic and independent of invocation order, shared mutation or
    shared RNG consumption. Owners select/freeze per-item randomness/context
    and validate numerical/financial results; the generic mapper invents none.
    No identical scheduling, callback-count-on-error, bitwise floating reduction,
    cancellation, side-effect rollback or panic-to-Result guarantee. Bound work
    and side effects with the caller; use staged owner publication when atomicity
    is required. No transactional parallelism or replacement scheduler framework.

19. **Remove blanket poisoned-lock recovery and fail closed.** Delete
    recover_lock/automatic PoisonError::into_inner continuation. Every current
    lock acquisition returns a Computation failure on poisoning, including FX
    quotes/pins/fixings and calendar intern/cache locks, without publishing a
    result from uncertain protected state. Propagate fallible errors through
    actual calendar/FX owners under their accepted API contracts. Do not clear
    authoritative quotes, substitute another provider, retry with an empty cache,
    or silently reuse a poisoned guard. Derived-state rebuild is not selected
    here; it would require an explicit owner invariant. A lock not being poisoned
    is not proof of financial consistency, and no ordinary partial-write defect
    is claimed merely from this panic-integrity concern.

20. **Reconcile the real caller/host boundaries and qualify them independently.**
    Update every strict byte/direct-typed/calibration/materialization path,
    registry restoration, actual artifact/position budget, exact threshold and
    recovery caller, table/Arrow/pandas/native host export, trace push/merge and
    lock consumer in the later slice. Python and WASM expose the complete same
    report (including fatal/truncated status), not Python's current diagnostics-
    only list. Update existing schemas/docs/declarations/parity together without
    new host classes, compatibility decoders, generic frameworks or binding math.
    Independently qualify stage ordering, lossless ingress, limits, pointers,
    fatal/retention state, nullable data, exact money/IDs, deterministic hints,
    trace merges, ordered failures and panic/poison boundaries. Reference examples,
    located tests and source review do not execute production behavior or certify
    financial suitability. This review changes no runtime, tests, schemas,
    fixtures, dependencies, generators or hosts.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Complete JSON depth96 /97, including nested final_market | Within depth ceiling / reject before tree construction; nested exception removed. |
| Braces inside an escaped JSON string | Not containers; full parser still validates syntax. |
| Duplicate root/extension key or oversized raw integer token | Reject under accepted024 before map overwrite/f64 promotion. |
| `<id>/1` / `<id>/01` / `<id>/+1` / numeric1.0 | Exact selected marker admitted / reject the other spellings. |
| Warning/error omitted at diagnostic cap0 | No fatal finding / fatal status true, with no fabricated retained error count. |
| Fatal cap0 child merged into a nonfatal report | Fatal status and truncation propagate despite the empty child diagnostic list. |
| Restored report claims a cap above the caller's limit | Reject even if its current diagnostic list is short; no silent cap rebinding. |
| Pointer empty / `/` / `/desk~1USD~0CSA` | Root / empty-key member / exact `desk/USD~CSA` member. |
| Exact Money1.0000000000000000000000000001 versus Decimal1 | Strictly greater; an f64 equality projection cannot reject it. |
| Recovery1 | Valid generic fraction; operation-specific positive-LGD rules remain with the owner. |
| Registry ID ` USD ` / table label ` USD ` | Reject surrounding-whitespace registry ID / preserve literal nonblank table label. |
| Nullable float None / Some(NaN) / Some(+infinity) | null / "nan" / "inf", without missing/undefined collapse. |
| Full-i64 table value exported natively to WASM | Exact BigInt, not rounded Number or a narrower shared integer domain. |
| Several equally distant hints with permuted source iteration | Same deduplicated top3; never change the exact requested ID. |
| Restored trace truncated=false then omitted entry | Truncated becomes true and stays true, including after merge. |
| Native later callback completes after earlier logical failure | Lowest-index error still returned; no side-effect rollback claim. |
| Poisoned authoritative FX lock or derived calendar cache lock | Computation failure; no blanket continuation/rebuild. |

## Observed choice and rationale

Strict versioned loading rejects missing/unsupported versions. Resource limits bound initial parsing and retained diagnostics. Generic validation functions expose explicit financial domains rather than applying blanket silent correction. Ordered parallel collection preserves input-result/error order across execution scheduling.

## Observed algorithm

1. Default load limits are 64 MiB bytes, depth 96, 100,000 artifacts, 1,000,000 positions, 256 diagnostics. Generic byte/depth precheck scans braces/brackets outside escaped JSON strings; it is not a JSON syntax parser and does not count artifacts/positions. Then serde_json parses, contract version is checked, then typed deserialization runs. Domain semantic validation must be called separately.
2. ContractDescriptor accepts exactly version 1 via `id/version` or `schema_version`; missing/invalid/unsupported versions become located diagnostics. Reports retain up to the cap, set truncated after overflow, and replace a retained warning with the first later error if necessary. A zero-cap truncated report conservatively counts as one error.
3. Generic helpers check finite/positive/nonnegative/unit-interval f64, strict/non-strict date ranges, currency equality/distinctness and amount greater than a supplied minimum. Recovery requires finite [0,1), explicitly excluding full recovery. Registry IDs must be nonempty and unique after trimming across all records; source/version metadata must be nonblank. These checks validate text presence, not provenance authority.
4. UnknownFieldGuard errors on the first leftover field. Current TableEnvelope construction, custom serialization and custom deserialization call validate: equal lengths/row_count, unique nonblank names and zero rows when no columns. The earlier draft's derived-deserialization bypass description is stale. Fields remain public, and actual Arrow/pandas export paths do not uniformly call core validation. Float64 uses sentinels; NullableFloat64 currently uses ordinary Option<f64> serde, which can erase present nonfinites into null.
5. Suggestions lowercase Unicode text, keep substring matches or Levenshtein distance≤2, stable-sort by distance and retain at most three. Distance uses insertion/deletion/substitution dynamic programming. Suggestions never change the requested ID. Error classification delegates through wrapped metric failures; missing resources map NotFound, solver/calibration/internal/cycle failures Computation, others Validation.
6. Explanation traces append up to a caller cap and then flag truncation; enabled convenience options default to 1,000 entries. Native ordered parallel map computes a Vec of per-item results with Rayon then serially returns the first input-order error; WASM serial collection stops when the first error occurs. Lock recovery keeps the contained value of a poisoned lock.

## Observed selection and controls

Load limits, trace cap and validation calls are explicit. Native/WASM execution is compile-target-selected. No financial domain validator is automatically applied by a generic schema parse, table constructor or result stamp.

## Observed limitations, failure and fallback

Artifact/position limits need higher-level enforcement. Generic Value parsing can erase duplicate names/large integers before typed checks. Calibration relaxes outer depth before cloning/nulling final_market. Current descriptor integer parsing admits /01 and /+1 aliases. Table public state can be invalid between guarded serde boundaries; nullable float JSON and some convenience exports lose result distinctions. validate_money_gt compares an f64 amount view with an f64 threshold. The shared recovery helper's [0,1) restriction is not universal financial authority. Trace caps can be unlimited/changed per push, and restored Some(false) truncation does not flip on overflow. Report cap0 can invent an error count for omitted warnings; Python drops report-level truncation when exposing only diagnostics. Parallel native tasks can finish side effects after an earlier logical error, while serial execution stops earlier; same error order is not identical execution history. Poison recovery exposes possibly inconsistent authoritative FX state as well as derived caches. Catch-all classification and unbounded error-source traversal need the separately accepted error contract and bounded rendering.

## Observed alternatives and tradeoffs

Strict loading and bounded diagnostics are the implemented path. Automatic version migration, fuzzy ID substitution, and transactional parallel cancellation are absent. Limits trade diagnostic detail/resource consumption; they should not be mistaken for a financial validation certificate.

## Evidence

- [Contract loading](../../../../finstack-quant/core/src/contract/load.rs), [descriptor tests](../../../../finstack-quant/core/tests/contract/descriptor.rs), [diagnostic tests](../../../../finstack-quant/core/tests/contract/diagnostics.rs), [validation](../../../../finstack-quant/core/src/validation.rs), [table](../../../../finstack-quant/core/src/table.rs), [suggestions](../../../../finstack-quant/core/src/error/suggestions.rs), [parallel tests](../../../../finstack-quant/core/src/parallel.rs).
- `lexical_depth_scan_ignores_delimiters_inside_strings_and_escapes` checks parser precheck boundaries; `zero_diagnostic_limit_fails_closed_across_json_roundtrip` checks conservative failure; `several_failures_always_report_the_serial_first_error` checks ordered native error selection. These are deterministic contract checks. These do not demonstrate every financial object's constructor/serde invariant parity. [Wagner–Fischer](../../../REFERENCES.md#wagner-fischer-1974) supplies edit-distance context.

Truncated traces are not complete calculation evidence.

- [Diagnostic state](../../../../finstack-quant/core/src/contract/diagnostics.rs),
  [descriptor](../../../../finstack-quant/core/src/contract/descriptor.rs),
  [limits](../../../../finstack-quant/core/src/contract/limits.rs),
  [unknown-field guard](../../../../finstack-quant/core/src/serde_guard.rs),
  [trace state](../../../../finstack-quant/core/src/explain.rs) and
  [poison helper](../../../../finstack-quant/core/src/lib.rs) establish actual
  admission/state boundaries. [Calibration nested loading](../../../../finstack-quant/calibration/src/api/schema.rs),
  [Arrow export](../../../../finstack-quant/arrow-interchange/src/lib.rs) and
  [Python errors](../../../../finstack-quant-py/src/errors.rs) identify specific
  reconciliation consumers. No production tests/builds were executed.
- [RFC6901](https://www.rfc-editor.org/rfc/rfc6901.html#section-3) defines pointer
  string syntax and escaping; it does not prove a referenced member exists.
  [Serde flatten documentation](https://serde.rs/attr-flatten.html) explains the
  deny_unknown_fields limitation, not financial validation. Official
  [Rust poisoning documentation](https://doc.rust-lang.org/std/sync/struct.Mutex.html#poisoning)
  and [PoisonError::into_inner](https://doc.rust-lang.org/std/sync/struct.PoisonError.html#method.into_inner)
  describe advisory poisoning and access to potentially uncertain data; the
  selected fail-closed policy is repository ownership policy. [Rayon indexed
  collection](https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html#method.collect_into_vec)
  supplies ordering context, not equal scheduling/side-effect guarantees.
  Existing accepted monetary/recovery/error decisions retain their own authority.
- Independent **2026-11-08** Python standard-library reference checks passed:
  an exact Decimal threshold counterexample lost by binary64 projection; parsed-
  tree depths0/1/95/96/97 and escaped string delimiters; six JSON Pointer
  escape roundtrips; **1,020** severity-sequence/retention-cap combinations;
  and **24** duplicate/tie hint-order permutations using a full-matrix distance
  calculation independent of the production two-row method. DP-work-limit
  arithmetic was also checked. These bounded examples establish accepted targets,
  not production parser/trace/lock/scheduler execution, performance budgets or
  financial correctness. Four read-only source/caller/reference reviews covered
  the complete reviewed package; their report-merge correction is included above.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
