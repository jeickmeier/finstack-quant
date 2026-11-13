# MODELS-MONTE-CARLO-18: Barrier observation, Brownian-bridge crossing and rebate timing

- Decision status: **Accepted — complete checked Monte Carlo barrier monitoring and rebate package, 2026-11-11**.
- Implementation status: Existing barrier payoff, asset GBM/Heston and FX GBM routes observed. Checked monitoring, first-passage timing, rebate discounting and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent crossing, first-passage and price examples reviewed. Runtime, host and product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo barrier monitoring and rebates

1. **Require explicit checked barrier terms.** Admit one flat barrier, a
   European call or put, an explicit up/down knock-in/out type, an explicit
   monitoring method, and a fixed maturity observation node. Reuse the
   terminal intrinsic and signed option quantity contract. The discrete
   comparison accepts finite signed same-unit barrier and spot levels when
   the process and product admit them. A log bridge requires strictly
   positive barrier and endpoint spots. Payable rebates have their own
   cash amount, entitlement and timing. Validate immutable terms and any
   restoration before draws, including explicit zero amounts and known
   limits. Remove mutable payoff terms and silent monitoring and timing
   defaults.
   [Accepted barrier classification](../core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing),
   [accepted terminal payoffs](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

2. **Bind the observation contract to the complete exact grid.** For live
   discrete monitoring require an explicit nonempty, strictly increasing
   list of stored nodes at or before maturity. Include maturity only when
   contractually eligible. A dated owner can combine equivalent
   observations only when the node, observable and observation phase are
   identical and contractually interchangeable. Distinct model times remain
   distinct. A continuous window includes its explicit start and maturity.
   At the start test only its own level. The preceding interval is outside
   monitoring. Subsequently use only actual adjacent monitored cells. Use
   named node-local state at eligible observations, with the owner establishing
   observation phase and date, including any eligible origin. An origin-only
   payoff can resolve directly. The generic pricing engine retains its positive
   horizon contract. Validate the complete binding before sampling.
   [Accepted exact grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

3. **Latch inclusive touch and preserve observation provenance.** At eligible
   nodes up hits at `S>=H` and down at `S<=H`. Validate the relevant inputs
   before testing either inequality. A first recognized hit is irreversible
   within the path. Record an exact node for node observations and a cell
   for a sampled crossing. Record a first-passage time for an explicitly
   timed bridge rebate. Consume every required event once in chronological
   order, even when entitlement is already fixed. Reject missing,
   duplicate, stale or incompatible inputs. Unmonitored nodes need no
   spot for the barrier. Maturity requires spot when it is itself a
   monitoring node or when earned intrinsic needs it.

4. **Qualify the single-factor GBM bridge cell law.** Use the unshifted
   contractual level and exact lognormal endpoint process. For the
   selected GBM profile, relative volatility and log drift are constant
   on each monitored cell. Bind actual width `h` and model-owned
   `V=sigma^2*h` in the required numerical scale. A piecewise-linear
   cumulative carry schedule supplies constant log drift on each segment.
   Require exact knots at its actual slope changes within monitoring.
   Any qualified piecewise-constant volatility profile also binds its
   breaks. The conditional log bridge is exact under these ideal cell
   laws. A general time-changed bridge needs log drift affine in its
   variance clock. Constant endpoints alone cannot justify it. Keep
   discrete observations on qualified paths, including accepted post-jump
   observations where applicable. Exclude crossing bridges across jumps,
   and generic local-volatility, rough and multifactor bridge selections.
   Continuity-shifted pricing of a discrete contract belongs to its own
   method. Apply no displacement inside a sampled bridge.
   [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [bridge references](../../../REFERENCES.md#sampled-log-barrier-crossing-and-first-passage).

5. **Make the Heston bridge a named approximation.** Preserve Heston QE-M
   discrete monitoring without any bridge-variance dependency. Admit a
   continuous Heston request only when it explicitly selects and qualifies
   the log bridge using the QE-M step's model-owned integrated variance
   proxy `I`. Reuse `I=h*(v_left+v_right)/2` for positive vol-of-vol,
   and the accepted deterministic variance integral at literal zero
   vol-of-vol. Use the actual interval, named process output and checked
   full scale. Remove receiving-endpoint variance substitution and scalar
   sigma fallback. The conditional crossing and sampled hit time approximate
   the Heston law, including when varying deterministic variance makes
   drift nonaffine in the variance clock. The bridge omits stochastic
   variance and leverage within the cell. Apply the same documented
   interpolation limit at `I=0`. Qualify the approximation for the
   product's domain, grid and rebate timing before admitting it.
   [Accepted Heston QE-M](09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).

6. **Sample crossing in checked log geometry.** For a safe left endpoint,
   let `a>0` be its oriented log distance from the barrier and `b` the
   signed safe-side distance of the right endpoint. For an up barrier
   use `log(H/S)`, and for a down barrier use `log(S/H)`.
   If `b<=0`, a continuous crossing is certain. For two safe endpoints
   with positive cell variance `V`, `p=exp(-2ab/V)`. With an open
   uniform `U`, hit exactly when `2ab<V*(-log(U))`. A resolved exact
   tie misses. Use scaled products and stable log ratios, including
   `log1p` for adjacent levels. Do not form a rounded probability for
   the comparison. Remove the exponent cutoff, clamping and epsilon
   rules. At literal zero model cell variance use the affine deterministic
   log path, which misses when both endpoints are safe. A positive
   variance lost to intermediate underflow does not select that branch.
   The finite Philox midpoint law has its own resolution. Its smallest
   draw is `2^-53`, so `p<=2^-53` is unreachable for that generator.

7. **Sample the first passage for an at-hit bridge rebate.** Use the same
   uniform that selected crossing. Conditional on a hit its rescaled
   target is `u=U/p`, with `p=1` when the right endpoint touches or
   breaches. This target is uniform under ideal continuous draws.
   The Philox midpoint profile gives a finite approximation. Let
   `c=abs(b)` and `R=(t-left)/(right-t)` for an interior
   model time. Under the cell bridge, the conditional first-passage CDF
   is `Phi((c*R-a)/sqrt(V*R))+exp(2*a*c/V)*Phi(-(c*R+a)/sqrt(V*R))`.
   This includes the right-touch limit `2*Phi(-a/sqrt(V*R))`.
   Compute the target and its complement with checked log differences.
   Evaluate the CDF or survival on its smaller scale. Use the equality
   of reflected density factors to evaluate the reflected term through
   a scaled normal tail, and a resolved difference for survival. At
   `c=0` use the normal/erf limits. Use bounded bisection in the
   ordered nonnegative binary64 encodings of the two stored node times
   to find the first representable interior time whose conditional CDF
   reaches the target. There are at most 64 midpoint probes. Treat cell
   endpoints as exact CDF zero and one. Reject unresolved comparisons
   or a sample that can only be represented as a cell endpoint. This
   is a directed binary64 time selection under the checked CDF.
   For zero cell variance and a reached right endpoint, the deterministic
   crossing occurs after width `h*a/(a+c)`. Resolve an interior time
   distinctly and preserve exact right touch at the right node.
   At the first eligible node a touch pays at that
   node. A guaranteed crossing at a stochastic right endpoint still
   samples the interior time. Never use the receiving node as a proxy
   for continuous first passage.
   [First-passage law](../../../REFERENCES.md#sampled-log-barrier-crossing-and-first-passage),
   [checked normal tails](../core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains).

8. **Reserve uniforms by exact monitored cell.** Reserve one present,
   node-local open uniform at every active bridge receiving node,
   even after a prior hit, on certain hits and at zero variance.
   Discrete observations and the first monitoring node consume no
   bridge uniform. Reuse the accepted Philox replay and `(U,1-U)`
   antithetic rule. Reset all payoff and draw state for each leg.
   Compatible Sobol GBM uses its separately addressed auxiliary Philox
   stream. The Sobol Gaussian bridge ordering has no contractual
   monitoring effect. Reject missing, stale, invalid or unused
   conflicting inputs. The first-passage inversion adds no draws.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation),
   [accepted Sobol pricing](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

9. **Separate activation, rebate entitlement and amount.** Knock-in earns
   intrinsic on a hit. Knock-out earns intrinsic if no hit occurs.
   Select absent cash, knock-out hit cash at the first hit or at the
   contractual expiry payment reference, or knock-in failure cash at
   that reference. A no-hit rebate at hit is invalid, including an
   explicit zero. Retain the total nonnegative rebate as exact
   currency-tagged Money, separate from the option quantity. Check its
   currency before branching and avoid the per-unit rebate round trip.
   Use the accepted complete terminal intrinsic arithmetic. Dated
   owners establish the cash amount and disposition of the full claim.

10. **Discount early cash using the same qualified law as settlement.**
    Keep the primary engine's single positive payment factor `D(P)`
    for the equivalent settlement reference `P`. For a rebate `R`
    paid at a live hit `t`, prepare `R*D(t)/D(P)` as the equivalent
    amount, then apply `D(P)` once through the engine. Require both
    factors from one deterministic owner-qualified discount source and
    clock. Use exact eligible node factors for discrete hits. For a
    sampled interior hit, bind an explicit piecewise-linear log-discount
    law in bridge model time with its breakpoints on stored knots,
    or its flat-rate special case. A dated owner qualifies that law
    and owns any curve approximation. A terminal factor alone supplies
    no intra-cell discount law. Check positive factors, signed rates,
    scaled ratio and accepted Money arithmetic. The required equivalent
    amount must fit the engine's Money contract. Different settlement
    lags, stochastic discounting and separately dated flows need a
    qualified product route.
    [Accepted engine discount](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
    [accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

11. **Keep historical hit and payment status with the dated owner.**
    Require explicit fresh-window or eligible no-hit history for a
    live barrier. Resolve a previously activated knock-in, already
    knocked-out claim or fully observed window before live simulation.
    Require actual recognized event and payment evidence for pending,
    settled or discharged cash. A recorded hit alone cannot settle an
    at-hit rebate or become a new origin hit. A prior no-hit record
    leaves today's eligible observation open. Resolve known amounts
    before new spot or volatility lookups when possible. Keep lifecycle,
    calendar, payment date and market-data authority at the asset and
    FX owners.
    [Accepted dated barrier boundary](../models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash).

12. **Complete settlement and reconcile existing callers.** Reset to
    admitted history and require the maturity event and complete
    observation coverage before final payoff. Capture can expose
    a completed settlement value and qualified event provenance
    through its separate diagnostic path. Capture does not change
    monitoring, draws, settlement or errors. Retain density-only LRM
    eligibility for discrete positive-node observation with fixed
    terms and no direct differentiated-input dependence. Continuous
    bridge LRM remains outside the selected score contract.
    Reconcile asset GBM, asset Heston QE-M and FX GBM pricing and
    their existing instrument host payloads and diagnostics. Keep
    the payoff callbacks Rust-only. Keep analytic, PDE, credit
    barrier and lookback algorithms with their respective owners.
    [LRM](26-likelihood-ratio-greeks.md), [capture](28-capture-and-simulation-contracts.md),
    [asset barriers](../valuations-fx-and-exotics/asset-barrier.md),
    [FX barriers](../valuations-fx-and-exotics/fx-barriers.md).

13. **Qualify timing, price and approximation independently.** Verify
    exact event coverage, touch and direction, dormant and seasoned
    windows, nonaffine carry splits, actual cell widths, tiny hit
    probabilities, deterministic and stochastic timing, stable time
    inversion, currency and cash status, fixed replay and capture
    transparency. Check conditional CDF and sampled times against
    independent references and compare qualified GBM premiums and
    hit/expiry cash with continuous reflection and first-passage
    benchmarks. Verify in/out parity and discrete schedule
    monotonicity for no-rebate long claims. Measure Heston bridge
    and first-passage bias under refinement independently of path sampling error,
    including near barriers, zero variance and leverage stress.
    Qualify actual dated markets, rebate discounting, product price
    and host parity on each applicable route.

## Observed behavior and limitations

The [payoff](../../../../finstack-quant/models/src/monte_carlo/payoff/barrier.rs)
sorts and deduplicates discrete steps, uses inclusive endpoint touch,
and bridges later continuous cells. It uses the receiving node's
variance when present and a supplied scalar volatility otherwise.
The crossing code drops probabilities with exponent below `-20`.
The detected cell's right time is stored for at-hit rebate carry.
The payoff uses a supplied flat rate to compound the rebate to
maturity, resets historical breach to a time-zero flag and uses
zero-initialized terminal spot. An exported continuity-shift
coefficient is used by other owners and the sampled barrier bridge
does not shift its barrier.

The [asset GBM](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/pricer.rs),
[asset Heston](../../../../finstack-quant/valuations/exotics/src/exotics/barrier_option/heston_mc_pricer.rs)
and [FX GBM](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs)
paths share this payoff. The dated adapter uses a tolerance to map
monitoring times. Asset GBM supplies a datewise carry schedule, and
FX GBM uses terminal-implied flat carry. Asset and FX pass rebates
through per-unit scaling and can treat a past at-hit rebate as
discharged from the breach flag. Existing tests cover the old
interval, rebate, variance and touch behavior. Implementation
evidence remains separate from approval and price qualification.
[Dated monitoring adapter](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/monitoring.rs).

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Safe log distances `a=b=0.5` and `V=0.02` | `p=exp(-25)≈1.3887943865e-11`. The Philox midpoint `U=2^-40+2^-53` selects a hit. The old `-20` cutoff misses it. |
| Deterministic spot `100→120→100` on `[0,0.5,1]`, up barrier `110` | The first segment crosses. Treating `[0,1]` as one affine deterministic cell misses the interior carry change. |
| Heston width `0.25`, `a=b=0.1`, variance endpoints `0.01,0.09` | The proposed `I=0.0125` gives `p≈0.2018965180`. The legacy receiving-variance proxy gives `p≈0.4111122905`. Both are conditional approximations. |
| `h=1`, `a=0.2`, `c=0.1`, `V=0.04` | Safe right endpoint gives `p=exp(-1)`. On either sign of the right distance, the conditional half-cell CDF is about `0.4901383399` and the median is about `0.5070151217`. A breached right endpoint gives certain crossing and the same conditional time law. |
| `T=1`, initial up log gap `0.2`, zero log drift, `sigma=0.2`, `r=0.03` | Independent first-passage integration gives discounted hit factor `0.3123588378`. Right-endpoint discounting instead gives `0.3079325651`. |
| Rebate `10`, `D(hit)=0.80`, `D(P)=0.95` | Equivalent amount about `8.4210526316` gives PV `8.00`. Inferring flat rates from `D(P)` for a half-horizon hit instead gives `9.7467943448`. |
| Down barrier `90` with `S=K=100`, `T=1`, `r=0.05`, yield `0.02`, `sigma=0.2` | Independent integration over terminal spot and the conditional bridge gives no-rebate call in `1.6400515384` and out `7.5869539697`, summing to vanilla `9.2270055082`. |

The examples check ideal formulas and selected numerical
distinctions. Seeded runtime replay, numerical range, Heston
convergence, host and dated product prices remain pending.
[Accepted analytic benchmarks](../models-analytic/barriers-and-rebates.md#independent-bounded-reference-checks).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
