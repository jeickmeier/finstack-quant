# MODELS-ANALYTIC-26: Issuer-call LSMC continuation, sample split and sparse-regression fallback

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Callable rate payoffs implementing ExerciseBoundaryPayoff. Exercise values are undiscounted issuer call amounts. The `value_after` contract is time-0 pathwise PV of cashflows strictly after the exercise date; coupon-bearing payoffs must override it accordingly. Its default returns the full payoff PV and is valid only for a bullet payoff paid after every exercise date. The actual minimization is not a generic holder-option exercise rule.

## Choice and rationale

Backward regression chooses issuer exercise when call amount is below predicted continuation. Optional split-sample fitting holds out odd base streams. Sparse regression uses realized future continuation pathwise, introducing look-ahead.

## Algorithm

Simulate full paths and record r,bank,exercise amount,inactive flags. Save pre-exercise PV=full PV−value_after. Work backwards; train only active paths with positive exercise amount (and even base streams under OOS). Regress target=(current PV−pre PV)×bank on payoff-supplied basis using shared least-squares solver when active count>num_basis+2. For every active path with positive amount, if amount<predicted continuation replace PV=pre PV+amount/bank. If samples insufficient, compare amount directly to realized (PV−pre PV)×bank instead. Aggregate pair means; held-out pricing streams alone contribute under OOS.

## Selection and controls

Default in-sample (oos=false). Antithetic partners stay together via p/multiplicity parity. Standard ShortRateBasis degree<3 gives[1,r,r²,tr]; degree≥3 gives[1,r,r²,r³,tr,tr²]; configuration degree is consumed by product payoff construction, not automatically by this harness. Exercise times only checked for nonempty and membership in events within1e−12; mapping scans in provided order.

## Conventions

Cashflows on or before exercise remain in PV when issuer calls only when the payoff implements the `value_after` contract correctly. The default makes pre-exercise PV zero, so it does not preserve intermediate or same-date coupons. Exercise basis has rate units and fixed-time redundant columns (tr proportional r); shared least-squares regularization/rank behavior matters. The result uses the pricer's configured currency. The harness passes that currency to payoff methods, extracts numeric amounts and does not validate their returned Money currency tags; payoff implementations must honor the requested currency.

## Limitations, failure and fallback

Unsorted/duplicate exercise_times are not rejected though one-pass event mapping assumes ascending unique entries; some states can remain default. Sparse fallback uses future outcomes even on held-out paths, so OOS is not an independent fitted policy in that branch. Module prose claims in-sample upper bias and OOS lower complement for callable PV; this direction is not established for the implemented issuer-minimization and must not be adopted as a certified bracket.

## Alternatives and tradeoffs

Holder exercise max rule would be another product-specific method and is not selected. More training paths or explicit sparse-fit failure could avoid look-ahead; cross-fitting/dual bounds are unimplemented alternatives.

## Implementation and validation evidence

[lsmc.rs](../../../../finstack-quant/models/src/rates/hw1f/lsmc.rs), [exercise.rs](../../../../finstack-quant/models/src/rates/hw1f/exercise.rs) and [config.rs](../../../../finstack-quant/models/src/rates/hw1f/config.rs). Inline noexercise_equals_discounted_par and coupon preservation tests check pathwise accounting; neither independently qualifies exercise optimality or bias direction. Reference: [Monte Carlo exercise](../../../REFERENCES.md#glasserman-2004-monte-carlo).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
