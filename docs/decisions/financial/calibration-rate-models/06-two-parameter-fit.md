# CALIBRATION-RATE-MODELS-006: Bounded two-parameter HW fitting, multistart seeds and quote-fit acceptance

**Decision status:** Accepted 2026-11-15 — complete free two-parameter Hull–White fit, identification and publication package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent mixed-quote weighting, Halton sequence and rank-degenerate pricing calculations verified. Production fitting, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for bounded identified two-parameter Hull-White fitting

1. **Freeze the prepared quote set.** Use the accepted pricing and
   inverse for each selected swaption or cap/floor quote. Require
   unique quote IDs, authored order, one explicit finite nonnegative
   weight and a positive native volatility tolerance per row. Require
   at least two positive-weight observations. Keep zero-weight rows in
   final acceptance. The fixed-κ scalar and piecewise fits retain
   their separately selected methods.
   [Accepted swaption quote reconstruction](03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction) ·
   [Accepted cap quote reconstruction](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

2. **Fit dimensionless tolerance-scaled premium errors.** Minimize the
   norm of `sqrt(w_i) (P_HW,i − P_market,i)/(ν_i τ_i)` on active
   rows. Freeze the prepared market premium, same-law market vega
   `ν_i` and native volatility tolerance `τ_i`. Use the actual
   strike and quote clock, including for strip vega. Evaluate the
   complete ratio in checked scale. Retire the fixed vega floor.
   Relative native tolerances deliberately enter the objective.
   [Accepted weighted solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

3. **Use explicit open native bounds and a complete interior seed.**
   Require finite bounds with nonnegative lower limits for κ in
   inverse model years and absolute σ per square root model year.
   Use the checked logistic map for every seed, probe, trial and
   fitted pair. Admit an expanded versioned preset by election.
   Retire implicit boxes, dummy maturity coordinates, clipping and
   distance-to-bound rejection. Preserve exact zero in its qualified
   pricing and fixed-parameter profiles.

4. **Use checked Levenberg–Marquardt with explicit controls.** Select
   the augmented pivoted QR kernel and checked central differences
   in the transformed solver coordinates. Require positive residual
   and step tolerances and a nonzero attempt budget. Preserve typed
   pricing and derivative failures. Track numerical termination
   separately from market fit and complete output admission.
   [Accepted least-squares kernel](../core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes).

5. **Elect and bound deterministic restarts.** Keep the primary seed
   at index zero. Require the additional proposal count and both
   solver-coordinate half-widths. Use Halton bases 2 and 3 for κ and σ.
   Record invalid proposals and exact duplicates without replacements.
   Reserve bounded work before running each admitted start once.
   Collect outcomes in start order under the selected execution policy.
   [Accepted multi-start contract](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection) ·
   [Halton's construction](https://doi.org/10.1007/BF01386213).

6. **Require two resolved local sensitivity directions.** At the
   actual candidate form the undamped weighted Jacobian on
   positive-weight rows in the elected solver coordinates.
   Require the smaller singular direction to clear an explicit
   relative cutoff after accounting for pricing and derivative
   probe uncertainty. Treat this as a required local admission
   check. Optional sensitivity reporting retains its advisory role.
   [Accepted sensitivity arithmetic](../calibration-framework/15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

7. **Reconstruct native quotes before ranking candidates.** Reprice the
   exact returned candidate and invert every consumed quote in its
   original convention, including zero-weight rows. Require each
   signed implied-minus-observed error to pass its native tolerance.
   Apply the identification and complete-output gates. Rank
   passing candidates by the recomputed weighted norm, using
   earlier start index on an exact tie.
   [Accepted candidate selection](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

8. **Publish the admitted pair with actual fit evidence.** Preserve
   the fitted parameters, prepared pricing methods, effective controls,
   indexed start outcomes and actual work across starts. Keep optimizer
   residuals and native signed quote errors separate. Summarize
   mixed-law fit with tolerance ratios. Failed runs may retain
   bounded diagnostics.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

## Independent calculation checks

- For `A=4`, `F=.03` and quote expiry `2`, start at Normal
  `.01` and Black `.20`, with tolerances `.0001` and `.01`.
  A move of half each tolerance gives normalized premium-over-vega
  errors of `.500000` and about `.499874`. Black curvature
  accounts for the latter's deviation from one half.
- On the one-period ATM synthetic swap with exercise in one
  model year and payment in two, use exercise discount `.99`
  and payment discount `.97`. Parameter pairs
  `(ln 2,.02)` and `(ln 4,.02 × (4/3) sqrt(8/5))` give
  the same total bond-option standard deviation and the same
  premium of about `.0041910289914` per unit notional.
  Quoting that payoff under both laws still supplies one local
  parametric sensitivity direction.
- For a primary solver seed `(−3,−4)` and half-widths
  `(.5,.5)`, Halton bases 2 and 3 give first proposals
  `(−3,−25/6)` and `(−13/4,−23/6)`.

## Observed choice and rationale at the audit revision

Swaptions and scalar caps without fixed κ share a two-parameter log-space Levenberg–Marquardt fit. Each quote's price error is divided by its market vega, approximating quoted-volatility error near the market premium. Source rationale is to prevent high-annuity/high-vega quotes dominating pure price least squares. The approximation and the subsequent exact quote-fit gate are distinct methods.

[CALIBRATION-FRAMEWORK-013](../calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection)
requires complete Hull–White quote and output gates on each candidate
before ranking passing final weighted norms. The accepted target package
above supplies the effective Hull–White fit and identification policy.

## Observed domain and ordered stages

1. Require at least two observations, finite positive quote-fit tolerance, valid quote/schedule inputs and finite market vegas at least `1e-8`. Duplicate observations retain separate indexed residuals; this does not ensure independent information.
2. Prepare market prices, vegas and cashflow/schedule data once. Swaption default seed uses horizon `h=mean(expiry+tenor/2)`, `κ₀=clamp(1/max(h,0.5),0.01,0.30)`. Mean absolute-vol scale uses each Normal quote directly and each Black quote times `max(|F|,0.005)`; `σ₀=clamp(mean_scale,0.001,0.05)`. Scalar cap default seed is `(0.03,0.01)`. Explicit initial guesses replace these seeds.
3. Optimize `x=(ln κ,ln σ)`; exponentiate to natural parameters for prices. Bounds are κ∈[0.001,1], σ∈[1e-5,2]. Dummy positive times `[1,2]` adapt two scalar parameters to the framework's knot interface; they are not financial maturities.
4. For each observation compute `r_i=(P_HW−P_market)/vega_market`. Swaptions select contractual or synthetic pricing by schedule presence; caps sum scheduled HW caplets. Outer residual weights are all one; the explicit vega division is the economic weighting. The [accepted global solve](../calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates) requires typed pricing failures and complete final target checks before candidate ranking.
5. Use a fresh default calibration config with solver tolerance `1e-12`, maximum 300 iterations and numerical Jacobians. Run the original seed plus five deterministic Halton restarts, additive half-width 0.5 in each log coordinate, clamped to bounds. This means natural-parameter factors about `[exp(−0.5),exp(0.5)]`, not a literal ±50% perturbation. The [observed multistart implementation](../calibration-framework/13-deterministic-multi-start.md#algorithm-and-selection) chooses the lowest final weighted residual norm; individual failed starts may be skipped.
6. Reprice and reconstruct actual implied quote volatilities, using [ATM inversion](03-atm-swaption-quotes.md) or [cap-strip inversion](02-cap-schedules-and-quote-inversion.md). Build the final report from these signed errors and the required maximum-absolute-error budget. Reject natural parameters within relative `1e-6` of either box boundary, even if quoted premiums fit. The cap path applies that boundary gate before final quote reconstruction; the swaption path applies it after reconstruction and report enrichment.

## Observed selection and report semantics

The plan adapter supplies only model inputs, seed and quote-fit tolerance to this solver; its global numerical solver settings do not replace the fixed HW controls above. The direct and engine paths invoke the same method. Caps with fixed κ or piecewise σ use separate algorithms.

Final main residuals, RMSE/max-error and acceptance use reconstructed quoted volatility. Optimizer objective/termination and inherited metadata are retained separately; inherited `final_*` optimizer residual diagnostics still describe vega-linearized errors, not the replaced exact-volatility vector. Final-bound clamping remains marked as penalty use. The observed engine may install outputs with an unsuccessful report under its permissive setting. The [accepted fit contract](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) requires separate objective and financial outcomes and limits exploratory failure to diagnostics.

## Observed conventions, limitations and failure behavior

κ has inverse-year units; σ is absolute rate volatility per square-root year. Normal residuals are decimal rate volatility; Black residuals are relative volatility. Mixed swaption quotations therefore have different economic units under a single scalar threshold. Market vega is fixed at its quoted level, so the LM objective is only locally equivalent to quote least squares. No Hessian/rank/identification acceptance rule is added here beyond quote count and bound rejection.

Box limits, restart band and solver tolerance are choices awaiting qualification, not proofs of a global optimum. A hard failure across all starts or final pricing/inversion error propagates. Normal numerical termination and acceptable quote fit need not be synonymous. Boundary error text interprets a pinned parameter as weak identification; pinning alone does not establish that economic diagnosis.

## Alternatives and tradeoffs

Raw-price least squares weights premium scale; exact implied-volatility residuals would put inversion inside every iteration; per-quote tolerances could normalize different conventions. Global search or wider starts would cost more without automatically establishing identification. The [fixed-κ cap method](07-fixed-kappa-cap-fit.md) explicitly selects raw-price SSE and is not equivalent to this objective.

## Implementation and evidence

[targets.rs](../../../../finstack-quant/calibration/src/hull_white/targets.rs), [swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs), [cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs); shared dependencies are owned by the [calibration framework](../calibration-framework/README.md). Existing formulation references include [numerical optimization](../../../REFERENCES.md#gilli-maringer-schumann-2011) and [Halton](../../../REFERENCES.md#halton-1960).

[tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) contains same-kernel synthetic parameter recovery, repeated observations, mixed-convention seed scaling, nonfinite-price context and bound cases. The broad swaption recovery test allows κ ±20% and σ ±50%; these are test envelopes, not claimed estimation uncertainty. `m11_quote_fit_budget_controls_acceptance_without_changing_parameters` inspects fit-budget separation on fixed-κ caps. No suite or market fit was executed.

Source comments label `1e-8` vega as “1 bp of annuity-year” and 0.5 log perturbation as “50%”; those unit descriptions do not match the numeric contracts. Cap report commentary calls the scaling ATM vega, but actual strip vega is evaluated at each quote's strike.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
