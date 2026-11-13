# MODELS-MONTE-CARLO-23: LSMC exercise fitting, replay and statistical interpretation

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing standalone Rust and Python/WASM GBM LSMC routes observed. Exact exercise binding, independent pricing, policy identity and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent exercise, sampling and GBM price examples reviewed. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked holder exercise policy training and pricing

1. **Scope the contract to one holder stopping decision.** Value a
   unit-quantity cash-settled call or put on one GBM spot. Require
   finite positive initial spot and strike, a selected call or put,
   one explicit currency, and a positive-horizon model grid. Exercise
   earns the positive part of `S-K` or `K-S` on the exercise node
   and terminates future option cash. The dated owner supplies
   exercise eligibility and settles any already known claim.
   Physical delivery, payment lags, jump dividends, stochastic
   rates and issuer exercise use their own qualified routes.
   [Accepted terminal payoff](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

2. **Bind explicit exercise rights to exact stored knots.** Require
   a nonempty strictly increasing list of unique stored-node indices
   that explicitly includes the final knot for this vanilla contract.
   Reject a missing maturity, duplicate or invalid index, or
   incompatible dated mapping. Permit origin exercise only when
   selected. An every-step American convenience values exercise on
   the selected grid, with an explicit origin choice. It is a
   discrete approximation of continuous exercise. Keep the actual
   stored times, including on irregular grids. No snapping, implicit
   maturity addition or inferred origin right.
   [Accepted exact grid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots).

3. **Use exact GBM and checked exercise-time discounting.** Reuse the
   accepted single-factor exact lognormal transition on actual
   adjacent intervals. Require finite nonnegative constant relative
   volatility and explicit flat carry or an admitted bounded
   cumulative carry schedule. The model admits positive states.
   Require finite signed continuously compounded discount rate `r`
   on the same model-year clock. Discount from `j` to `i` using
   `exp(-r*(t_j-t_i))` on stored node times. A standalone flat-market
   adapter may map its declared rate and dividend yield to net
   carry `r-q`. The low-level owner qualifies the relationship
   between carry and discounting. Preserve scale through the
   realized cash and discount, including negative rates and
   literal zero cash. Errors propagate without default levels,
   dropped paths or unqualified clock conversions.
   [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

4. **Specify run controls and bound actual work.** Require explicit
   basis identity and its relevant configuration, training and
   pricing budgets, seed, execution and pairing choices wherever
   used. Require at least two base groups in a used training or
   pricing stage and at most ten million per stage. Retain the
   100,000-interval and 64,000,000-retained-spot-value limits.
   Check the actual physical trajectory counts, stored event rows,
   basis and work allocations, aggregate address ranges and byte
   layouts before draws. Use fallible reservation and ordered
   errors. The two phases need no simultaneously retained path
   matrices. Remove registry pricing defaults. Native execution
   must preserve row and reduction order, and current WASM
   rejects an unsupported parallel request.
   [Accepted run controls](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

5. **Assign disjoint Philox groups to training and pricing.** Use
   one explicit root seed with training base IDs `[0,N_train)` and
   pricing IDs `[N_train,N_train+N_price)` for the composed run.
   With antithetics both legs stay in the same base group and stage.
   Preserve exact stream and step draw order. Standalone replay
   uses the frozen root seed and an explicit pricing interval
   disjoint from the frozen policy's training interval.
   Reuse of a pricing interval is replay and supplies no new
   independent sample. Distinct addresses provide the selected
   pseudorandom partition. Statistical interpretation uses ideal
   independent base-group draws. A price path is never used for
   training, transforms, policy selection or the origin decision.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

6. **Fit later-policy cash backward on strictly in-the-money legs.**
   Initialize the selected cash and time to maturity intrinsic,
   including zero. At each earlier positive-time exercise date,
   form the response from the actual later selected cash discounted
   back to this decision node. Fit on training legs with strictly
   positive intrinsic. Use only current spot information in the
   selected basis and keep rows in base-group and leg order.
   Each eligible leg contributes one equally weighted row. Use
   the fitted decision to update that path's selected cash and
   time before stepping backward. Future realized cash is a
   regression response. A pricing decision only uses information
   available at its node.
   [Longstaff–Schwartz method](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing).

7. **Freeze and report a continue-only rule for sparse dates.** For
   `p` fitted basis columns, count distinct training base groups
   with at least one eligible leg. With fewer than `max(2,p)`
   groups, including zero, freeze continue-only at that date.
   Retain the contractual right and report the date, eligible
   group and leg counts and rule used. An antithetic pair supplies
   at most one group to this gate. Sufficient coverage requires
   a checked full-rank fit under the selected regression contract.
   A malformed or numerically failed admitted fit errors with
   its date and context. Remove the extra-ten threshold, implicit
   basis downgrade and realized-future exercise fallback. The
   sparse rule deliberately restricts this fitted holder policy
   and can reduce value. Numerical basis and solver choices
   belong to record 24.
   [Regression record 24](24-regression-policies.md).

8. **Exercise only on strict positive improvement.** At a fitted
   node exercise iff intrinsic is strictly positive and exceeds
   the checked fitted continuation in decision-time payoff units.
   Continue at equality. Preserve finite signed predictions and
   the actual comparison. An out-of-the-money leg continues even
   under a negative prediction. A continue-only node always
   continues. Use this rule in training rollback and forward
   pricing. Apply no continuation clipping or artificial time
   smoothing.

9. **Choose origin exercise from training alone.** For an eligible
   origin, compare its positive intrinsic with the mean time-zero
   PV of continuing under the learned later rules on training
   groups. Use equal antithetic pair means when paired. Freeze one
   origin decision for all pricing legs, exercising only on strict
   improvement. If origin is ineligible or tied, continue. A
   frozen origin exercise fixes the amount conditional on that
   policy, so return it with actual training work and zero pricing
   paths. Pricing paths do not revisit that decision.
   [Fixed policy valuation](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing).

10. **Bind the complete immutable policy and replay it forward.**
    Freeze the actual knot sequence, exact exercise nodes, payoff
    side and strike, initial spot, currency, GBM carry and
    volatility, discount law, basis identity and transforms,
    training provenance, origin action, per-date rules and
    coefficients. Require compatible full inputs and checked
    ordered rule shapes at replay. Matching counts alone is
    insufficient. Keep the policy Rust-owned without public
    mutation or unchecked reconstruction. Price chronologically
    using the first exercise selected by the frozen rule and
    otherwise the admitted maturity cash. Keep numerical errors
    contextual to stage, group, leg and node. Retain exact
    execution provenance and publish no partial estimate.

11. **Resolve structural cases directly.** At literal zero
    volatility, evaluate all eligible discounted intrinsic
    amounts on the checked deterministic path. Select the largest
    with ties retaining the later date. Report the known amount
    with no training or pricing paths. Tiny positive volatility
    uses the stochastic law. With only a maturity right at
    positive volatility, price the maturity cash on the declared
    pricing group range with no regression or training draws.
    Full relevant input validation still applies. A deterministic
    outcome without a sample has no sample-based uncertainty
    estimate. Show its actual work and outcome reason.

12. **Report independent policy value and conditional uncertainty.**
    For sampled pricing, use one checked time-zero Money PV per
    leg, its checked numeric view for statistics, and one
    observation per base group, averaging complete antithetic
    pairs. Keep the fixed-sample mean, sample standard error and
    95% Student-t interval of the accepted engine. Report actual
    training and pricing groups and physical paths, the origin
    action and per-date fit and coverage diagnostics. Conditional
    on an admissible frozen policy and ideal draws, pricing
    estimates that policy's value. Its population value is at
    most the optimal holder value on the same grid and law.
    A learned policy's population value can be below immediate
    exercise or maturity-only value. Sampling error can place a
    reported price on either side of the optimum. The interval
    describes pricing sampling only, excluding training, policy,
    grid and model error.
    Remove in-sample pricing, intrinsic or European floors on
    reported estimates, and the `unbiased` naming. Training
    outcomes never enter pricing statistics.
    [Accepted fixed-sample inference](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
    [holder policy bound](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing).

13. **Reconcile the existing standalone and product boundaries.**
    Use one two-pass `price_american` operation on the existing
    standalone Rust/Python/WASM convenience, with explicit grid
    controls, origin eligibility, basis and work settings. Keep
    the exact-schedule fit and replay through their Rust owner.
    Reconcile existing schemas, declarations, stubs, errors,
    examples, replay fields and parity. Host layers transport
    exact seeds and return canonical diagnostics. Keep the
    dated equity option's exercise and settlement state, model
    selection and risk with its product. Hull–White issuer
    training and callable-bond or rates exercise retain their
    own economics and fit admission.
    [Dated equity option](../valuations-equity-and-composition/06-option-lifecycle.md),
    [accepted issuer LSMC](../models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing).

14. **Qualify the policy and price independently.** Check exact
    nodes and mandatory maturity, sparse and malformed fits,
    antithetic group counting, strict ties and frozen origin,
    deterministic and terminal-only reductions, identity and
    stream rejection, cash scaling, replay and host parity.
    Compare single-intermediate-date prices to independent
    conditional integration and multi-date examples to qualified
    exercise-grid benchmarks. Test basis, training-size and
    grid dependence separately from held-out sampling error.
    Exercise-set monotonicity concerns optimal values on a
    common model. Independently fitted policies may differ.
    Upper-bound and continuous-exercise error qualification
    require their own evidence.

## Observed behavior and limitations

The [Rust pricer](../../../../finstack-quant/models/src/monte_carlo/pricer/lsmc.rs)
generates uniform GBM rows. It initializes maturity intrinsic
regardless of the supplied exercise list, reconstructs times
from the step count, and skips positive-time fits with at most
basis count plus ten in-the-money rows. Time-zero training
exercises at equality. The ordinary price reports in-sample
values and can replace its mean and interval using intrinsic
while retaining stochastic error from the original sample.
The two-pass convenience requires a different pricing seed.
Direct frozen replay checks only step count and basis width
and does not enforce a disjoint training partition. The public
policy fields can be mutated.

The [Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/pricers.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs)
standalone wrappers expose both pricing paths and optional
registry defaults. Native parallel paths are supported. The
current WASM path executes serially even when parallel is
requested. The dated equity option dispatcher has its own
exercise and settlement contract.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Same ITM node, immediate payoff `5`, later cash `0` or `24` with equal probabilities, discount to the decision `1/2` and decision-to-origin `1/2` | Correct decision continuation is `6`, so hold on both paths and value `3` at origin. A rule that sees the realized later cash produces `4.25`. |
| Origin intrinsic `5` and two training continuation PVs sampled from `0,12` with equal probabilities | The in-sample maximum has expectation `7.25`. The independently priced fitted policies have average population value `5.75` across training runs. The optimal value is `6`. |
| Origin intrinsic `5` and training continuation PVs `[0,10]` | Continue on equality. A held-out sample of `[0,0]` yields zero without flooring. |
| Two antithetic legs in one eligible base group with a two-column basis | One group falls below `max(2,2)`. Freeze continue-only and report both leg and group counts. |
| Grids `[0,0.2,1]` and `[0,0.8,1]`, `r=log(2)`, intermediate exercise cash `7` and later cash `10` | Conditional continuation values are about `5.74349` and `8.70551`. Matching grid length cannot justify policy reuse. |
| Zero-volatility put, `S0=80`, `K=100`, carry `-0.20`, `r=0.03` and rights at `0,0.5,1` | Discounted payoffs at `0.5` and `1` are about `27.2019024478` and `33.4818651546`. Exercise at `1` without path sampling. |
| Put `S0=K=100`, `r=0.05`, yield `0.02`, vol `0.20`, exercise at `0.5` and `1` | Independent conditional integration gives optimal grid value `6.4896326729` and intermediate boundary about `88.0936957607`. Maturity-only value is `6.3300806275`. A frozen rule exercising below `95` at `0.5` is worth `6.3075620932`. |

These examples check the selected stopping, sampling and discount
semantics. Existing tests cover selected bounds, seed checks and
time-zero behavior. Runtime, host and financial qualification
remain pending.
[Primary references](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
