# MARGIN-COLLATERAL-IM-015 — Collateral eligibility and haircut initial margin

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope below, including the stated limitations and discrepancies.

**Validation status:** Evidence inspected or specifically located; no tests/builds executed. Audit date **2026-11-05**; source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Choice and rationale

Observed: the strict haircut calculator selects the first eligible contractual entry then multiplies collateral value by its haircut. Lightweight schedule helpers have looser semantics. Proposed rationale: eligibility and contractual haircuts must be considered before estimating collateral-based IM.

## Input domain and conventions

Strict calculation requires finite nonnegative collateral Money. Optional remaining maturity is finite/nonnegative years; an entry with maturity/rating restrictions needs corresponding supplied metadata. Ratings use the core CreditRating parser/order; NR never satisfies a minimum rating. Decimal base/FX haircuts are each required within[0,1].

## Ordered algorithm

1. Iterate schedule entries in vector order; match asset class, then optional maturity constraints, then minimum rating. Maturity comparison permits ±2.7e-6 years; minimum and maximum are inclusive with tolerance. Overlapping boundary entries therefore select the first.
2. A rating-restricted entry requires a non-NR rating no worse than its parsed floor. An invalid encountered rating label errors. The matching entry's base and FX haircuts must each be finite/in[0,1].
3. Total haircut is base+FX addon when mismatch=true, else base. No total cap is applied. IM=value×total_haircut, one class breakdown, fixed MPOR 2; no CSA thresholds are applied.
4. Trait calculation requires explicit exposure base. Mismatch is inferred only if posted-collateral currency was declared and differs from the amount currency; absent declaration means no FX addon.
5. Lightweight `haircut_for` selects the first class entry without maturity/rating checks, `haircut_for_maturity` checks only maturity, and both can return default_haircut. `is_eligible` treats any configured default haircut as class eligibility. Strict `HaircutImCalculator` never uses that fallback and does not infer metadata.

## Selection, controls and limitations

Embedded `bcbs_standard` permits cash 0% and AA- or better government bonds at 0.5%≤1y,2%≤5y,4%thereafter with8% FX addon; no default haircut/rehypothecation. `us_treasuries` has0.5%≤1y,1%≤5y,1.5%≤10y,2%thereafter, zeroFX addon and permits reuse; there is no issuer/country field enforcing actual US Treasury identity. Class conveniences use broader defaults (government 2%, corporate5%, equity/gold/funds15%, etc.) independently of these schedules.

## Alternatives and tradeoffs

Security-level identifiers and explicit eligibility outcomes would prevent conflating broad asset class with issuer/legal eligibility. A single strict lookup policy would be easier to reason about than the helper/calculator split but would narrow helper behavior. A compounded or capped combined haircut is a different contractual convention.

## Source and validation evidence

Inspected hand/invariant tests cover maturity10y selecting1.5%, zero Treasury FX addon, rejected missing maturity/negative collateral/ineligible equity, and 8% cash FX addon. Tests establish the implemented cases, not general regulatory or legal eligibility.

Implementation and data: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [src/types/serde_validation.rs](../../../../finstack-quant/margin/src/types/serde_validation.rs), [src/calculators/im/haircut.rs](../../../../finstack-quant/margin/src/calculators/im/haircut.rs), [data/margin/collateral_schedules.v1.json](../../../../finstack-quant/margin/data/margin/collateral_schedules.v1.json).

Existing evidence: [src/types/collateral.rs](../../../../finstack-quant/margin/src/types/collateral.rs), [src/calculators/im/haircut.rs](../../../../finstack-quant/margin/src/calculators/im/haircut.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Adoption, implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
