# CALIBRATION-RATE-MODELS-006: Bounded two-parameter HW fitting, multistart seeds and quote-fit acceptance

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

Swaptions and scalar caps without fixed κ share a two-parameter log-space Levenberg–Marquardt fit. Each quote's price error is divided by its market vega, approximating quoted-volatility error near the market premium. Source rationale is to prevent high-annuity/high-vega quotes dominating pure price least squares. The approximation and the subsequent exact quote-fit gate are distinct methods.

## Domain and ordered stages

1. Require at least two observations, finite positive quote-fit tolerance, valid quote/schedule inputs and finite market vegas at least `1e-8`. Duplicate observations retain separate indexed residuals; this does not ensure independent information.
2. Prepare market prices, vegas and cashflow/schedule data once. Swaption default seed uses horizon `h=mean(expiry+tenor/2)`, `κ₀=clamp(1/max(h,0.5),0.01,0.30)`. Mean absolute-vol scale uses each Normal quote directly and each Black quote times `max(|F|,0.005)`; `σ₀=clamp(mean_scale,0.001,0.05)`. Scalar cap default seed is `(0.03,0.01)`. Explicit initial guesses replace these seeds.
3. Optimize `x=(ln κ,ln σ)`; exponentiate to natural parameters for prices. Bounds are κ∈[0.001,1], σ∈[1e-5,2]. Dummy positive times `[1,2]` adapt two scalar parameters to the framework's knot interface; they are not financial maturities.
4. For each observation compute `r_i=(P_HW−P_market)/vega_market`. Swaptions select contractual or synthetic pricing by schedule presence; caps sum scheduled HW caplets. Pricing errors/nonfinite model values make that candidate infeasible through the [global solver](../calibration-framework/12-weighted-global-solve.md), rather than constituting real quote residuals. Outer residual weights are all one; the explicit vega division is the economic weighting.
5. Use a fresh default calibration config with solver tolerance `1e-12`, maximum 300 iterations and numerical Jacobians. Run the original seed plus five deterministic Halton restarts, additive half-width 0.5 in each log coordinate, clamped to bounds. This means natural-parameter factors about `[exp(−0.5),exp(0.5)]`, not a literal ±50% perturbation. The [shared multistart](../calibration-framework/13-deterministic-multi-start.md) chooses the lowest final weighted residual norm; individual failed starts may be skipped.
6. Reprice and reconstruct actual implied quote volatilities, using [ATM inversion](03-atm-swaption-quotes.md) or [cap-strip inversion](02-cap-schedules-and-quote-inversion.md). Build the final report from these signed errors and the required maximum-absolute-error budget. Reject natural parameters within relative `1e-6` of either box boundary, even if quoted premiums fit. The cap path applies that boundary gate before final quote reconstruction; the swaption path applies it after reconstruction and report enrichment.

## Selection and report semantics

The plan adapter supplies only model inputs, seed and quote-fit tolerance to this solver; its global numerical solver settings do not replace the fixed HW controls above. The direct and engine paths invoke the same method. Caps with fixed κ or piecewise σ use separate algorithms.

Final main residuals, RMSE/max-error and acceptance use reconstructed quoted volatility. Optimizer objective/termination and inherited metadata are retained separately; inherited `final_*` optimizer residual diagnostics still describe vega-linearized errors, not the replaced exact-volatility vector. Final-bound clamping remains marked as penalty use. Generic report/gating behavior belongs to [fit reports](../calibration-framework/14-fit-reports.md). A direct return may contain an unsuccessful report; the engine's configured policy determines whether it installs it.

## Conventions, limitations and failure behavior

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
