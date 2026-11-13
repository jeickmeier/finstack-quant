# MODELS-MONTE-CARLO-17: Terminal payoffs and weighted historical Asian fixings

- Decision status: **Accepted — complete checked terminal and Asian payoffs package, 2026-11-11**.
- Implementation status: Existing terminal and Asian payoffs, capture and pricing routes observed. Checked terms, completion, exact schedules and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent payoff, range and geometric-control examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked terminal and Asian payoffs

1. **Use immutable checked terms and signed position size.** Admit finite signed
   spot levels, strikes and forward delivery levels in a common price unit.
   Admit finite signed quantities for European, forward and Asian payoffs,
   including zero. Positive quantity is long. More restrictive underlying,
   strike or quantity domains belong to the selected product and process.
   Select call or put explicitly where applicable. Validate terms and their
   entire event binding before draws and restore any serialized inputs through
   the same checks. Keep missing terms distinct from literal zero. Replace
   mutable fields and unchecked constructors with validated terms.

2. **Define the three terminal settlement formulas.** At the single observed
   spot `S`, a European call pays `q*max(S-K,0)` and a put pays
   `q*max(K-S,0)`. A cash digital call pays its exact signed, currency-tagged
   fixed amount only if `S>K`. The put pays it only if `S<K`. Both pay zero
   at equality, including on deterministic paths. Use an exact comparison
   without a tolerance or a smoothed indicator. A forward pays `q*(S-F)`
   with signed quantity as the single source of position direction. An
   adverse long forward can owe cash. These formulas specify cash settlement.
   Physical delivery and product-specific digital exercise conditions belong
   to their product owners.
   [European payoff reference](../../../REFERENCES.md#black-scholes-1973).

3. **Bind and complete the terminal observation exactly once.** Require a
   single explicit observation node on the accepted grid, including origin
   if selected. Use the named, finite post-event `SPOT` at that stored node.
   Dated owners establish the contractual observation, its phase and its
   exact model-time mapping. Before observation, final value is incomplete.
   Reject a repeated observation callback or an omitted required observation,
   and clear per-path state before each leg. Non-observation nodes leave the
   observed spot untouched. The observation node can precede the settlement
   reference. [Accepted execution](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [accepted grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots).

4. **Select the Asian average and every fixing slot explicitly.** Choose
   arithmetic or geometric fixed-strike averaging. Require one immutable
   ordered row per future contractual observation, with its exact node
   index and finite positive, dimensionless spot multiplier. Require
   nondecreasing indices and preserve ties as distinct rows in the given
   order. Each row supplies one observation. Multiple rows at one node
   share its single simulated `SPOT` but may have different multipliers.
   Use an exact dated-to-stored-knot mapping. Distinct times stay distinct,
   while distinct dates with the same model time retain their slots and
   dated carry scales. Permit origin only when explicitly selected and
   qualified by the caller. Remove every-node schedule inference and the
   legacy mutable schedule and weight-cache paths.
   [Discrete Asian payoff references](../../../REFERENCES.md#discrete-asian-payoff-and-settlement).

5. **Supply actual historical observations.** Accept an explicit ordered
   collection of already observed contractual levels. Apply the historical
   contract conversion at the owning product before passing them in.
   Prepare immutable checked state from these levels. Retire independently
   supplied sum, log-sum and count. Derive `n` from all historical and
   scheduled rows with checked counts and allocation. Require
   `1 <= n <= 2^53` so the denominator is exact in binary64, subject
   to the engine's actual resource checks. Allow a fully observed contract
   with empty future rows. Reuse the prepared historical state on every
   path and every paired leg.

6. **Average contractual levels by count.** The included levels `Z_i` are
   the supplied historical levels and `m_j*S_j` for future rows. Compute
   `A=sum(Z_i)/n` or, for positive geometric levels,
   `G=exp(sum(log(Z_i))/n)`. The multiplier changes the observed level.
   Every slot retains weight `1/n`, irrespective of the sum of the
   multipliers. Arithmetic admits signed finite inputs and zero. Geometric
   admits positive inputs and literal zero, with a zero-product state if
   any level is exactly zero. A negative geometric level errors, even
   after a zero. This preserves absorbing-zero dividend paths where the
   product permits zero. The lognormal analytic and geometric-control
   routes retain their strictly positive domain. Apply the signed quantity
   to the call or put intrinsic of the completed average.
   [Accepted dividend jumps](06-cash-dividend-jumps.md#accepted-decision-for-checked-discrete-dividend-jumps-on-exact-grid-knots),
   [accepted analytic Asian domain](../models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing).

7. **Consume each eligible Asian node once and require full coverage.** Read
   the single finite named spot at the required node and process all its
   contractual rows atomically. Reject skipped, duplicate or out-of-order
   required node callbacks. A nonfixing node neither reads spot nor changes
   the average. Reset the cursor and running state to checked history for
   each leg. Final value is available only after every declared slot,
   including every tied row, is included. A fully observed Asian can
   evaluate directly from its history. Continued validation after a zero
   geometric factor or zero quantity preserves event coverage and errors.

8. **Check the complete payoff arithmetic before converting to Money.**
   For terminal differences and arithmetic Asian averages, use checked
   scaled products and ordered compensated accumulation. Carry the scale
   through the count, strike difference and quantity so an intermediate
   product or raw sum outside binary64 does not by itself invalidate a
   representable required result. Preserve a resolved near-strike gap
   before applying positive part. For positive geometric levels, sum
   `log(spot)+log(multiplier)` in fixed order and retain the genuine zero
   branch separately. Use a stable strike-relative log difference and
   `expm1` near positive strikes, with scaled evaluation as needed.
   A negative or zero strike uses the signed difference and exact support
   branches. Reject unresolved sign or nonzero amounts lost to numerical
   range. Avoid epsilon clamps, artificial zero prices and nonfinite
   inputs to intrinsic. Convert a computed final amount once through
   the accepted checked Money adapter. A digital returns its fixed Money
   amount or exact zero in the run currency after checking currency on
   both branches. Payoffs are undiscounted single settlement amounts.
   Apply the caller's qualified positive payment factor once under the
   engine and Money rules, without implicit FX or minor-unit rounding.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
   [checked summation](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).

9. **Give diagnostics their own availability.** Expose final settlement
   through complete-only `value()`. An optional terminal or Asian payoff
   snapshot becomes available after completion. If a caller requests an
   intermediate Asian average, report it as a separate observed-only
   diagnostic with seen and total slot counts, absent before the first
   included level. Its denominator is the seen count. Capture must leave
   draws, callbacks, price and error outcome identical. Record 28 owns
   diagnostic request, representation and range availability. Remove
   the intermediate calls to final `value()` from capture. LRM needs
   the required path states and final payoff, so stop forcing
   intermediate payoff snapshots for it.
   [Accepted capture boundary](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [capture record](28-capture-and-simulation-contracts.md).

10. **Bind the geometric control to the same payoff law.** Keep paired
    arithmetic target and geometric control on one simulated path per
    leg. Require positive admitted history and a qualified positive
    GBM law for the control. Match history, exact future rows and
    multiplicities, carry scales, volatility and covariance, strike,
    direction, quantity, currency and payment discount in the exact
    geometric control mean. An origin row must enter the control
    mean with its deterministic level when qualified. Reject an
    explicitly requested incompatible control or pooled Sobol pairing.
    A checked plain arithmetic route admits signed history. Control
    coefficient, training/bias and uncertainty belong to record 27.
    [Accepted analytic Asian contract](../models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing),
    [accepted QMC boundary](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics),
    [control variate record](27-control-variate.md).

11. **Reconcile live pricing, Greek and host callers.** Use the checked
    terminal payoff through GBM, Heston, rough equity and commodity
    callers within their accepted model and estimator domains. Use the
    checked Asian contract through the existing GBM, Heston QE and
    geometric-control routes and the existing Python/WASM pricing
    operations. Require explicit Asian averaging, fixing rows and history
    and explicit engine controls at the covered convenience boundary.
    Keep low-level payoff callbacks Rust-only. Reconcile the existing
    schemas, docs, declarations, examples and parity ledger. LRM is
    eligible only if payoff dependence on the differentiated input is
    through the sampled density. Reject direct dependence from an
    origin observation or a differentiated fixing multiplier for
    that estimator. Keep the score law and final discounted scoring
    with record 26. Product owners retain calendars, fixing publication
    and history coverage, pricing measure, currency, settlement timing
    and model selection. A fully observed dated claim needs its
    remaining settlement treatment without a new live spot or vol
    request. [LRM record](26-likelihood-ratio-greeks.md),
    [asset Asian product](../valuations-fx-and-exotics/asset-asian.md).

12. **Qualify payoff and priced routes separately.** Verify signed linear
    settlement and call/put parity, strict digital ties including atoms,
    missing and duplicate event failures, origin and exact distinct knots,
    repeated rows and their order, fully observed/reset paths, geometric
    zero and negative inputs, mixed-scale and near-strike arithmetic,
    payment timing, currency errors and capture transparency. Check
    single-fixing equivalence to the terminal payoff and an independent
    seasoned geometric benchmark with ties and nonunit multipliers.
    Independently qualify product cashflows, control matching, sample
    uncertainty and host results under their own contracts.

## Observed behavior and limitations

[Terminal payoffs](../../../../finstack-quant/models/src/monte_carlo/payoff/vanilla.rs)
initialize and reset terminal spot to zero. An early put or forward
valuation can use that value before the required observation. Eligible
events require finite named spot, while repeated callbacks overwrite
it. Terms are public and some nonfinite values can bypass the intended
payoff test. Digital comparisons are strict and forwards already allow
signed cashflows.

[Asian payoffs](../../../../finstack-quant/models/src/monte_carlo/payoff/asian.rs)
store a historical sum, log-sum and count. A hash map caches multiplicity,
sum of multipliers and sum of their logarithms at each node. Arithmetic
uses Kahan accumulation. Geometric sends every nonpositive spot to the
same zero-average branch. A partial `value()` divides by observed count,
and reset restores the supplied aggregates. Current convenience
[pricing and LRM](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs)
select arithmetic and infer fixing steps `1..=num_steps`. Current
[capture](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs)
calls `value()` at intermediate nodes.

The [dated GBM route](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
pairs target and control on one path and applies per-fixing carry
multipliers. The [Heston route](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/heston_mc_pricer.rs)
also applies dated multipliers. The shared fixing mapper uses a
`1e-10` matching tolerance. The grid and mapper can merge or conflate
distinct nearby times. The existing product files own remaining dated
pricing decisions.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Terminal `S=-5`, `K=F=-10`, `q=-2` | European call and forward each settle `-10`. Put settles zero. Both cash digitals settle zero on a separate path with `S=K=-10`. An unobserved terminal put errors on final valuation. |
| `S=1.2e308`, `K=-1.2e308`, `q=1e-300` | Call is approximately `2.4e8`. Direct binary64 subtraction overflows before applying the quantity. |
| History `[64,81]`, shared spot `120` with multipliers `0.75,1.25`, then spot `80` with multiplier `0.5` | Five levels `[64,81,90,150,40]` give arithmetic `85` and geometric about `77.519640597338`. At strike `80` and quantity `2`, arithmetic call is `10` and geometric put is about `4.960718805324`. An explicit payment factor `0.95` gives arithmetic PV `9.50`. |
| Arithmetic spots `[MAX,-MAX,3]` with scales `[2,2,1]` | The average is `1` and the call at strike `0.75` with quantity `2` is `0.5`. Direct products overflow and can hide the residual. |
| One geometric fixing `S=10^16+2`, strike `10^16` and unit scale and quantity | The call is `2`. Independent binary64 evaluation with a strike-relative `log1p` and `expm1` retains that gap even though the separately rounded logs are equal. |
| Geometric history `[0]`, then a required negative spot | Missing the spot leaves the payoff incomplete. Observing the negative spot errors. With a valid zero or positive remaining spot the completed average is exactly zero. |
| Model times `0.5` and `0.50000000005` | Two distinct stored knots. Equal model times share a knot while preserving all rows. |
| Ideal GBM geometric control, history `[80]`, dated forwards `[100,130,120]`, times `[0.5,0.5,1]`, volatility `0.24`, strike `105`, quantity `2` and `D=0.96` | Independent joint-normal covariance gives log variance `0.018`, mean average `105.125592400155`, discounted call `10.909740085677` and put `10.668602677380`. |

Existing fixed-path tests exercise terminal cases, repeat scales,
historical reset and geometric comparisons. The independent examples
check the proposed formulas and edge cases. Runtime and product
qualification remain pending.
[Primary references](../../../REFERENCES.md#discrete-asian-payoff-and-settlement).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
