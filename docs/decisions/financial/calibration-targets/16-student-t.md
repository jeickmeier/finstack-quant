# CALIBRATION-TARGETS-016 — Single-tranche Student-t degrees-of-freedom calibration

- **Decision status:** Accepted 2026-11-15 — complete single-tranche Student-t calibration and scalar publication package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent flat-response and pricing-allowance calculations verified. Production fitting, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for identified single-tranche Student-t calibration

1. **Fit one exact dated tranche quote.** Select it by quote ID and
   require an authored valuation close, exact index, series and
   version, and explicitly bound discount and active hazard roles.
   Use the accepted buyer-side preparation for seasoning, original
   tranche notional and dated upfront settlement. Freeze the
   contract, pool, hazards, recoveries and discounting throughout
   the solve. Retire discount-base-date inference and the
   reciprocal-width shortcut to manufacture unit tranche notional.
   [Accepted tranche preparation](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement).

2. **Vary only degrees of freedom on an explicit flat correlation
   basis.** Require literal asset correlation `0 ≤ ρ < 1` and apply
   the same private basis at both active tranche strikes. Preserve
   the common positive Student-t scale at zero correlation. The
   common-uniform endpoint at `ρ = 1` is ineligible for fitting:
   its fixed-marginal payoffs are invariant to degrees of freedom.
   Retire the template-curve lookup and replacement.
   [Accepted copula law](../models-and-valuations-credit/09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors) ·
   [Demarta and McNeil](https://onlinelibrary.wiley.com/doi/10.1111/j.1751-5823.2005.tb00254.x).

3. **Require root evidence on a qualified bounded branch.** Declare
   closed bounds `2 < lo < hi ≤ 1e8`, an in-bounds seed, exact
   scan coordinates, coordinate tolerance and bounded work.
   Reuse checked bounded search and Brent on one qualified
   continuous feasible branch. Require a computed zero or a
   sign-bracketed solution tied to the actual returned value.
   Reject ambiguous branches, merely tolerance-passing seeds and
   unbracketed approximations. Preserve typed pricing errors.
   [Accepted scalar search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence).

4. **Require resolved local sensitivity.** Use explicit probe
   steps at distinct represented values on the selected branch,
   with a qualified one-sided probe at an endpoint. Require a
   residual change in the branch's direction that exceeds the
   combined operational pricing allowances. Reject flat,
   exhausted or unresolved payoffs. A positive-attachment
   tranche ending at 100% can qualify through its attachment side
   when the complete residual resolves the parameter.
   [Accepted loss and numerical allowance rules](../models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).

5. **Reprice the actual returned configuration in native units.**
   Apply a finite positive explicit tolerance to the signed buyer
   residual `(P − c A_dirty − B C) / N₀`, a discounted fraction
   of original tranche notional. Retire the fixed 10 bp budget.
   Record the root evidence, sensitivity probes, operational
   pricing allowances and final quote error separately.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

6. **Publish the admitted scalar with its fitted basis.** Use an
   explicitly authored typed scalar key. Preserve the full-precision
   df, exact quote and market bindings, fixed correlation, model
   version, controls and actual work in the existing Rust-owned
   result. Publish only the scalar and its fit evidence, preserving
   the market's credit index and correlation source. Pricing, risk
   and replay require their own qualified binding to this fit basis.
   [Accepted execution and publication](../calibration-framework/03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution).

## Independent calculation checks

- For two equal zero-recovery names at one horizon, each with default
  probability `.5` and asset correlation `.5`, the Student-t
  thresholds are zero. The common positive scale leaves the default
  signs unchanged. The joint default probability is `1/3`; the
  0–30% terminal expected loss is `.30 × (1 − 1/3) = .20`
  of pool notional for every admitted df.
- If both probe evaluations carry a symmetric allowance of `2e-7`
  in native residual units, a change of `3e-7` is unresolved.
  A change of `5e-7` in the branch's direction clears the combined
  `4e-7` allowance. The final native quote gate is separate.

## Observed choice and scope at the audit revision

[CALIBRATION-FRAMEWORK-008](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement)
supplies the same prepared dated tranche and raw residual basis used for
base-correlation calibration. Hold the qualified correlation basis and
other contract and market inputs fixed when varying degrees of freedom.

[CALIBRATION-FRAMEWORK-011](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence)
supplies the shared bounded search, exact-coordinate evidence, typed
failure and work accounting. Apply the separate target financial gate
to the returned configuration.

Fit one Student-t copula degrees-of-freedom parameter to a selected tranche's complete discounted upfront quote while holding flat correlation, index hazards and recovery inputs fixed. This is not joint correlation/tail calibration. The requested existing base-correlation curve supplies a detachment grid/identity template, whose values are overwritten with the chosen flat correlation for this fit.

## Observed inputs and algorithm

1. Require finite bounds `2<lo<hi`, initial df inside them and correlation in `[0,1)`. Defaults are df 5, bounds `[2.1,50]`, correlation .3. Select the tranche quote with exact `tranche_instrument_id`. Require that index and template base-correlation curve exist. Discount ID is explicit or, in public preflight, the unique discount curve; multiple candidates require an ID.
2. Clone the template knot coordinates with every correlation set to params.correlation; insert it before rebinding the credit index so canonical context references retain the override. As-of is the chosen discount curve's **base date**, not a separate parameter date.
3. Build the full quote contract including coupon and upfront with portfolio build notional `1/(detachment-attachment)`, producing unit tranche notional under the builder's width conversion. Reject nonpositive/nonfinite width. Default tranche construction conventions apply; there are no separate schedule overrides in this target.
4. For candidate df in bounds, construct Student-t copula and pricer, evaluate full contractual NPV and divide by tranche notional. Invalid copula/pricer/valuation or out-of-bounds df maps to **positive infinity**, losing the detailed price error in the root objective. Hardcoded acceptance is `1e-3` discounted upfront fraction; numerical max iterations comes from global config.
5. Evaluate initial df first; accept immediately if finite residual is within .001. Otherwise scan bounds/initial, eight linear subdivisions and initial ±`{.1,.25,.5,1,2,5}` when inside bounds; sort and exact-deduplicate. Call shared scalar bracket solver with NearestFirst. A returned in-domain root must still reprice within tolerance; cached diagnostics are reused only when best point matches root within `1e-12`.
6. No acceptable root or final nonfinite/excess residual errors. Report ScalarRootFind, DiscountedUpfrontFraction, df/bounds/model version and residual under `<quote_id>_df`; iteration field records objective evaluations. Step runtime publishes a scalar `<tranche_instrument_id>_STUDENT_T_DF` separately from the index/correlation template used internally.

## Observed alternatives and limits

A wide fit budget can accept the initial parameter without identifying the true market-implied df. One quote with fixed correlation does not identify the full joint tail/correlation structure. Root multiplicity, flat df sensitivity and quadrature noise can dominate estimated df; the first acceptable nearest bracket does not prove uniqueness. Mapping all pricing failures to infinity can make a configuration/market error look like failure to bracket.

Alternatives are multi-tranche weighted calibration, joint correlation/df estimation with regularization, minimizing quote error in a transformed bounded parameter, or Gaussian-limit comparison. They change the model identification problem and must be agreed explicitly.

## Evidence

- Owning [student_t.rs](../../../../finstack-quant/calibration/src/targets/student_t.rs) inspected. [Tranche builder](../calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement) and [accepted scalar root search](../calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence) separate contract/kernel ownership.
- Inspected `student_t_reprices_upfront_at_cash_settlement` and returned-fit tests generate quotes through the same Student-t pricer; they establish settlement/return consistency, not independent copula accuracy. Narrow-bound, invalid-domain and loose-solver-tolerance rejection tests are located inline.
- [Demarta–McNeil](../../../REFERENCES.md#demarta-mcneil-2005-t-copula) supplies copula provenance, not this single-quote identification or .001 budget.

No tests were run.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
