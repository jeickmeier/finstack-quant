# CALIBRATION-FRAMEWORK-021: Lossless hazard replay, spread and recovery shocks

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Hazard replay requires a lossless recipe containing original calibration inputs, par-spread risk inputs, target parameters and full calibration config. It verifies the stored curve can be reproduced before applying a risk change, preserving quote convention and accepted recovery assumptions.

## Ordered algorithm and units

Deserialize target params/policy, validate recipe bindings, require stored/output IDs and discount ID agree. Requested documentation clause must share the stored convention family when both exist; family resolution uses explicit clause, registry primary family, then IsdaNa fallback. Requested valuation convention must equal stored convention when both are present; absent values do not themselves override the stored target.

Replay original calibration inputs at original recovery on the identity/source market (or target when no distinct identity market is supplied). Every quote receives that recovery, and runtime materializes/solves with stored policy. Require accepted report unless stored `fail_on_bad_fit=false`. Compare resulting knots to source: same count, time difference≤1e-12×max(1,|times|), hazard difference≤1e-10×max(1,|hazards|). This is knot identity, not a general off-knot pricing test.

If recovery bits are unchanged, all bumps zero/absent, risk-center mode false and no separate identity context, return original-input replay. Otherwise spread/risk-center/recovery actions select stored par-risk inputs; dependency-only replay selects original inputs. Parallel bump adds bp to every risk quote. Tenor bump sums every specified bump within absolute time distance<0.1 years of each stored pillar: it can hit several or no observations. Exact-index bump checks range and selects one ordered risk input. Nonzero spread bump on an upfront quote fails in the quote module; upfront contracts are not reinterpreted by changing their fixed coupon.

Recovery requests require finite [0,1); they recalibrate **unchanged par-risk spreads** at the new recovery. Replayed curves retain source FX policy and new replay recipe. Public no-recipe paths fail rather than falling back to hazard-space shifts.

## Limitations and alternatives

Stored strictness governs whether a failed-fit output is allowed. A direct spread request on a dependency-changed market without source identity context can fail the initial identity check; provider actions distinguish this intentionally. Near-zero or negative bumped spreads face quote validation. Global constant recipe-policy replay can differ from rate replay's fresh defaults.

Alternatives include hazard-rate shocks (different risk definition), preserving original upfront quotes for recovery risk, or exact-date-only bucket selection.

## Source and validation evidence

- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — recipe binding, identity check, action input choice, tenor/index bumps and recovery replay.
- Implementation: [recalibration/mod.rs](../../../../finstack-quant/calibration/src/recalibration/mod.rs) — stored strictness fit acceptance.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — public action selection and domains.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — strict/permissive report gate, recovery values and replay-input identity regressions; inspected or located, not executed.
- Reference context: [ISDA CDS standard model](../../../REFERENCES.md#isda-cds-standard-model) — reference convention context, not release-level validation; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
