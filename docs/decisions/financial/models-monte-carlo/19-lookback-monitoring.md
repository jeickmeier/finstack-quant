# MODELS-MONTE-CARLO-19: Lookback extrema and conditional lognormal bridge sampling

- Decision status: **Accepted — complete checked Monte Carlo lookback package, 2026-11-11**.
- Implementation status: Existing fixed and floating payoffs and dated asset GBM pricing observed. Checked monitoring, extremum arithmetic, completion and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent payoff, bridge and price examples reviewed. Runtime, host and dated product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo lookback extrema

1. **Use one checked fixed/floating lookback contract.** Require call or
   put, an explicit fixed strike or floating selection, finite signed option
   quantity, a maturity node, monitoring and qualified initial history
   state. Keep only the relevant extremum and share checked state between
   the four payoffs. Admit a finite fixed strike in the underlying price
   unit. The generic discrete payoff admits finite signed levels and zero
   when the product and process do. The log bridge requires strictly
   positive monitored endpoints and a positive supplied historical extremum.
   Dated product and analytic domains may be narrower. Validate immutable
   terms and any wire restoration before draws. Retire mutable terms and
   the setter-order dependency between monitoring and bridge volatility.
   [Accepted terminal payoff domain](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs),
   [accepted analytic lookbacks](../models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums).

