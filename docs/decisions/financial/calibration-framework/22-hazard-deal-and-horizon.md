# CALIBRATION-FRAMEWORK-022: Deal quote overrides and horizon hazard reconstruction

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

A deal spread override changes one stored risk observation while preserving the original calibration evidence. Horizon replay instead derives a new quote recipe from a rolled target market so subsequent quote risk is anchored to the conditional survival curve at the new date.

## Deal override stages

Require finite replacement spread, a hazard recipe and at least one par-risk quote. Use the template quote to resolve the deal contract end through CDS standard-date rules; compute the target time using source hazard day count. Find an exact matching stored pillar date with time tolerance1e-12×max(1,|times|); no nearest-tenor fallback is allowed. Require the selected risk input be a par quote, replace only its spread, and rebuild the recipe with original calibration inputs and full config unchanged. Original knot values are not recalibrated by this preparation operation; later risk-center/bump action consumes the modified par-risk inputs. The template/selected quote identity and conventions are retained.

## Horizon stages

Reject any deal override. First replay source at zero shock on the source market and verify identity. Read the rolled hazard from target market and require a strictly later base date. Update target params base date; retain only risk input pillars after it. Each surviving input must be par-spread form. Build a dated CDS on the new base date at the same absolute maturity, obtain its par spread from the rolled target market, then calibrate those synthesized quotes using stored policy. No surviving pillar is an error.

Require fit acceptance and compare rebuilt survival to the rolled source representation at every rolled knot and adjacent-knot midpoint; absolute difference must be≤1e-8. Preserve source FX policy and attach the newly generated horizon recipe. The rolled input market itself is prepared by the caller, outside this record.

## Alternatives

The horizon method preserves sampled conditional survival, not constant original spreads; the contract must make this distinction visible. Dated CDS rolls can alter a nonstandard contractual end, and identity checks cover a finite set of survival times. Alternative horizon methods hold market quotes constant or directly shift hazards, producing different carry and risk. Existing local tests inspect recipe/override mechanics; a same-kernel synthesis/recalibration roundtrip is not independent credit-model evidence.

## Source and validation evidence

- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — exact deal-pillar selection, recipe mutation, horizon synthesis and survival gate.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — action and override handling.
- Implementation: [build/cds.rs](../../../../finstack-quant/calibration/src/build/cds.rs) — contract date normalization.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — deal-override risk-input identity and cache distinctions; inspected or located, not executed.
- Test evidence: [tests/calibration/reference_envelopes.rs](../../../../finstack-quant/calibration/tests/calibration/reference_envelopes.rs) — calibration envelope evidence located; not an independent horizon oracle; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
