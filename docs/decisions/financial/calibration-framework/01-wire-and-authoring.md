# CALIBRATION-FRAMEWORK-001: Strict envelopes and authoring defaults

- Decision status: **Accepted 2026-11-15 — complete calibration envelope, authoring, restoration and publication package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent wire and authoring examples checked.** Production schema, admission, numerical and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for strict calibration envelopes and explicit authoring

1. **Use one strict Rust admission path.** Require the exact
   `finstack_quant.calibration/1` runtime marker. Use optional
   `$schema` as editor metadata. Apply the accepted byte, depth and
   collection limits to the complete request or result, including
   nested `final_market` with its ancestor depth. Reject duplicate
   decoded object names and invalid numeric representations before
   conversion to a map. Validate the closed typed envelope and each
   flattened `StepParams` variant. Remove the calibration depth
   bypass.
   [Accepted contract loading](../core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) ·
   [RFC 8259 object names](https://www.rfc-editor.org/rfc/rfc8259#section-4).

2. **Distinguish draft diagnostics from executable admission.** A named
   solver-free dry run can return the typed draft with findings. Strict
   loading and execution require semantic success before a plan is usable.
   Validate all supplied typed inputs, including unused quote payloads.
   Require exact references and preserve quote-set order. Reject duplicate
   quote membership within one set. Different sets may explicitly reuse
   a quote. Preserve fatal status independently of retained diagnostics,
   including under a zero retention cap. [Accepted record 002](02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market)
   governs numerical policy and initial context.
   [Accepted record 003](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution)
   governs dependency ordering and execution.

3. **Require one authority for each identity.** Require exact nonblank
   plan and step IDs, quote-set bindings and author-selectable output IDs.
   Reject `kind`, `id` and `quote_set` in a parameter map when supplied
   separately, even if their values agree. Remove fallback IDs derived
   from step ID and reject required nulls. Retain fixed output keys
   defined by the target contract and resolve their exact identities
   before execution. Preserve typed source identity, including Price
   versus VolIndex.
   [Accepted typed market state](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

4. **Require material financial choices at authoring.** Require explicit
   futures convexity adjustment, including literal zero when intended,
   option side, swaption convention, cap/floor side, hazard seniority
   and parametric family where consumed. Use decimal rates and volatility,
   bp CDS coupons and spreads, and upfront fractions of the stipulated
   notional. A deliberately selected owner-approved versioned preset may
   expand into explicit persisted values. Bootstrap, interpolation,
   extrapolation, weights, solver controls and model parameters have
   their own target and controls records. The observed settings below
   retain their evidence role.

5. **Validate result restoration and its declared status.** Check the
   nested market and report through Rust. Validate identities, references,
   units, field invariants and consistency with the outcome. Prefix nested
   diagnostics at `/result/final_market` with escaped JSON Pointer
   segments. A valid failed run can be restored as diagnostic evidence.
   Successful fit and replay authority require their owner checks.
   [Record 014](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics)
   governs financial fit and report consistency.
   [JSON Pointer escaping](https://www.rfc-editor.org/rfc/rfc6901#section-3).

6. **Use the Rust-owned contract for schemas, examples and hashes.**
   Follow the accepted Rust-derived offline schema publication and
   require owner admission of final examples. Apply the same checks
   across Rust, Python, WASM and persistence. Use the accepted canonical
   content hash on admitted typed payloads, preserving serialized
   metadata and array order. Keep hash version, artifact version and
   fit status explicit.
   [Accepted canonical wire](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity) ·
   [Accepted schema publication](../core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication).

## Independent calculation checks

- JSON keys `"sofr"` and `"\u0073ofr"` decode to the same name.
  Ordinary map parsing retains the second value and erases the duplicate.
  Strict admission rejects the repeated decoded name.
- Equal-weight observations 2% and 4% fit a constant of 3%. Repeating
  the 2% observation changes the fit to 2.6667%.
- Futures price 98.50 gives 1.50% at zero convexity adjustment.
  An explicit positive 2.5 bp adjustment gives 1.475%.

## Observed choice and rationale

The request is a typed, versioned envelope rather than an executable untyped map. `CalibrationStep::from_wire_fields` fills missing or null output IDs from the step ID for six curve and three surface kinds, defaults `quote_set` to the step ID, chooses senior hazard debt and the NS parametric variant, then deserializes the typed step. It overwrites caller map fields `kind`, `id`, and `quote_set`. This centralizes host-language authoring; historical adoption rationale is not established.

## Input domain and ordered behavior

1. Parse under `LoadLimits`, require the current schema marker, reject malformed/version-incompatible shapes and unknown fields at typed structures. Flattened steps explicitly deserialize their remaining fields as `StepParams`.
2. `parse_envelope` performs shape loading; `validate` separately assembles semantic errors. Request `from_slice_strict` returns `(envelope, report)` even when that semantic report contains errors. Execution explicitly rejects the first semantic error. `dry_run` is a static dependency/quote check and does not solve, build the context, or run full numeric configuration/preflight checks.
3. Quote constructors build and validate typed rate/CDS/inflation/volatility/XCCY/tranche quotes. Futures convexity defaults to zero; option type defaults to call, swaption convention to USD, cap/floor side to cap. Rates and volatility are decimal inputs, CDS coupons/spreads are bp, and CDS/tranche `upfront_pct` is a fraction despite its name.
4. Step defaults include discount bootstrap/log-linear/flat-forward; forward global solve with finite differences and cubic Hermite; hazard bootstrap/linear par interpolation; inflation bootstrap/linear; SABR beta 0.5; Student-t initial df 5, bounds [2.1,50], correlation 0.3; normal/lognormal conventions remain explicit where required. Surface extrapolation defaults to Error, missing SABR bucket fallback false, HW cap volatility scalar. Detailed target methods are in [calibration targets](../calibration-targets/README.md).
5. Result loading checks the nested final market using the market-context strict loader, prefixes its diagnostics, and applies depth limits separately to outer and nested structures. Content hashes use canonical serialization. Hand-written schema catalogues derive artifacts and examples; they are not generated runtime source.

## Limits and alternatives

A well-shaped result can carry inconsistent report numbers: result loading restores market state but does not rerun calibration or independently validate the report's financial acceptance. A successful request parse does not imply semantic validity; consumers must honor the returned report. Quote constructors do not fill missing financial conventions beyond the named defaults.

Alternatives are eager rejection of semantic errors at every entry point or a distinct validated-envelope type. Both would narrow the opportunity to ignore a report but change the authoring contract. No compatibility behavior is proposed.

## Source and validation evidence

- Implementation: [api/schema.rs](../../../../finstack-quant/calibration/src/api/schema.rs) — shape/version checks, semantic-report handoff, step defaults and nested-market restoration.
- Implementation: [api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — validated quote construction and map-default precedence.
- Implementation: [api/validate.rs](../../../../finstack-quant/calibration/src/api/validate.rs) — parse/dry-run distinction.
- Implementation: [api/market_datum.rs](../../../../finstack-quant/calibration/src/api/market_datum.rs) — quote versus context datum classification.
- Implementation: [api/prior_market.rs](../../../../finstack-quant/calibration/src/api/prior_market.rs) — typed prior-object variants.
- Implementation: [json_schema.rs](../../../../finstack-quant/calibration/src/json_schema.rs) — hand-written artifact catalogue and examples.
- Test evidence: [src/api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — constructor/default/unknown-field regressions; inspected or located, not executed.
- Test evidence: [tests/calibration/schema_parity.rs](../../../../finstack-quant/calibration/tests/calibration/schema_parity.rs) — schema-surface evidence located; inspected or located, not executed.
- Test evidence: [tests/calibration/serialization.rs](../../../../finstack-quant/calibration/tests/calibration/serialization.rs) — wire roundtrip evidence located; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
