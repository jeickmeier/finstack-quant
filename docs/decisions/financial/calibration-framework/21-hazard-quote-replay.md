# CALIBRATION-FRAMEWORK-021: Lossless hazard replay, spread and recovery shocks

- Decision status: **Accepted 2026-11-15 — complete hazard replay qualification, par-risk targeting, original-quote recovery and publication package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent source-clock and recovery-basis checks verified.** Production replay, pricing, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for qualified hazard quote replay and publication

1. **Qualify the original evaluator before every replay.** Rebuild the
   original observations at their original recovery on the explicit
   immutable source market. Require the original fit and output gates.
   Match the resolved CDS terms, convention edition, restructuring
   clause, pricing convention, typed discount dependency, dates,
   clock, recovery, FX policy, intensity segments and tail. Optional
   caller conventions assert the exact resolved record. Retire regional
   family inference, the `IsdaNa` fallback and knot-only identity.
   Apply owner-qualified numerical tolerances to the fitted values.
   [Accepted replay provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) ·
   [Accepted CDS terms and settlement](07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement).

2. **Bind each original observation and par-risk input exactly.** Keep
   an original par quote literal. Preserve fixed coupon, signed upfront
   and settlement basis in the original upfront observation. Derive
   a par-risk spread for that observation with the checked CDS pricer
   on the qualified original fit and its stated clean or dirty basis.
   Verify its derived value and exact quote, maturity and convention
   binding on replay. Carry model-implied provenance and enforce the
   target's quote domain without flooring.
   [Accepted CDS quote representations](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery) ·
   [ISDA converter specification](https://cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

3. **Use par-risk inputs for spread actions and their center.** Fit an
   explicit zero-shock par-risk center on the selected target market.
   Use the same par-risk inputs, recovery and discount state as the
   basis for the center and shocked sides. Apply signed bp moves in
   parallel or to exact quote IDs or verified recipe indices. A pillar
   date selects a quote only when it resolves uniquely. Reject empty,
   duplicate, ambiguous or missing targets, invalid resulting quotes
   and nonzero moves that round away. Validate zero actions, retire
   the 0.1-year tenor window, and report exact targets and represented
   moves.
   [Accepted CDS spread risk](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).

4. **Replay original observations after a dependency change.** Keep
   original forms, values, recovery, terms, cut and fit policy. Refit
   the original observations against the explicitly bound target
   dependency state. A changed discount state can require a new fit
   with zero spread move. An identical original-form request may
   reuse the accepted source reconstruction. Direct intensity shocks
   have a separately elected basis.

5. **Preserve original quotes for recovery replay.** Hold discount
   state fixed and refit the original par or upfront observations at
   the elected effective recovery in `[0,1)`. Preserve historical
   assumptions and derive a fresh par-risk view from the accepted new
   fit. Frozen-intensity recovery has a separate operation.
   [Accepted Recovery01 basis](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).

6. **Return the admitted target fit and evidence.** Require complete
   native quote repricing and applicable curve checks on the actual
   delivered evaluator. Return its new construction record and fit
   report. Exploratory bad-fit settings cannot qualify replay. Preserve
   stage-specific errors through restore and existing Rust, Python and
   WASM paths. Deal overrides, horizon replay and cache policy have
   separate owners.
   [Accepted fit reports](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted curve validation](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication) ·
   [Accepted deal override and horizon replay](22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay) ·
   [Replay cache](23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

## Independent calculation checks

- Two identical numeric terminal knots `(1,.06)` on constant
  intensity tails give integrated hazards `73/1200` on ACT/360
  and `3/50` on ACT/365F after 365 days. Default probabilities
  differ by about 7.844769 probability bp.
- In a one-period example with unit discount, survivor-only
  premium and no accrual on default, a 100 bp coupon, 10% default
  probability and 40% recovery give 5.10% upfront and par spread
  `1/15`. At 20% recovery, the original coupon and upfront fit
  default probability `61/810`. Holding derived par instead gives
  `1/13` and implies upfront `17/325`, or about 5.230769%.

## Observed choice and rationale at the audit revision

Hazard replay requires a lossless recipe containing original calibration inputs, par-spread risk inputs, target parameters and full calibration config. It verifies the stored curve can be reproduced before applying a risk change, preserving quote convention and accepted recovery assumptions.

## Observed algorithm and units

Deserialize target params/policy, validate recipe bindings, require stored/output IDs and discount ID agree. Requested documentation clause must share the stored convention family when both exist; family resolution uses explicit clause, registry primary family, then IsdaNa fallback. Requested valuation convention must equal stored convention when both are present; absent values do not themselves override the stored target.

Replay original calibration inputs at original recovery on the identity/source market (or target when no distinct identity market is supplied). Every quote receives that recovery, and runtime materializes/solves with stored policy. Require accepted report unless stored `fail_on_bad_fit=false`. Compare resulting knots to source: same count, time difference≤1e-12×max(1,|times|), hazard difference≤1e-10×max(1,|hazards|). This is knot identity, not a general off-knot pricing test.

If recovery bits are unchanged, all bumps zero/absent, risk-center mode false and no separate identity context, return original-input replay. Otherwise spread/risk-center/recovery actions select stored par-risk inputs; dependency-only replay selects original inputs. Parallel bump adds bp to every risk quote. Tenor bump sums every specified bump within absolute time distance<0.1 years of each stored pillar: it can hit several or no observations. Exact-index bump checks range and selects one ordered risk input. Nonzero spread bump on an upfront quote fails in the quote module; upfront contracts are not reinterpreted by changing their fixed coupon.

Recovery requests require finite [0,1); they recalibrate **unchanged par-risk spreads** at the new recovery. Replayed curves retain source FX policy and new replay recipe. Public no-recipe paths fail rather than falling back to hazard-space shifts.

## Observed limitations and alternatives

Stored strictness governs whether a failed-fit output is allowed. A direct spread request on a dependency-changed market without source identity context can fail the initial identity check; provider actions distinguish this intentionally. Near-zero or negative bumped spreads face quote validation. Global constant recipe-policy replay can differ from rate replay's fresh defaults.

Alternatives include hazard-rate shocks (different risk definition), preserving original upfront quotes for recovery risk, or exact-date-only bucket selection.

## Source and validation evidence

- Implementation: [recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — recipe binding, identity check, action input choice, tenor/index bumps and recovery replay.
- Implementation: [recalibration/mod.rs](../../../../finstack-quant/calibration/src/recalibration/mod.rs) — stored strictness fit acceptance.
- Implementation: [recalibration/provider.rs](../../../../finstack-quant/calibration/src/recalibration/provider.rs) — public action selection and domains.
- Test evidence: [src/recalibration/hazard.rs](../../../../finstack-quant/calibration/src/recalibration/hazard.rs) — strict/permissive report gate, recovery values and replay-input identity regressions; inspected or located, not executed.
- Reference context: [ISDA CDS standard model](../../../REFERENCES.md#isda-cds-standard-model) gives convention context. The independent checks above cover source-clock and recovery-basis distinctions. Production replay qualification remains pending.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
