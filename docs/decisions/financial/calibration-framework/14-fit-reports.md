# CALIBRATION-FRAMEWORK-014: Residual acceptance, plan aggregation and report units

- Decision status: **Accepted 2026-11-15 — complete fit rows, financial acceptance, ratio summaries, typed outcomes, work and restoration package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent range, arithmetic and identity checks completed.** An isolated run of the live summary reproduced loss of the largest ratio. Production calibration, replay and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for fit acceptance and report semantics

1. **Make final fit rows authoritative.** Preserve one row for every
   consumed quote in authored order. Keep exact step and quote IDs
   as separate fields, with signed final residual, declared financial
   basis, units and effective tolerance. Preserve required quote
   coverage independently of optional diagnostic capture. Attach
   optional quote values and sensitivities by exact identity. Distinguish
   available finite values, unavailability and typed evaluation
   failures without synthetic zero or nonfinite placeholders.

2. **Apply the financial gate in native units.** Require complete
   coverage, finite residuals and the finite positive effective
   tolerance in each row's units. Pass only when every
   `|r_i| ≤ τ_i`, including equality and zero-weight rows. Retire
   magnitude-based penalty classification. Native max and RMS
   summarize comparable bases only, including when one step contains
   distinct residual bases such as projection-deposit growth and
   swap PV per notional.
   [Accepted deposit and FRA bases](04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks) ·
   [Accepted global candidate gates](12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

3. **Use tolerance ratios for plan-wide fit statistics.** Use
   `q_i = |r_i|/τ_i` and report the maximum and equal-per-quote
   `sqrt(Σ q_i²/n)` for the complete population. Retire pooled
   native max and RMS. Compute summaries with scaled arithmetic.
   Keep the exact worst row, with ties resolved in authored
   order. Incomplete populations or unrepresentable reported
   statistics have typed unavailability and coverage counts.
   Preserve finite native measurements and the applicable fit outcome.
   [Netlib scaled sum of squares](https://www.netlib.org/lapack/explore-html/d8/d76/group__lassq_gae8f40b0a34771b4f2d9c863de3af7be5.html).

4. **Derive success from applicable checked stages.** Keep numerical
   termination, native financial fit, required post-fit validation
   and complete output admission distinct. Report the actual stage,
   identities and typed cause on failure. Preserve advisory findings
   under the elected policy. Exploratory failure may retain bounded
   diagnostics. Only admitted outputs may serve dependencies,
   publish a calibrated market or qualify for replay. A deliberate
   context-only plan returns the validated initial context with
   no fit, zero solve work and absent fit statistics.
   [Accepted ordered execution and installation](03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution).

5. **Preserve objective, status and actual work.** Report method,
   stop reason and optimizer objective with its actual basis
   separately from financial fit. Count actual attempts, evaluations,
   reprices and optional diagnostic work, including all starts.
   Aggregate only like counters with checked arithmetic. Preserve
   authored step, start and event order. Apply trace caps independently
   of required outcomes and work evidence.
   [Accepted multi-start accounting](13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

6. **Use one Rust report validation contract.** Check report identities,
   coverage, units, tolerances, work, derived summaries and outcome
   consistency across Rust, Python, WASM and restoration. Use finite
   numbers and explicit absence on the wire. A restored diagnostic
   retains its recorded status. Replay requires the separate owner
   qualification.
   [Accepted result restoration](01-wire-and-authoring.md#accepted-decision-for-strict-calibration-envelopes-and-explicit-authoring) ·
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [JSON number rules](https://www.rfc-editor.org/rfc/rfc8259#section-6).

## Independent calculation and source checks

- For ratios `1,000,000` and `0.1`, the correct maximum is
  `1,000,000` and the equal-per-quote RMS is about `707,106.78119`.
  An isolated run of the live summary gave max and RMS `0.1`, with
  a separate worst-quote selector returning `1,000,000`.
- The distinct pairs `(a:b, c)` and `(a, b:c)` produce the same
  observed concatenated label `a:b:c:tolerance_ratio`. Separate
  identity fields keep both rows in the population.
- Scaled arithmetic returns RMS `1e308` for four `1e308` entries
  whose unscaled squares overflow. A finite residual of one divided
  by the smallest positive binary64 subnormal has no finite ratio.

## Observed choice and rationale at the audit revision

Step acceptance uses each observation's unweighted absolute residual, while solver objective and iteration counters retain method-specific meanings. The engine normalizes residuals by step tolerance to compare heterogeneous steps. The report schema also retains raw pooled summary fields, which require careful interpretation.

## Equations and stages

The generic residual summary treats nonfinite values or |r|≥5e5 as penalties. If any valid values exist, `max_residual` and RMSE use **only valid** values; otherwise they use all. The worst-quote selector independently ranks nonfinite values as infinite magnitude and otherwise absolute residual. Empty per-type residual maps fail. Nonempty maps fail if any penalty exists or max absolute residual exceeds tolerance; equality passes. `CalibrationReport::new` accepts caller-supplied success and sets objective to RMSE without independently overriding success.

Global fits overwrite objective with weighted L2, attach actual LM stats and units; bootstrap reports use objective-evaluation count as `iterations`, unlike LM iteration count. Final clamping and validation failures force success false. Warn-mode validation only adds metadata and leaves success/`validation_passed` intact. `quote_rows` prefers saved diagnostics rows, otherwise returns residual-only rows with missing quote value and NaN sensitivity.

For each plan step, require finite positive `success_tolerance`; construct residuals `a_i=|r_i|/tolerance_step`. Plan success is the conjunction of all step success and validation flags. Generic report construction computes ratio max/RMS, then `with_plan_aggregation` stores those in `max_residual_ratio`/`rmse_ratio` and replaces raw max/RMS with max of step maxima and quote-count-weighted RMS across step RMSEs. Objective remains the ratio RMSE and `residual_units` is absolute-residual-over-step-tolerance. Raw step residuals remain in `step_reports`. Traces merge in lexicographic step-ID order, not original plan order, and preserve truncation flags.

## Confirmed discrepancies and limits

The fixed 5e5 penalty threshold is reused on dimensionless ratios. In a permissive plan, raw residual 0.01 with tolerance 1e-8 produces ratio 1e6; if another ratio is 0.1, the report summary excludes 1e6 and reports max ratio 0.1. Plan success remains false from the step reports. The original audit derived the example from the aggregation equations. The later isolated check executed the live summary function.

Raw maxima/RMS pooled across quoted volatility, PV per unit notional and upfront-fraction steps are not one economic unit. When a step excludes penalties, pooling its valid-only RMSE using the full residual count also differs from pooling all valid raw observations. NaN-only maps can have finite zero maximum despite failure; fallback quote-row NaNs need host serialization care. Error strings claiming max residual exceeded tolerance can misdescribe a validation/clamping failure.

## Alternatives considered at the audit revision

Per-unit raw summaries or ratio-only plan summaries would be meaningful alternatives; choosing one is an API policy decision.

## Source and validation evidence

- Implementation: [report.rs](../../../../finstack-quant/calibration/src/report.rs) — penalty classification, max/RMS, quote rows, success and pooled summaries.
- Implementation: [api/engine.rs](../../../../finstack-quant/calibration/src/api/engine.rs) — tolerance ratios, conjunction, trace merging and fit error creation.
- Implementation: [validation/mod.rs](../../../../finstack-quant/calibration/src/validation/mod.rs) — Warn/Error attachment.
- Implementation: [versions.rs](../../../../finstack-quant/calibration/src/versions.rs) — human-readable model labels only.
- Test evidence: [src/report.rs](../../../../finstack-quant/calibration/src/report.rs) — threshold/nonfinite acceptance, RMS and optional-wire regressions; inspected or located, not executed.
- Test evidence: [src/api/engine.rs](../../../../finstack-quant/calibration/src/api/engine.rs) — plan residual aggregation tests located; inspected or located, not executed.
- Test evidence: [tests/calibration/tolerances.rs](../../../../finstack-quant/calibration/tests/calibration/tolerances.rs) — target acceptance tolerance tests located; inspected or located, not executed.
- Independent examples and the isolated source check cover the
  report arithmetic and row identities. Production plan execution,
  restoration and host qualification remain open.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
