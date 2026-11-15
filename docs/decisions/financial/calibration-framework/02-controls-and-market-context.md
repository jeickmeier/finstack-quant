# CALIBRATION-FRAMEWORK-002: Numerical policy, freshness and initial context

- Decision status: **Accepted 2026-11-15 — complete controls, freshness, quote-side and initial-market package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent solver/fit, freshness and initial-market examples checked.** Production calibration, market state and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for calibration controls, freshness and initial market

1. **Require effective settings for the selected solver and target.**
   Supply applicable controls explicitly or elect an approved versioned
   preset expanded into persisted values. Give scalar roots separate
   coordinate tolerance and work budget. Give least squares its own
   residual tolerance, parameter-step tolerance and work budget.
   Require finite positive tolerances and checked work limits. Bind
   scan, seed, weight and Jacobian choices to the actual methods that
   consume them. Remove shared numerical defaults and hidden LM step
   tolerance. Detailed algorithms remain with the solver and target
   owners.
   [Accepted Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search) ·
   [Accepted sequential bootstrap](10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing) ·
   [Accepted least squares](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes).

2. **Keep search, fit acceptance and post-fit validation distinct.**
   Require finite ordered search bounds on the actual target coordinate.
   Retire `AutoCurrency` and its fallback bands. A negative search bound
   does not select admissible discount-curve shape. Check the returned
   fit against the target's financial residual in its stated units and
   scale. Remove blanket numerical ordering of solver and fit tolerances.
   Apply the independently selected post-fit policy before publication.
   An expressly elected exploratory run may retain failed-fit evidence.
   A failed fit cannot supply installed dependencies or a usable final
   market.
   [Accepted curve validation](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

3. **Validate resolved configuration and overrides atomically in Rust.**
   Merge ordinary objects and replace arrays and scalars. Require
   complete replacement for a changed tagged variant. Admit `null`
   only under the field's optional contract. Reject unknown or
   inapplicable overrides. Preserve configured diagnostic and
   explanation caps while capture is disabled. Revalidate the entire
   effective configuration and persist its provenance before execution.
   Serial or parallel execution and optional capture leave the required
   acceptance checks in force.

4. **Use exact caller-supplied freshness instants.** Elect unverified
   freshness or require capture and assessment timestamps and a positive
   whole-seconds age limit. Admit the supported RFC 3339 profile with
   explicit offset and exactly representable fractional seconds.
   Compare exact nonnegative elapsed time with the inclusive limit
   using checked wide arithmetic. Reject incomplete assertions,
   unsupported timestamp forms, future captures and stale quotes.
   Retain supplied timestamps, applied limit and qualified result.
   Passing means the supplied quote snapshot met the declared policy.
   Source authentication and fixing publication retain their own checks.
   [RFC 3339 timestamp syntax](https://datatracker.ietf.org/doc/html/rfc3339#section-5.6).

5. **Treat quote side as optional supplied metadata.** Record Mid,
   Bid or Ask only where explicitly supplied and applicable. Remove
   implicit Mid. Freshness can pass with side unknown. A side label
   does not select from pairs, average prices or synthesize a mark.

6. **Publish only a fully validated initial market.** Assemble one
   private candidate. Admit prior objects and direct data by actual
   typed storage key. Reject duplicate keys and prior/data collisions
   while allowing shared text in unrelated namespaces. Keep Price
   and VolIndex kinds explicit and retire obsolete standalone
   basis-spread inputs. Use an accepted explicit same-kind operation
   for intentional replacement. Build FX from qualified directed
   quotes using explicit pivot or direct/reciprocal-only policy.
   Reject duplicate directed pairs even under distinct datum IDs.
   Resolve credit-index references after their sources exist, then
   attach the validated ordered typed hierarchy as structural state.
   Actual hierarchy selection requires exact active bindings.
   Calibration instrument quotes stay in quote sets. Share the Rust
   validation, effective controls and provenance with Python, WASM
   and stored results.
   [Accepted context lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle) ·
   [Accepted hierarchy](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap) ·
   [Accepted FX policy](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

## Independent calculation checks

- A parameter error of `5e−7` at slope 0.002 gives residual `1e−9`,
  within a `1e−8` fit tolerance. At slope 1,000 a smaller parameter
  error of `5e−11` gives residual `5e−8`, outside the same gate.
- Capture `2026-01-01T00:00:00+01:00` and assessment
  `2025-12-31T23:01:00Z` differ by exactly 60 seconds. The inclusive
  limit of 60 passes. One further millisecond exceeds it.
- One-year factors 0.95 and 0.97 under one discount key would change
  a 100 payment's PV from 95 to 97 under last-wins insertion.
  The duplicate fails admission.

## Observed choice and rationale

A shared `CalibrationConfig` controls solver accuracy, acceptance, validation and execution. Its role is to reject inconsistent controls before solving and to materialize an explicit market snapshot. This is observed behavior, not evidence that the chosen numerical limits have been approved for every market.

## Input domain, controls and stages

The solver requires finite positive tolerance and at least one iteration; defaults are 1e-12 and 100. Acceptance tolerances must be finite positive and at least the solver tolerance: default 1e-8 for discount, forward, hazard and inflation, 1e-3 for volatility. Discount-control fields default to `scan_grid_points=48`, minimum scan count 20, `scan_grid_step=1e-4`, DF bounds [1e-12,1e6], bootstrap seeding enabled and Jacobian step 1e-6. The [generic fallback scan](10-sequential-bootstrap.md) interprets the count per side and uses the step; the [discount target](../calibration-targets/03-discount-calibration.md) instead uses a maturity-aware custom grid with fixed log offsets, so those fields do not imply an exponential per-side discount scan. Hazard bounds default [0,10]; CPI [1,10000]. Weighting defaults to square-root time, with equal/linear-time/inverse-duration variants implemented by the targets. Counts are checked for allocation overflow. Recursive JSON override merges objects, replaces other values, preserves disabled explain settings, rejects unknown fields, then revalidates.

The default rate-bound policy is AutoCurrency. Currency rate bounds default to EUR/JPY/CHF [-0.05,0.30], USD/CAD/AUD/NZD [-0.02,0.50], GBP [-0.03,0.50], TRY/ARS/BRL/ZAR/MXN [-0.05,2.00], others [-0.02,0.50]. `with_rate_bounds` selects Explicit. These decimal annual-rate search bounds and post-fit `ValidationConfig` are separate. Serial execution, `ValidationMode::Error`, `fail_on_bad_fit=true`, disabled diagnostics/explanations, and no hierarchy are defaults.

Initial context insertion proceeds in order: prior curves/surfaces; nonquote market scalars, dividend schedules, fixing and inflation series, FX delta surfaces and volatility cubes; the FX matrix from explicit quotes; credit-index states; optional hierarchy. Credit-index insertion is delayed until dependencies exist. Calibration quotes never become initial curves. Price versus volatility-index prior tags must agree with their actual `PriceCurveKind`. Duplicate prior objects are not separately rejected by envelope uniqueness checks; insertion semantics govern replacement.

Freshness may be absent, producing `unverified` metadata. If enabled, snapshot time, assessment time and positive max age must all be supplied; timestamps parse as RFC3339, future snapshots fail, and age uses whole seconds. No wall-clock lookup occurs. Quote side Mid/Bid/Ask is metadata over a single-side quote schema, not selection from bid/ask pairs. FX uses a quote-only provider and the configured `FxConfig`.

## Limits and alternatives

Freshness `verified` means the supplied timestamps satisfy the supplied policy; it does not authenticate collection time or provider. The `u64` age limit is cast to `i64` for comparison without an explicit upper-bound guard; extremely large values can become negative and reject valid ages.

Per-instrument calibrated limits would offer more precision than one global tolerance; market-regime-specific bounds would replace static currency heuristics. Both require explicit policy and independent stressed-market evidence.

## Source and validation evidence

- Implementation: [config.rs](../../../../finstack-quant/calibration/src/config.rs) — configuration defaults, range validation, overrides, freshness and currency policy.
- Implementation: [solver/config.rs](../../../../finstack-quant/calibration/src/solver/config.rs) — validated solver wire boundary.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — independent post-fit policy.
- Implementation: [api/context_builder.rs](../../../../finstack-quant/calibration/src/api/context_builder.rs) — ordered snapshot materialization.
- Test evidence: [src/config.rs](../../../../finstack-quant/calibration/src/config.rs) — invalid controls and freshness regressions; inspected or located, not executed.
- Test evidence: [src/solver/config.rs](../../../../finstack-quant/calibration/src/solver/config.rs) — deserialization validation; inspected or located, not executed.
- Test evidence: [src/api/context_builder.rs](../../../../finstack-quant/calibration/src/api/context_builder.rs) — FX/context materialization regressions; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
requires typed source identity and one validated final context candidate. Prior/
market-data replacements must be intentional same-kind actions rather than
last-wins duplicate ingestion; reject preserved duplicate keys and embedded-ID
mismatches before map construction. Restore registered composites after sources
through the existing canonical state owner, preserving actual composition and
role metadata. This record specifies solver, freshness and initial
market policy for calibration.

## Relationship to the accepted typed hierarchy package

[CORE-MARKET-21's accepted package](../core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap)
was accepted on **2026-11-08**. Calibration transports canonical typed hierarchy associations and
ordered name/node sequences through the shared validated context/state path;
attachment remains structural metadata, without requiring all associations to
exist in the current calibrated market. Actual active financial selection uses
strict typed binding. The accepted clauses above apply to the
calibration boundary.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
