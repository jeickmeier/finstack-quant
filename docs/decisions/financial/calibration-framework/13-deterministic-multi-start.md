# CALIBRATION-FRAMEWORK-013: Deterministic Halton restart selection

- Decision status: **Draft (observed implementation; adoption pending)**.
- Implementation status: **Observed for the stated source scope**; discrepancies and limits are stated below.
- Validation status: **Evidence inspected/located, not executed**. No numerical qualification or external compliance certification is implied.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Observed choice and rationale

Multi-start uses a deterministic low-discrepancy perturbation rather than random seeds. This makes repeatable exploration possible, while ordered collection resolves ties reproducibly. The perturbation lives in **the target parameter units**, not as a fraction of the initial value.

## Algorithm and selection

Always attempt the primary initial guess. For restart index j and parameter dimension i, compute radical inverse `H_(j+1)(base_i)` using bases [2,3,5,7,11,13,17,19,23,29] cyclically. Set `x'_i = x_i + scale × (2H−1)` then clip to target lower/upper bounds. A target using logarithmic parameters consequently receives multiplicative movement after exponentiation; for scale 0.5 this is about exp(-0.5) to exp(0.5), not ±50% in physical units.

Native restart computation uses Rayon regardless of plan `use_parallel`; WASM runs the same ordered restart sequence sequentially. Results collect in restart order. Keep any successful single solve whose weighted L2 is strictly smaller than current best; ties retain the earlier winner (primary first). Failed attempts are skipped; if all fail, return primary error where available. Only the winning solve's optimizer stats and evaluation-error counts are published, with total configured restarts and winner label.

The public plan has no generic restart setting. Targets explicitly choose whether to use this path and supply count/scale. HW's exact configuration is in [rate-model calibration](../calibration-rate-models/06-two-parameter-fit.md).

## Limits and alternatives

This is deterministic sampling, not global optimization. Beyond ten dimensions, identical base streams repeat and correlate perturbations. Additive scale is not normalized by parameter range, nor by |initial|. No shared validation of a negative/nonfinite scale or pathological restart count is shown here; current callers must supply valid constants. Perturbation clipping followed by inward solver projection can duplicate starts.

Alternatives include a target-scaled Sobol design, stratified bounded restarts or optimizer-specific basin searches. These increase control complexity and need evidence before adoption.

## Source and validation evidence

- Implementation: [solver/multi_start.rs](../../../../finstack-quant/calibration/src/solver/multi_start.rs) — radical-inverse bases and additive clipped perturbations.
- Implementation: [solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — parallel restart collection, strict improvement and winning-stat selection.
- Test evidence: [src/solver/multi_start.rs](../../../../finstack-quant/calibration/src/solver/multi_start.rs) — known Halton values, near-zero movement, scale independence and bounds; inspected or located, not executed.
- Test evidence: [src/solver/global.rs](../../../../finstack-quant/calibration/src/solver/global.rs) — deterministic perturbation regression located; inspected or located, not executed.
- Reference gap: no method-specific independent source or external benchmark is established by this record; source and regression evidence describe implementation behavior.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
