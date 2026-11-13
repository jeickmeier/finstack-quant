# MODELS-MONTE-CARLO-21: Cliquet local/global clamps and seasoned return aggregation

- Decision status: **Accepted — complete checked Monte Carlo cliquet payoff package, 2026-11-11**.
- Implementation status: Existing Rust payoff and dated asset GBM route observed. Shared replay, exact event binding, checked arithmetic and dated-owner reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent reset, accumulation and price examples reviewed. Runtime, host and dated product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo cliquet payoffs

1. **Require explicit immutable cliquet terms.** Select the additive or
   multiplicative accumulation mode, the full nonempty period-end
   schedule, finite local and global bounds, and an exact signed
   Money notional in the payment currency. Positive notional is long.
   Validate all relevant terms before execution or a zero-amount
   shortcut. Restore terms through the same checks. Remove the
   additive default, unchecked mutable terms and implicit initial
   anchor. The unit payoff is a long option amount paid once, with
   principal and per-period cash outside this selected contract.
   [Cliquet contract examples](../../../REFERENCES.md#cliquet-reset-and-accumulation-contracts),
   [accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

2. **Distinguish the strike-setting fixing from a period end.** Start
   with a qualified positive finite contractual anchor. A qualified
   strike-setting event can establish that anchor at inception and
   contributes no period return, even with a positive local floor.
   A period end always uses the preceding contractual fixing as its
   anchor. Preserve that anchor when the live model starts from a
   different current spot. Require the actual strike-set fixing for
   a trade whose strike has already been set, including before its
   first period end. The dated owner distinguishes event roles and
   eligibility. A date match or an omitted initial level supplies
   no inference about the role of an observation.

3. **Bind period ends to exact stored nodes.** Require strictly
   ordered distinct contractual period ends. In the dated route
   period ends are at or before payment. Bind the live suffix and
   all model coefficient breaks to the full accepted grid. Future
   dated resets need strictly positive, distinct model times.
   An expressly qualified origin event uses its own event role.
   Reject missing, skipped, duplicate and incompatible required
   events, including two distinct future dates collapsed by the
   model clock. Remove time tolerances, repeated endpoint reuse
   and end-of-grid catch-up. Non-reset nodes need no payoff spot.
   [Accepted exact grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted Monte Carlo events](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

4. **Replay qualified history with the live reset transition.** Require
   a validated observed fixing for every eligible historical period
   end, ordered after strike setting. A same-day period end that
   meets the owner's information cutoff locks a real return against
   its preceding fixing. Require an observed value from the
   qualified fixing source. Resolve pre-publication or prospective
   cases through an explicit owner route. Retire generic substitution
   of valuation spot for missing fixings. Reject duplicate or missing
   required observations. Replay and simulate through the same
   checked transition, with one accumulator for the selected mode,
   the last anchor and the consumed prefix. Keep caller-supplied
   unchecked sum/growth seeds outside the contract. Commit each reset's
   anchor and accumulator atomically, and restore the validated
   historical state on every path and antithetic leg.
   [Accepted information cutoff](../conventions/0045-financial-clocks-and-date-boundaries.md).

5. **Compute each clipped simple return in the original geometry.**
   Require finite positive fixing levels and finite local bounds
   `-1 <= L <= C` in both modes. For previous fixing `A` and
   current fixing `S` the contractual return is `r=(S-A)/A`
   and the locked return is `l=min(C,max(L,r))`. Near par retain
   the actual difference before division. Compare against local bounds
   in scale so an unbounded ratio that would overflow does not hide
   a finite cap. Preserve exact equality and the actual clipped or
   unclipped branch. The next period anchors to `S` even when
   clipping bound. No tolerance or intermediate rounding changes
   the contractual branch.

6. **Add the clipped returns in chronological order.** Additive
   starts with zero and uses the sum of all `l` across the historical
   and simulated periods. Keep signed contributions and use checked
   scaled compensated accumulation. A large intermediate sum may
   later cancel. Apply no global floor or cap while accumulating.
   Preserve a resolved sign and return an error on unresolved
   required arithmetic.
   [Accepted checked sums](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).

7. **Compound the clipped gross factors.** Multiplicative starts at
   gross growth one, uses `G=product(1+l)` and takes aggregate
   return `G-1` at completion. For an unclipped period retain
   `S/A` directly. Use a checked scaled gross product or compensated
   log growth with stable `log1p` and `expm1` on their applicable
   branches. Preserve the telescope across consecutive unclipped
   periods. A truly zero factor from a clipped return of `-1`
   makes growth zero. A positive factor stays positive in scale,
   even when a standalone simple return rounds to `-1` or raw
   growth would overflow or underflow. Keep validating required
   fixings after an exact zero. Apply no early global clamp.
   [Multiplicative contract reference](../../../REFERENCES.md#cliquet-reset-and-accumulation-contracts).

8. **Apply the global limits once to the whole contract.** Use finite
   `0 <= L_g <= C_g` for this long-option unit payoff, aligning the
   low-level and dated domains. The amount per unit is
   `min(C_g,max(L_g,aggregate_return))`. The nonnegative global
   floor implements the option's zero bound. Reject negative global
   floors instead of accepting an inert parameter. Apply the bounds
   in scale before materializing a standalone aggregate growth or
   return. Pay the exact applicable bound on a resolved
   capped/floored branch, and preserve literal zero. Convert the
   final scalar through the accepted Money adapter and scale the
   signed Money notional once. Required conversion, range or
   unresolved arithmetic failures error.
   [Accepted Money transforms](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

9. **Finalize the same payoff when no periods remain.** Require complete
   observation coverage before final value, including on a zero
   notional. After the last contractual reset the return is fixed.
   A seasoned trade with no future resets applies the same global
   limits to its locked state. Value this known amount without a
   dummy path, new terminal spot or new volatility lookup. Reject
   an empty underlying contract and an incomplete claimed history.
   The last reset date fixes the return. Later payment does not
   add a reset or a new observation.
   [Accepted completed payoff boundary](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

10. **Discount one eligible payment on its actual date.** Return the
    option cash amount at the explicit payment date. Apply one
    positive factor from the qualified dated discount source,
    allowing factors above one. The dated owner supplies an
    explicit same-day cash selection and pending, paid or discharged
    status. An admitted payable same-day leg has factor one. A
    known unpaid claim retains discount risk until its actual payment
    under the owner's dated treatment. Model-time collapse or a
    past payment date alone does not settle cash. Qualify any overdue
    claim before discounting it. Keep the payment clock and reset
    clock distinct.
    [Accepted dated cash selection](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting),
    [dated cliquet owner](../valuations-equity-and-composition/14-cliquet.md).

11. **Reconcile the live ATM-term GBM and fixed-budget route.** Use
    the accepted checked ATM variance and carry profile on one exact
    checkpoint and event grid. End simulation at the last required
    live reset. The current market spot starts the model while the
    last contractual fixing anchors the next return. Keep deterministic
    payment discount separate. Use the existing fixed-budget Philox
    route, explicit controls and selected antithetic pairing. No
    payoff uniforms are required. A requested precision gate runs
    after the fixed budget. Product owners retain market-source
    mapping, risk and model qualification, including the limits of
    ATM term volatility for forward-start return options.
    [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
    [dated GBM owner](../valuations-equity-and-composition/12-path-gbm.md).

12. **Reconcile diagnostics, risk and existing hosts.** In-flight
    anchors and locked returns can be optional capture diagnostics.
    Capture leaves event coverage, draws, value and errors unchanged.
    Preserve contractual anchors and actual observed fixings in
    bumped pricing. A hypothetical theta observation is a separate
    product scenario. Reconcile dated payment sensitivity against
    the actual discount clock, including known cash after the final
    reset. Keep payoff callbacks Rust-only. Reconcile existing
    Python/WASM instrument envelopes, schemas, docs, parity and
    estimator diagnostics through Rust. Keep Greek estimators and
    risk uncertainty with their owners.
    [Capture](28-capture-and-simulation-contracts.md),
    [finite-difference Greeks](25-finite-difference-greeks.md).

13. **Qualify the payoff and dated route independently.** Check
    strike setting, same-day and past reset eligibility, exact
    event coverage, anchor versus live spot, signed returns, both
    accumulation laws and the order of local and global bounds.
    Include near-flat, zero-factor and mixed-scale cases, locked
    history with no future resets, payment lag, same-day and
    outstanding cash, fixed-budget replay and host parity. Check
    one-period prices against an independent capped call spread
    or endpoint integral. Separately qualify multi-period prices,
    dated market mapping and risk under applicable term sheets
    and distinguish price/model error from sampling uncertainty.

## Observed behavior and limitations

The [payoff](../../../../finstack-quant/models/src/monte_carlo/payoff/cliquet.rs)
defaults to additive and uses mutable terms. The `1e-6` tolerance
allows one node to supply multiple crossed resets. The payoff reads
spot at every callback and stores the reset spots. It replays them
at final value, maintains independently seeded historical sum and
growth, and returns zero before inspecting either seed when no
reset spot was recorded. Local bounds and aggregate limits are
applied in the selected branch. The low-level long-call floor
allows an otherwise negative global floor.

The [dated pricer](../../../../finstack-quant/valuations/equity/src/cliquet_option/pricer.rs)
separately replays past observations and already has a fully observed
cash branch. It looks up current spot before replay, uses current
spot for a missing same-day fixing, can infer the initial anchor
from current spot and can return zero on a collapsed live model
clock. Its payoff horizon ends at the last live reset while discount
runs to payment. It filters or tolerance-deduplicates times when
preparing the grid. The dated validator rejects negative global
floors and some nonfinite terms can evade ordering comparisons.
The [rho calculator](../../../../finstack-quant/valuations/equity/src/cliquet_option/metrics.rs)
also uses the instrument year fraction in its payment eligibility
guard. The [shared pricing driver](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs)
still forwards optional width-driven stopping.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Strike-set anchor `100` followed by one flat period at `100`, local floor `2%` | Lock `2%` once. Treating strike setting as a period fabricates another `2%`. |
| `100→120→114` with local bounds `[-10%,10%]`, global bounds `[0,20%]`, notional `1000` and `D=0.95` | Locked returns `10%,-5%` give additive PV `47.50` and multiplicative PV `42.75`, including when all resets were observed. A global floor `6%` gives `57` in either mode. |
| Last contractual anchor `120`, live model spot `110`, next fixing `114` | Period return is `-5%` before clipping. Replacing the anchor by live spot gives about `+3.63636%`. |
| `100→120→108` with local bounds `[-10%,20%]` and global cap `15%` | Final additive return `10%` and multiplicative return `8%`. Applying the global cap after the first reset changes both results. |
| Anchor `1.5`, next fixing the adjacent larger binary64 value, local cap `2e-16` | The difference-based return is about `1.480297366e-16`. Dividing first and subtracting one instead gives about `2.220446049e-16` and falsely activates the cap. |
| `1→2^-60→2`, local bounds `[-1,2^61]` and global bounds `[0,2]` | The unclipped gross factors telescope to `2`, so multiplicative return is `1`. Rounded simple return and add-back can fabricate a zero factor. |
| `2^-600→1→2^600→2^-599`, local bounds `[-1,2^601]` and global bounds `[0,2]` | Gross growth is `2` and return is `1` despite overflow and underflow in naive intermediate products or ratios. |
| Multiplicative local floor and cap both `-1`, global floor `0.02`, global cap `0.15`, notional `1000` and `D=0.95` | Gross growth is exactly zero. The global floor still applies and PV is `19`. |
| Anchor `100`, current spot `105`, carry `0.03`, volatility `0.20`, one reset in `0.5` years, local `[-0.20,0.10]`, global `[0,0.30]`, notional `1000` and independent `D=0.96` | Either mode gives PV about `49.4275418442`. Independent normal endpoint integration agrees with the discounted call spread on `100` and `110`, scaled by `1000/100`. |
| Two unseasoned GBM periods with cumulative carry increments `0.015,0.0225` and variance increments `0.02,0.03`, local `[-0.15,0.10]`, global `[0.02,0.13]`, notional `1000` and `D=0.96` | Independent conditional lognormal integration gives additive PV about `50.0346192243` and multiplicative PV about `49.6823696420`. |
| Fully observed cash `50`, valuation January 30 and payment January 31, instrument 30E/360 and ACT/365F discount at `5%` | The instrument model time is zero. The dated PV is about `49.993151154` and a `+1bp` discount bump changes PV by about `-0.00001369675`. |

The independent examples exercise contractual, numerical and
price distinctions. Existing Rust tests cover local/global bounds,
history, delayed cash and rho. Runtime, host and dated market
qualification remain pending.
[Reference cases](../../../REFERENCES.md#cliquet-reset-and-accumulation-contracts).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
