# MODELS-NUMERICAL-003: Tree discounting, OAS conversion and backward induction

- Decision status: **Accepted — complete package, 2026-11-11**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Source and callers inspected. Independent Decimal discount and OAS comparisons and exact rational rollback checks were executed. Production, external price and risk qualification remain open.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Accepted decision for checked tree discounting and rollback

1. **Use the accepted compounding contract at each outgoing interval.**
   Admit explicitly selected Continuous, Simple or Periodic(m) base
   quotes under CORE-NUMERICS-01. Discount by exp(−r*h),
   1/(1+r*h) or (1+r/m)^(−m*h), respectively, on the
   departing node and its interval. Use the accepted checked
   forward, inverse and known-rate conversions. Remove the tree's
   duplicate floors, epsilon-zero handling and raw-rate fallbacks.
   Signed rates, factors above one and positive subnormal factors
   are valid in the applicable domains. Preserve any stricter rate
   domain owned by the selected model.
   [CORE-NUMERICS-01](../core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads)

2. **Use one prepared model clock.** The selected lattice owns its
   increasing node times, positive outgoing interval widths and
   horizon. Calibration, forward state prices, base discount,
   OAS conversion and rollback use the same widths and topology.
   A homogeneous model uses its declared mathematical step and
   checked canonical labels with the exact horizon. An event-aligned
   model uses its prepared actual widths. Product and curve owners
   supply dated targets and resolve day counts and mandatory
   events. Quote conversion uses the same economic interval.
   [Exact grid ownership](../core-numerics/time-grids.md)

3. **Require the OAS unit and quote basis explicitly.** A tree
   request admitting OAS uses a signed annual decimal spread s
   and selects Continuous or Periodic(m) independently of the
   base compounding. Convert fractional bp at the checked unit
   boundary, with 100 bp = 0.01. Reject nonfinite spreads and
   invalid frequencies. Simple remains a base discount convention.
   A Simple OAS quote is unsupported in this tree contract.
   Validate supplied quote basis even at zero spread. Existing
   products can restrict the model-level choices further.
   [CORE-CONVENTIONS-018](../core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision)

4. **Apply Continuous OAS as one additive discount shift.**
   With base log discount L=log(D) and interval h, the effective
   shift is delta=s and shifted log discount is L−s*h, regardless
   of base compounding. At exact zero spread preserve the admitted
   base factor directly. Do not form an unused implied rate on the
   zero-spread path or use the base compounding to discount s.

