# CALIBRATION-FRAMEWORK-022: Deal quote overrides and horizon hazard reconstruction

- Decision status: **Accepted 2026-11-15 — complete exact deal override, model-derived horizon refit and dated survival gate package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent maturity, quote-center, clock and survival-coverage checks verified.** Production replay, publication, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for deal overrides and horizon hazard replay

1. **Elect an exact deal par-spread scenario.** Select one bound
   par-risk observation by exact quote ID, verified recipe index or
   uniquely resolving contractual maturity. Match entity, currency,
   seniority, resolved CDS terms and the clean or dirty par basis.
   Use the literal last day of protection. Retire clean-display
   triggers, first-match selection and approximate time matching.
   [Accepted CDS risk boundary](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk) ·
   [ISDA standard contract dates](https://www.cdsmodel.com/assets/cds-model/docs/Standard%20CDS%20Contract%20Specification.pdf).

2. **Fit the overridden center after original-source qualification.**
   Qualify the untouched original source. Apply the finite admissible
   absolute replacement spread in bp to one temporary par-risk input
   and fit the explicit scenario center. Any subsequent spread moves
   use the same recovery, discount state and prepared CDS terms.
   Preserve original history and report original and replacement
   quotes and represented moves. Reject overrides on recovery,
   dependency-only and horizon actions.
   [Accepted hazard replay](21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).

3. **Elect a separate model-derived horizon calibration.** Require
   an explicit later valuation cut and qualify the original fit on
   its immutable source market. Bind the horizon discount and
   information state. Construct the conditional reference from the
   qualified original hazard, retaining its original base, clock,
   intensity segments and tail. For horizon `v` and date `T ≥ v`,
   use `S_v(T) = exp(−J(τ_B(v),τ_B(T)))` with the local integral
   on the original clock. Any supplied conditioned view must match
   that lineage. An explicit request elects requoting.
   [Accepted hazard conditioning](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation) ·
   [Accepted horizon provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted context lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

4. **Synthesize horizon quotes at surviving contractual maturities.**
   Report expired observations and require a nonempty set of live
   contracts. Keep each absolute contractual maturity and resolve
   new protection, accrual and settlement dates for the hypothetical
   horizon CDS. Compute par spreads on the declared basis with the
   conditioned hazard, exact horizon discount state and retained
   recovery. Refit the model-derived quotes at the horizon with the
   bound effective policy. Preserve per-observation lineage and
   reject invalid or unattainable quotes.
   [Accepted CDS materialization](07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement).

5. **Require a separate dated survival-equivalence gate.** Elect a
   finite end date, absolute integrated-hazard tolerance and work
   limit. Include all required pricing and requested-use dates in
   the window. Compare each civil date within it using each
   evaluator's actual clock and local integral from the horizon.
   Require complete coverage and report the maximum error, its date
   and the tested window. The gate covers those dates. Continuous
   and out-of-window claims require separate evidence.

6. **Publish the admitted evaluator and evidence atomically.**
   Require complete native quote repricing and applicable curve
   checks on the delivered result. Publish its construction record,
   report and affected typed bindings together. Preserve the original
   fit history and propagate stage-specific failures through
   restoration and existing valuation and host paths. Cache
   policy has its own record.
   [Accepted final fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve checks](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Replay cache](23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

## Independent calculation checks

- With a weekdays-only Following calendar, Sunday June 20, 2027
  is the contractual maturity and Monday June 21 is its payment
  date. A June 21 maturity selector cannot target the June 20
  binding. Replacing 140 bp with 175 bp and applying ±2 bp gives
  173 and 177 bp around the scenario center.
- US 30/360 from January 29 gives a source-clock width of
  `1/360` from January 30 to January 31. A new clock starting
  January 30 gives zero width to January 31.
- A constant 2% intensity and a candidate equal to 2% except
  for 4% on `(1,1.25]` and zero on `(1.25,1.5]` agree at
  years 1, 1.5 and 2. Their integrated hazards at 1.25 are
  .025 and .030, a survival difference of about 48.64 probability bp.

## Observed choice and rationale at the audit revision

A deal spread override changes one stored risk observation while preserving the original calibration evidence. Horizon replay instead derives a new quote recipe from a rolled target market so subsequent quote risk is anchored to the conditional survival curve at the new date.

## Observed deal override stages

Require finite replacement spread, a hazard recipe and at least one par-risk quote. Use the template quote to resolve the deal contract end through CDS standard-date rules; compute the target time using source hazard day count. Find an exact matching stored pillar date with time tolerance1e-12×max(1,|times|); no nearest-tenor fallback is allowed. Require the selected risk input be a par quote, replace only its spread, and rebuild the recipe with original calibration inputs and full config unchanged. Original knot values are not recalibrated by this preparation operation; later risk-center/bump action consumes the modified par-risk inputs. The template/selected quote identity and conventions are retained.

## Observed horizon stages

Reject any deal override. First replay source at zero shock on the source market and verify identity. Read the rolled hazard from target market and require a strictly later base date. Update target params base date; retain only risk input pillars after it. Each surviving input must be par-spread form. Build a dated CDS on the new base date at the same absolute maturity, obtain its par spread from the rolled target market, then calibrate those synthesized quotes using stored policy. No surviving pillar is an error.

Require fit acceptance and compare rebuilt survival to the rolled source representation at every rolled knot and adjacent-knot midpoint; absolute difference must be≤1e-8. Preserve source FX policy and attach the newly generated horizon recipe. The rolled input market itself is prepared by the caller, outside this record.

## Observed limits and alternatives

The horizon method preserves sampled conditional survival, not constant original spreads; the contract must make this distinction visible. Dated CDS rolls can alter a nonstandard contractual end, and identity checks cover a finite set of survival times. Alternative horizon methods hold market quotes constant or directly shift hazards, producing different carry and risk. Existing local tests inspect recipe/override mechanics; a same-kernel synthesis/recalibration roundtrip is not independent credit-model evidence.

## Source and validation evidence

- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — exact deal-pillar selection, recipe mutation, horizon synthesis and survival gate.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — action and override handling.
- Implementation: [build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — contract date normalization.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — deal-override risk-input identity and cache distinctions; inspected or located, not executed.
- Test evidence: [tests/calibration/reference_envelopes.rs](../../../../finstack-quant/calibration/tests/calibration/reference_envelopes.rs) — calibration envelope evidence located; not an independent horizon oracle; inspected or located, not executed.
- Independent examples above cover quote binding, conditional clocks and
  coverage. Production CDS fitting and complete replay qualification
  remain pending.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
