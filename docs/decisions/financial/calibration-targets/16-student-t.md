# CALIBRATION-TARGETS-016 — Single-tranche Student-t degrees-of-freedom calibration

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and scope

Fit one Student-t copula degrees-of-freedom parameter to a selected tranche's complete discounted upfront quote while holding flat correlation, index hazards and recovery inputs fixed. This is not joint correlation/tail calibration. The requested existing base-correlation curve supplies a detachment grid/identity template, whose values are overwritten with the chosen flat correlation for this fit.

## Inputs and algorithm

1. Require finite bounds `2<lo<hi`, initial df inside them and correlation in `[0,1)`. Defaults are df 5, bounds `[2.1,50]`, correlation .3. Select the tranche quote with exact `tranche_instrument_id`. Require that index and template base-correlation curve exist. Discount ID is explicit or, in public preflight, the unique discount curve; multiple candidates require an ID.
2. Clone the template knot coordinates with every correlation set to params.correlation; insert it before rebinding the credit index so canonical context references retain the override. As-of is the chosen discount curve's **base date**, not a separate parameter date.
3. Build the full quote contract including coupon and upfront with portfolio build notional `1/(detachment-attachment)`, producing unit tranche notional under the builder's width conversion. Reject nonpositive/nonfinite width. Default tranche construction conventions apply; there are no separate schedule overrides in this target.
4. For candidate df in bounds, construct Student-t copula and pricer, evaluate full contractual NPV and divide by tranche notional. Invalid copula/pricer/valuation or out-of-bounds df maps to **positive infinity**, losing the detailed price error in the root objective. Hardcoded acceptance is `1e-3` discounted upfront fraction; numerical max iterations comes from global config.
5. Evaluate initial df first; accept immediately if finite residual is within .001. Otherwise scan bounds/initial, eight linear subdivisions and initial ±`{.1,.25,.5,1,2,5}` when inside bounds; sort and exact-deduplicate. Call shared scalar bracket solver with NearestFirst. A returned in-domain root must still reprice within tolerance; cached diagnostics are reused only when best point matches root within `1e-12`.
6. No acceptable root or final nonfinite/excess residual errors. Report ScalarRootFind, DiscountedUpfrontFraction, df/bounds/model version and residual under `<quote_id>_df`; iteration field records objective evaluations. Step runtime publishes a scalar `<tranche_instrument_id>_STUDENT_T_DF` separately from the index/correlation template used internally.

## Alternatives and limits

A wide fit budget can accept the initial parameter without identifying the true market-implied df. One quote with fixed correlation does not identify the full joint tail/correlation structure. Root multiplicity, flat df sensitivity and quadrature noise can dominate estimated df; the first acceptable nearest bracket does not prove uniqueness. Mapping all pricing failures to infinity can make a configuration/market error look like failure to bracket.

Alternatives are multi-tranche weighted calibration, joint correlation/df estimation with regularization, minimizing quote error in a transformed bounded parameter, or Gaussian-limit comparison. They change the model identification problem and must be agreed explicitly.

## Evidence

- Owning [student_t.rs](../../../../finstack-quant/calibration/src/targets/student_t.rs) inspected. [Tranche builder](../calibration-framework/08-tranche-materialization.md) and [scalar root search](../calibration-framework/11-scalar-root-search.md) separate contract/kernel ownership.
- Inspected `student_t_reprices_upfront_at_cash_settlement` and returned-fit tests generate quotes through the same Student-t pricer; they establish settlement/return consistency, not independent copula accuracy. Narrow-bound, invalid-domain and loose-solver-tolerance rejection tests are located inline.
- [Demarta–McNeil](../../../REFERENCES.md#demarta-mcneil-2005-t-copula) supplies copula provenance, not this single-quote identification or .001 budget.

No tests were run.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