5. **Apply Periodic OAS to the node quote.** Let c be the
   equivalent annual continuous base rate on the interval.
   Periodic(m) restates the node base in that convention, adds s
   there and converts the result back. Compute the equivalent
   continuous shift with the accepted stable definition
   `delta=m*log1p((s/m)*exp(-c/m))`, and apply
   `log(D_shifted)=L-delta*h`. Derive c from the known base
   quote when available. For a factor-only base use `c=-L/h`.
   Require a strictly positive shifted periodic base at every
   needed node. A constant quoted s can yield different local
   shifts. This is the selected nodewise convention.
   [Tree discount and OAS references](../../../REFERENCES.md#checked-tree-discounting-and-rollback)

6. **Check the complete discount computation.** Preserve available
   log information when discount factors round near one. Use
   checked arithmetic and equivalent complete expressions when
   detached powers, shift multipliers or rate round trips would
   lose scale. Required discount factors must be positive and
   finite. Preserve a valid factor above one and ordinary output
   rounding. Reject invalid domains or unresolved required
   quantities without substituting zero, the base DF, a floor or a
   cap. OAS changes applicable discounting. Calibrated dynamics,
   branch weights and base projection remain those of the selected
   model.

7. **Validate the selected lattice's complete geometry.** Require
   one root, matching levels, admissible node state and the
   model's exact owned branching. Validate every stored branch
   and child index against the next row, including the shifted
   edges of the capped BK tree. Require finite nonnegative
   weights in [0,1], with their checked sum within eight
   binary64 epsilons of one. Model-specific support and
   normalization requirements still apply. Use the same admitted
   weights for pricing and forward state prices. Do not skip
   invalid children, renormalize during rollback or replace
   missing states with zero.

8. **Use the departing-node discounted expectation.** Set
   `C(n,i)=D_shifted(n,i)*sum_b p(n,i,b)*V(n+1,child_b)`
   or use the base DF for a request with no OAS. Evaluate the
   ordered signed sum and discount with checked scale and
   cancellation handling. Preserve a representable signed
   continuation across term underflow/overflow where possible.
   Return a numerical error when the required result cannot
   be resolved. Share the scalar rule with the existing rollback
   paths and keep model-owned binary, trinomial and joint topology.
   [Tree rollback references](../../../REFERENCES.md#checked-tree-discounting-and-rollback)

9. **Keep payoff and event ordering in the callback.** Initialize
   terminal values from the fallible payoff callback. At an
   earlier node pass its already discounted continuation to the
   fallible node callback. The owner applies the appropriate
   current-node cash, exercise, conversion, coupon, survival
   or recovery treatment and returns the complete local value.
   Do not discount it again at that node. Finite signed callback
   values and exact zero are valid. Preserve the accepted vanilla
   and cash-event rules where they apply.
   [Vanilla lattice](equity-binomial-construction.md),
   [dividend phases](dividends-and-lattice-exercise.md)

10. **Make callback state reflect the applied interval.** Expose
    the node identity and model time, along with genuine spot
    or model state where applicable. For a departing interval,
    distinguish its positive width and any required raw signed
    base quote and basis, base DF, applied DF and effective
    continuous OAS shift. A required callback DF is the checked
    factor used by rollback. Keep the input OAS quote in its
    original basis and remove the overloaded quote/shift field.
    Credit callbacks that require the applied DF propagate its
    absence as an error. Numerical paths that consume only the
    complete discounted expectation may retain its scale
    without materializing a standalone factor.

11. **Give the terminal node no outgoing interval.** A terminal
    callback can receive genuine terminal model state, such as
    spot or a centered factor, and any separately qualified
    dated observable. It has no departing width, calibrated
    interval rate, interval discount or interval-converted OAS.
    Remove held-last, zero-base and synthetic terminal-rate
    substitutes. Where a model supports a terminal-only request,
    validate the direct inputs and invoke the terminal payoff once.
    No discount inversion is performed at zero time.

12. **Keep off-grid cash and terminal projection with their owners.**
    Exact terminal cash is valued at that node. If cash or
    exercise is folded across different dates, the product must
    supply a qualified dated valuation and spread rule or reject
    the case. A local OAS shift and prior interval rate provide no
    automatic post-grid discount. Retain genuine Hull–White
    conditional bond projection under record 007. A raw signed
    joint rate stays distinct from any credit/payoff transform.
    Record 010 owns joint survival, recovery, effective hazard
    and reset-coupon ordering.
    [Hull–White event lattice](hull-white-event-grid-and-affine-bonds.md),
    [rates-credit coupon folding](rates-credit-coupon-folding.md)

13. **Bound work and propagate errors in a defined order.** Use
    checked row, node, edge and allocation sizes and an explicit
    positive pricing node budget. Account for the actual stored
    geometry before evaluation. Use two adjacent value
    rows or equivalent scratch bounded by the maximum width.
    Visit each stored terminal node once, then descending time
    levels and increasing stored node indices within each level.
    Propagate callback, malformed lattice, discount and numerical
    errors with useful node/interval context. A failed request
    returns no partial premium. Per-model calibration budgets
    and transactional publication remain with their owners.

14. **Reconcile calibration and zero-spread pricing together.**
    Use the same admitted base discount and transitions for
    Arrow–Debreu propagation, fit objectives, zero-coupon
    checks and rollback on Ho–Lee, BDT, BK and Hull–White.
    Reconcile Hull–White Bermudan exercise-policy diagnostics
    with the same checked continuation as pricing. Individual
    geometry, calibration fit tolerances, diagnostic acceptance,
    and affine projection remain with their model records.
    [Ho–Lee](ho-lee-curve-fit.md), [BDT](bdt-lognormal-curve-fit.md),
    [Black–Karasinski](black-karasinski-trinomial.md),
    [Hull–White](hull-white-event-grid-and-affine-bonds.md)

15. **Use the same spread law for direct PV, inversion and risk.**
    Reconcile callable bond and term-loan lattice and applicable
    deterministic routes with the checked interval operator.
    Search only in the selected quote's admitted domain across
    required nodes. Use fallible root objectives and the product's
    residual gate. Propagate invalid or failed repricings without
    price sentinels. Preserve existing product basis restrictions
    and require explicit rejection of unsupported inputs.
    Quote targets, settlement basis, credit adjustments and
    bump sizes remain at their owners.
    [Fallible Brent](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search),
    [bond spread quotes](../valuations-bonds/12-spread-quotes.md),
    [term-loan pricing](../valuations-other-fixed-income/08-term-loan-pricing.md)

16. **Reconcile existing hosts and qualify the selected use.**
    The runtime lattice and callbacks stay Rust-only. Reconcile
    the actual product and risk paths, schemas, host docs and
    parity with Rust validation. Qualify signed and negative-rate
    cases, nonuniform intervals, callable/exercise ordering,
    shifted BK edge branches, terminal cash timing, valid and
    invalid spreads, and the affected rate and OAS sensitivities.
    Use independent path enumeration or state prices for the
    finite tree and an external price reference on the same
    clock and spread convention where available. Report separate
    model, grid and numerical limitations. In particular, a
    maturity-based conversion to a single continuous spread and
    the nodewise periodic convention can produce different PVs.

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| Simple base .12, interval .75, Continuous OAS +200 bp | Base DF ≈ .917431192660550459. Shifted DF ≈ .903772421654185928. Independently applying a Simple spread multiplier gives ≈ .903873096217291092. |
| Same base and spread with Periodic(2) OAS | Nodewise addition gives shifted DF ≈ .904589738016402835. Independently compounding the spread gives ≈ .903839758570250827. |
| Continuous base −.5, interval .25, Periodic(2) OAS | Base DF ≈ 1.133148453066826317. The admissible annual decimal spread is strictly greater than ≈ −1.557601566142809736. A spread of −1.55 passes and −1.56 fails. |
| Known continuous base and OAS both −2^−54 over one year | The two separate factors each round to one. The combined known-rate factor rounds to the next binary64 number above one. |
| Binary two-step signed payoff, branch weights (.25,.75), root DF .95, second-interval DFs (1.1,.9), terminal values (−120,60,180), local cash (−25,7) and root cash −3 | Earlier node values are (−8.5,142). Backward induction and independent forward state prices both give 96.15625. |

The examples check arithmetic and the finite-tree operator. They leave
model calibration, independent callable pricing and end-to-end production
and host qualification open.

## Observed implementation and evidence

### Choice and algorithm

[tree_framework.rs](../../../../finstack-quant/models/src/trees/tree_framework.rs) prices binary recombining claims with V(n,i)=df(n,i)·{p_up V(n+1,i+1)+p_down V(n+1,i)} before invoking the product callback. Terminal values come entirely from the callback. Spot nodes are S0 d^n (u/d)^i and use flat exp(−r dt); short-rate nodes and discount rates are supplied by closures. The callback owns coupon, recovery and exercise ordering beyond that continuation calculation.

`TreeDiscounting` delegates to core compounding: continuous exp(−r dt), simple 1/(1+r dt), or periodic (1+r/m)^(−m dt). Simple and periodic bases below 1e−15 are floored at that level. The inverse returns zero with a warning when |dt|<machine epsilon or DF≤0. Continuous conversion is −ln(df)/dt; degenerate dt returns the input rate and nonpositive DF logs and returns the raw rate.

### Selection, scope and conventions

[short_rate_tree/pricing.rs](../../../../finstack-quant/models/src/trees/short_rate_tree/pricing.rs) uses equal binary probabilities for Ho–Lee/BDT and its own capped trinomial rollback for BK. OAS input is basis points. Convert the base node rate to continuous, then ask `oas_compounding.continuous_spread_shift(base,s/10000)` for the additional continuous rate. The callback receives that effective continuous OAS in basis points when the quote is noncontinuous. BK discounts by base convention DF times exp(−shift dt); binary models supply the equivalent combined continuous rate. This preserves the distinction between adding a spread to a periodic base rate and independently compounding a spread.

### Failure behavior and alternatives

Uncalibrated trees and malformed lattice geometry fail before pricing. Binary closure out-of-range zero fallbacks are guarded by geometry validation in this caller. BK skips invalid child indices; normal calibrated geometry should supply all branches. Terminal rates are populated without a post-maturity fit and may be used by terminal callbacks. Floors avoid invalid discount bases but materially alter extreme-rate economics; rejection instead is an unimplemented policy alternative. Compounding variants are supported, and their adoption per product remains separate.

The Hull–White bond helper applies a continuous OAS with the base
compounding's factor. Its current registered base is continuous. The
helper differs for a noncontinuous base, as the arithmetic above shows.
Callable bond paths also use interval-derived terminal shifts for
cash displaced from its actual date. The joint rates-credit callback
exposes a floored rate while discount and projection use the raw rate.
Bond and term-loan OAS objectives currently substitute a large positive
residual for failed pricing. Bermudan swaption exercise diagnostics
run a separate infallible discounted-expectation loop.
[Bond rollback](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/bond_valuator.rs),
[bond OAS objective](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/oas.rs),
[rates-credit callback](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/pricing.rs),
[term-loan pricing and risk](../valuations-other-fixed-income/08-term-loan-pricing.md),
[Hull–White swaption valuation](../valuations-rates/08-hull-white-swaptions.md)

### Evidence

[short-rate tests](../../../../finstack-quant/models/src/trees/short_rate_tree/tests.rs) include `tree_compounding_rate_from_df_inverts_df` (roundtrip) and `periodic_oas_is_added_to_each_node_rate_on_its_own_compounding` (independent deterministic formula). Framework state-handoff tests verify callback fields. [Hull](../../../REFERENCES.md#hull-options-futures) supports discount conventions, not the floor thresholds. Model calibration is separately recorded in records 004–007.

## Follow-up tracking

Implementation reconciliation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