2. **Define the four payoff and observation roles.** Let `M` be the
   eligible maximum, `m` the eligible minimum, `S_T` the floating
   payoff's separate contractual terminal fixing and `Q` the quantity.
   Fixed call pays `Q*max(M-K,0)`, fixed put `Q*max(K-m,0)`,
   floating call `Q*max(S_T-m,0)` and floating put
   `Q*max(M-S_T,0)`. The terminal fixing enters the extremum only
   when maturity is monitored. A floating spread can be adverse
   with an unmonitored terminal fixing, so the positive part is
   contractual. When maturity is monitored, inclusion makes the
   floating spread nonnegative. Fixed terms need no terminal spot
   beyond the eligible extrema.
   [Fixed and floating payoff references](../../../REFERENCES.md#sampled-lognormal-lookback-extrema).

3. **Select exact discrete observations or a continuous bridge.** A discrete
   contract specifies its strictly increasing stored-node indices at or
   before maturity. Retain exact date-to-knot mapping, including explicit
   origin or maturity eligibility. A dated owner may combine only
   economically identical observations at the same node and phase.
   Preserve distinct model times and reject a mapping that conflates
   different fixing data. A continuous contract uses the exact stated
   start node through maturity, both inclusive. At start consume only
   its own observation. A singleton continuous window has one level
   and no bridge cells. Remove the continuous label for node-only
   monitoring and the automatic every-node schedule.
   [Accepted time grid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted barrier observation binding](18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).

4. **Make historical coverage and the empty case explicit.** Require a
   qualified prior maximum/minimum or an explicit fresh window with
   no earlier eligible observations. The dated owner supplies the
   relevant cutoff and qualifies which observations were included.
   An eligible new observation can advance a strictly prior extremum.
   Check an already included current fixing against through-current
   history. The qualified start spot anchors the first continuous
   bridge even when its observation is already covered. A current
   quote outside monitoring supplies no extremum evidence. A discrete
   schedule with no remaining observations is
   valid with certified history, including for a fixed payoff that
   is fully determined. A floating payoff still needs its terminal
   fixing. Reject missing required history and a contract with neither
   history nor eligible observations. Represent absence explicitly,
   without infinite sentinels or a synthetic zero.

5. **Require complete, atomic observation processing.** Bind the full
   monitoring and any terminal fixing to the exact grid and terms
   before draws. Consume each required node once in chronological
   order using named, finite node-local spot. Reject a skipped,
   duplicate or incompatible event. Non-observation nodes need spot
   only when they provide the required floating fixing. In bridge
   mode validate the endpoint, actual cell and reserved draw before
   committing the new extremum and previous endpoint together. Reset
   to admitted history with no prior cell endpoint or terminal
   fixing for each leg. Final value requires all applicable events,
   including for zero quantity. A fixed payoff can complete as soon
   as its last eligible observation or certified history fixes the
   extremum. The dated owner still determines when payment is due.
   [Accepted engine events](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

6. **Bind continuous sampling to the qualified GBM cell law.** Use one
   positive single-factor exact lognormal path with constant relative
   volatility and constant log drift within each monitored cell.
   Reuse model-owned volatility and actual width `h` to prepare
   integrated variance `V=sigma^2*h`. Bind every represented
   carry-slope change inside monitoring to an exact knot. Use the
   same stored grid for the path and bridge. Regenerating a
   numerically close grid or integrating across an unrepresented
   carry break supplies no qualified bridge. This law gives the exact
   ideal conditional marginal extremum in each cell. Extrema on
   disjoint cells are conditionally independent given the GBM endpoints.
   The payoff requests only one extremum. Joint minimum and maximum
   or joint barrier and lookback events need a separately qualified
   coupling. Local, Heston, rough, jump and multifactor bridges
   require their own crossing and extremum laws. The barrier-specific
   Heston approximation does not extend to lookbacks. Exact-node
   discrete observations retain the broader qualified process domain.
   Apply no continuity shift to either selected monitoring mode.
   [Accepted GBM law](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [conditional extremum law](../../../REFERENCES.md#sampled-lognormal-lookback-extrema).

7. **Sample one anchored extremum without cancellation.** Let
   `L,R` denote the endpoint spots, `d=abs(log(R/L))`,
   `w=sqrt(2*V*(-log(U)))` and `g=(hypot(d,w)-d)/2`.
   Evaluate the positive excursion as
   `g=w^2/[2*(hypot(d,w)+d)]` in scale. The sample is
   `max(L,R)*exp(g)` for the maximum or `min(L,R)*exp(-g)`
   for the minimum. Use stable log ratios, including `log1p`
   for adjacent endpoints. At literal zero cell variance return the
   appropriate actual endpoint, preserving it exactly. The deterministic
   log path is affine in the admitted cell. Use checked scaled geometry
   at positive variance, including when `V` or the raw excursion
   rounds to zero by itself. The selected positive excursion places
   the ideal extremum strictly beyond the corresponding endpoints.
   [Bridge inversion references](../../../REFERENCES.md#sampled-lognormal-lookback-extrema).

8. **Preserve the required payoff scale.** Retain the selected extremum
   with its endpoint anchor and signed log excursion or equivalent
   checked scaled representation. Resolve comparisons against other
   cells, history, strike and the terminal fixing in that scale. Preserve
   near-strike differences with a checked log ratio and `expm1`
   where applicable. Carry scale through the positive part and signed
   quantity. Standalone rounding of the extremum must not erase a
   representable final amount. An unresolved sign or unrepresentable
   required amount errors. Convert the final undiscounted amount once
   to checked Money. The engine applies the independently supplied
   positive payment factor once, including when it exceeds one.
   Payment date and carry law are separate owner inputs. Preserve
   exact zero without floors, implicit FX or minor-unit rounding.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

9. **Reserve draws by monitored cell.** Reserve one present, node-local
   strictly open uniform per bridge receiving node, including zero
   variance. Discrete nodes and the first continuous node use no
   extremum draw. Keep draw addresses independent of path outcomes
   and capture. Reuse the accepted Philox midpoint, replay and
   antithetic `(U,1-U)` rules. Compatible Sobol GBM uses its
   independently addressed Philox auxiliary stream. Gaussian bridge
   ordering for Sobol does not select contractual monitoring. Reject
   absent, stale or invalid draws. Retire endpoint uniforms and
   probability floors. Exact bridge statements concern ideal draws;
   the finite Philox midpoint profile bounds the sampled tails.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation),
   [accepted Sobol pricing](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

10. **Keep lifecycle and market authority with the dated product.**
    Reconcile the existing asset GBM route to explicit monitoring
    inception or qualified history, exact remaining observations,
    common model clock and owner-qualified carry and payment inputs.
    Reuse stored knots when preparing carry, or merge independent
    carry breakpoints exactly. Refinement of a fixed carry law and
    resampling the curve are distinct experiments. A collapsed live
    clock requires its own product treatment. Resolve known claims,
    including fully observed fixed lookbacks, before unnecessary
    spot and volatility queries. Require the contractual terminal
    fixing for floating settlement, and for a fixed payoff only
    when it completes an eligible observation. Product owners retain
    fixing source, payment, residual-claim and scenario semantics.
    The analytic continuous price keeps its accepted separate domain.
    [Dated asset lookback](../valuations-fx-and-exotics/asset-lookback.md),
    [accepted analytic lookbacks](../models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums).

11. **Reconcile capture, Greeks and existing host routes.** Keep final
    settlement value complete-only. Any exposed in-flight extremum
    is a diagnostic under record 28, with absent and unrepresentable
    diagnostics distinct from a genuine value. A snapshot must not
    change draws, event coverage, final value or errors. For
    density-only LRM require discrete positive-node observations
    and any needed terminal fixing, with fixed history and terms
    independent of the differentiated input. Origin and sampled
    extrema are outside that LRM contract. Preserve product-owned
    finite-difference and historical observation rules. Keep low-level
    callbacks Rust-only and reconcile the existing Python/WASM
    instrument pricing envelopes, docs, diagnostics, schemas and
    parity. The analytic lookback host operation retains its own owner.
    [Capture](28-capture-and-simulation-contracts.md),
    [LRM](26-likelihood-ratio-greeks.md).

12. **Qualify the payoff, simulation law and product independently.**
    Verify all four payoff cases with and without maturity monitoring,
    missing history, complete known claims, singleton and delayed
    windows, exact carry splits, atomic failures and reset. Compare
    conditional extrema with independent reflection probabilities
    and continuous prices with independent payoff integration under
    the common analytic domain. Check zero and tiny positive
    variance, mixed scale and near-strike cases, seeded replay, and
    host and capture parity. On the same path, a superset of
    observations weakly increases all four unit payoffs. Refining an
    admitted GBM bridge grid with the same carry law preserves the
    ideal payoff distribution. Independent finite draws do not
    imply pathwise agreement across grids. Qualify model, dated
    sources and sampling uncertainty separately.

## Observed behavior and limitations

The [payoff](../../../../finstack-quant/models/src/monte_carlo/payoff/lookback.rs)
has separate fixed/floating types with infinity sentinels for missing
history and a zero-initialized floating terminal spot. The low-level
default continuous label samples nodes only. A separate bridge setter
enables interval sampling and resets the monitoring start to zero.
The inverse formula accepts endpoint uniforms, floors zero, and
stores its next endpoint before validating the whole cell. A zero
variance still reconstructs the endpoint through logarithm and
exponential. Final value does not require complete event coverage.
[Engine capture](../../../../finstack-quant/models/src/monte_carlo/engine/simulation.rs)
currently calls that payoff value at intermediate nodes.

The [dated asset GBM pricer](../../../../finstack-quant/valuations/exotics/src/exotics/lookback_option/pricer.rs)
does explicitly enable the bridge for continuous MC. Its observed
carry helper builds knots separately from the path and the
dated monitoring helper uses tolerant matching. Continuous history
can fall back to an unseeded extremum, and the expiry and collapsed
clock branches query terminal spot for both payoff types. These
are observed routing choices. The proposed contract preserves
the separate analytic and MC model owners.
[Carry helper](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs),
[dated monitoring](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/monitoring.rs).

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Observations `90,110`, `K=100`, unmonitored terminal `120` | Unit fixed call `10`, floating call `30` and floating put `0`. With terminal monitoring the fixed call becomes `20`. |
| Certified history maximum `125`, no future observations, `K=100`, `Q=-2` | Fixed call settles `-50` without a terminal spot. A unit floating put with the same maximum and terminal fixing `105` settles `20`. |
| Endpoints `80,120`, `V=0.04`, `U=0.5` | Separate marginal minimum and maximum quantiles are approximately `77.5029791881` and `123.8662061841`. Substitution into each reflected tail recovers `0.5`. |
| Endpoints `1,2`, `K=2`, `V=2^-120`, `U=0.5`, `Q=2^120` | The maximum rounds to `2` in binary64. The anchored log excursion is about `3.7615819e-37` and the checked fixed call amount is approximately `1`. |
| Deterministic `S=K=100`, zero yield, `D(0.5)=0.80` and `D(1)=0.95` with qualified log-linear segments | The maximum is `125`. Fixed call PV is `23.75` and floating put PV is `18.75`. Flattening carry from `D(1)` gives `5` and `0`. |
| Independently regenerated knots for `T=1` and ten steps | `3*(T/10)` gives `0.30000000000000004`. `(3*T)/10` gives `0.3`. Exact bridge preparation must use the stored knots and actual carry breaks. |
| `S=100`, `K=105`, historical max/min `120/80`, `T=1`, carry `0.03`, `sigma=0.2`, `D=exp(-0.05)` | Independent integration over the conditional extremum and terminal Gaussian gives fixed call `19.5704972562`, fixed put `25.5039576654`, floating call `23.6447354235` and floating put `21.4297194981`. |

Existing source tests cover fixed paths, history, bridge reset and a
named continuous grid/seed comparison. The examples check the
proposed payoff and ideal bridge law. Runtime behavior and dated
market and host qualification remain pending.
[Accepted analytic benchmarks](../models-analytic/lookback-extrema.md#independent-bounded-reference-checks).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
