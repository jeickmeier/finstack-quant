# MODELS-MONTE-CARLO-22: Observation-count range accrual with history and carry multipliers

- Decision status: **Accepted — complete checked Monte Carlo range accrual package, 2026-11-11**.
- Implementation status: Existing Rust payoff and dated asset GBM route observed. Complete observation binding, checked coupon arithmetic and caller reconciliation are pending.
- Validation status: Source, selected tests, issuer terms and independent count, dated carry and price examples reviewed. Runtime, host and dated product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo range accrual

1. **Require immutable single-coupon terms.** Specify an effective
   finite ordered band `L<U`, a finite nonnegative full-period
   coupon fraction `c`, exact Money notional `M`, and the entire
   eligible observation schedule with its qualified historical prefix.
   The generic payoff admits signed notional and zero, and
   finite signed observation levels in compatible units. The
   dated asset product retains positive notional and its positive
   lognormal levels. Validate relevant terms and restoration before
   shortcuts, including zero coupon or notional. Remove mutable
   term vectors and stale multiplier caches.
   [Issuer count and boundary examples](../../../REFERENCES.md#range-accrual-observation-counts-and-coupon-terms).

2. **Use equal contractual counts for one period.** For total
   scheduled count `n>0` and in-band count `k`, the amount is
   `M*c*k/n`. History and live rows enter the same fixed denominator.
   Each eligible row counts once regardless of time between rows.
   The low-level coupon fraction is the fraction of notional
   earned at full occupancy. For the dated asset convention the
   owner prepares the annual coupon times the accrual year fraction
   from accrual start to final contractual observation, once.
   Payment can fall later. Day weights, a different accrual end,
   continuous occupation and principal cash need their own terms.
   [Issuer coupon and accrual conventions](../../../REFERENCES.md#range-accrual-observation-counts-and-coupon-terms).

3. **Fix the band and its reference at the product boundary.** The
   payoff consumes the admitted band in observation units. The dated
   asset owner selects absolute bounds or relative bounds with an
   explicit positive contractual inception reference. Prepare the
   latter from that reference under the owner's checked quote
   geometry and any explicit contractual rounding. Reject an
   unresolved or collapsed effective band. Preserve the reference
   and qualified history during risk repricing. Current spot starts
   the model. It changes the band only in a distinct scenario
   that explicitly changes contract terms.
   [Accepted inclusive touch](../core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing).

4. **Bind every live row to an exact stored node.** Use one immutable
   row per contractual observation with its exact node and finite
   positive dimensionless multiplier `m_j`. Require nondecreasing
   node indices and retain the contractual order of ties. The
   dated asset schedule retains strictly increasing contractual
   dates. Distinct dates can share a positive model-time node only
   under a qualified common-state mapping, with distinct row
   identities, carry scales and membership tests. Distinct model
   times remain distinct. Validate the full grid and row binding
   before draws. An origin row needs explicit eligibility. A
   future date that collapses to the origin requires qualified
   owner treatment. Reject an unsupported clock or row mapping.
   Retire tolerance matching and skipped-date reuse.
   [Accepted exact grids](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted tied fixing rows](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

5. **Validate the historical tally against the exact prefix.** A
   certified total and in-band count are sufficient for this
   payoff when the dated owner binds them to the eligible prefix,
   cutoff, effective band and fixing basis. Require the historical
   count to equal the eligible prefix count, including zero.
   Require `0<=historical_in_band<=historical_count`. Reject an
   absent required tally, stale nonzero history for an empty
   prefix or overlapping history and live rows. A same-day
   observation within the cutoff requires qualified observed
   evidence. The owner qualifies any pending publication or
   historical reconstruction. Reset each leg to the validated
   counts and next live row.
   [Accepted fixing cutoff](../conventions/0045-financial-clocks-and-date-boundaries.md).

6. **Classify each tied group atomically at its own node.** Read
   one finite named `SPOT` at the required node and test each row
   with `L<=m_j*SPOT<=U`. Equality at either boundary qualifies.
   Compare the product in original scaled geometry so overflow,
   underflow or rounding to a boundary cannot change the result.
   A literal zero remains a level. Require valid spot and comparison
   even for zero economic amount. Reject missing, nonfinite,
   skipped, repeated or out-of-order required events. Commit all
   tied rows together after validation. Other nodes need no payoff
   spot. Require full contractual coverage before final value.

7. **Preserve the exact count ratio through Money conversion.** Use
   checked integer counts and verify `0<=k<=n`. Prepare the coupon
   fraction with the accepted checked scalar admission. Evaluate
   the exact admitted-Decimal notional and coupon times the integer
   fraction `k/n` with one final bounded Money rounding under the
   accepted nearest-even precision rule. A standalone rounded
   Decimal or binary64 ratio, full coupon amount or integer
   numerator need not fit if the final amount is representable.
   Preserve literal zero and full occupancy. Reject conversion,
   nonzero underflow, unresolved arithmetic and overflow errors.
   Retain notional currency and check it against the run.
   [Accepted Money precision and rounding](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

8. **Resolve complete cash on its actual payment date.** A fully
   observed nonempty schedule uses the same completed count and
   Money finalizer without live spot, volatility or quanto
   lookups. The dated owner supplies the contractual payment
   date at or after the last observation and resolves
   same-day cash inclusion, entitlement, paid status and
   overdue cash. An admitted same-day leg has factor one.
   Apply one positive deterministic factor to the included
   coupon, allowing factors greater than one. A past date alone
   supplies no discharge evidence. The observation and payment
   clocks stay distinct. An empty underlying schedule is invalid.
   [Accepted dated discounting](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting).

9. **Use the accepted single-factor GBM on the observation horizon.**
   The live asset MC route uses exact positive lognormal steps with
   one qualified constant relative volatility and an explicit
   carry schedule. Use a qualified constant override or an
   explicitly prepared surface quote at the last live observation
   horizon with its declared asset-price coordinate. The model
   ends at the last required live fixing. The payment date does
   not choose the diffusion horizon or volatility coordinate.
   Keep the term/skew limitations of a single volatility explicit.
   Qualify the dated financing and any quanto inputs under the
   accepted curve, measure, orientation and clock rules. Cash
   settlement uses the payoff currency and its actual discount
   factor. Model carry uses the qualified per-observation inputs.
   [Accepted GBM and bounded carry](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
   [accepted quanto orientation](../models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums).

10. **Preserve the individual dated forward on tied rows.** Prepare
    one qualified cumulative log growth `A_j` for each live
    dated row. At each unique stored time use the first contractual
    row's growth as the common GBM checkpoint. For each row at
    that node use `m_j=exp(A_j-A_common(t_j))` with checked scale.
    An unshifted row uses exactly one. The node's shared
    stochastic spot and each row's scale preserve its own dated
    expected level under the selected ideal GBM law. Reject
    missing carry data or unrepresentable required scales. This
    representation assigns perfect common-state dependence to
    tied rows. Qualify that assumption at the dated owner. Keep
    exact model knots and dated source queries distinct.

11. **Use complete paths as estimator observations.** Use accepted
    fixed-budget Philox pricing and the compatible explicit Sobol
    GBM mode under their own count and diagnostic contracts.
    Range tests need no payoff uniforms. Preserve one whole
    discounted amount per path, one paired mean per antithetic
    estimator or one mean per Sobol replicate. Observation rows are
    dependent within a path. A fixed deterministic discount
    and linear ideal coupon allow an independent price check
    from the sum of marginal in-band probabilities plus the
    historical tally. The sampling variance retains joint
    observation dependence, including exact ties. Keep partial
    tallies diagnostic and final value complete-only. Capture
    leaves draws, outcomes and errors unchanged.
    [Accepted pricing and inference](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
    [accepted Sobol](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics),
    [capture](28-capture-and-simulation-contracts.md).

12. **Reconcile existing asset routes and preserve owner boundaries.**
    Keep the low-level payoff Rust-only. Reconcile the existing
    asset GBM pricing, Python/WASM instrument envelopes,
    schema, docs, parity, diagnostics and finite-difference
    risk. Share qualified asset terms, counts, cutoff and
    completed dated cash with the static asset route. Smile
    digital approximation, spread width and static model
    choice remain with that owner. Callable rate range accrual
    owns tenor-forward fixings, exercise, principal and
    stochastic discounting. Preserve those contracts even
    where the terms type is shared. Other day-weighted and
    multi-period products map their economics at their owners.
    [Asset range accrual](../valuations-fx-and-exotics/asset-range-accrual.md),
    [callable rate range accrual](../valuations-fx-and-exotics/callable-range-hw.md).

13. **Qualify counting, dated carry and pricing independently.**
    Verify both boundaries, tied-node multipliers, history
    coverage including an empty prefix, missing state and
    reset, checked mixed-scale comparisons and exact count
    arithmetic. Test qualified tied-date carry, unsupported
    origin collapse, stable reference under bumps, known and
    same-day payments, replay and host parity. Compare ideal
    prices with independent marginal CDF or quadrature
    calculations, including deterministic boundary atoms.
    Check the actual fixed-budget inference on whole path
    amounts, and qualify dated carry, model and product prices
    separately from sampling uncertainty.

## Observed behavior and limitations

The [payoff](../../../../finstack-quant/models/src/monte_carlo/payoff/range_accrual.rs)
stores dates and multipliers separately and consumes dates with a
`1e-6` year tolerance. A coarse step can evaluate multiple skipped
dates on its receiving spot. Missing spot defers a required
observation. Nonfinite spot can enter the consumed count without
qualifying. Final value divides by consumed plus historical counts
and returns zero for an empty consumed state. Equal dates can carry
distinct multipliers.

The [dated asset pricer](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs)
uses a single volatility at the payment horizon and datewise carry.
It already preserves distinct dated forwards when model times tie.
Its static and MC branches classify live dates differently when
the model clock collapses. The [shared terms](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs)
require distinct ordered contractual dates but skip historical-count
coverage validation for a fresh prefix. The asset bands labelled
relative to initial spot currently use valuation spot. Both asset
routes can return zero on or after payment before checking the full
request. The [callable rate payoff](../../../../finstack-quant/valuations/exotics/src/exotics/callable_range_accrual/pricer.rs)
is a separate production implementation.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| History `1/2`, one shared spot `100`, row scales `0.875,1.125,1.25`, band `[87.5,112.5]` | Both boundary rows qualify and the third misses. The completed fraction is `3/5`. With notional `1000`, period coupon `0.08` and payment factor `0.95`, PV is `45.60`. |
| Required live row lacks spot, or a new trade supplies nonzero historical counts | Fail validation or completion. A later spot supplies no replacement observation. The full denominator stays fixed. |
| Inception reference `100` and relative bounds `0.90,1.10` | The band remains `[90,110]` when current spot moves to `130`. A historical qualifying observation at `100` stays in the certified tally. |
| `S=m=nextafter(1,+∞)` and upper bound equals their rounded product | The exact product exceeds the bound by `2^-104`. It misses despite rounded equality. With `S=2^-1074,m=0.5` and band `[-1,0]`, the positive product also misses. |
| Notional `4e28`, period coupon `2` and fraction `1/2` | The final amount `4e28` fits Money even though the standalone full coupon exceeds its range. |
| January 30 and 31, 2025 from January 1 on a 30E/360 model clock, zero vol and yield, ACT/365F flat 5% financing, spot `100` | Both model times are `29/360`. Their dated forward levels are about `100.3980503985` and `100.4118044982`. A band of the first forward ±`0.0001` gives one of two hits. Notional `1e6`, annual coupon `0.10`, accrual `29/360` and January 31 payment give PV about `4011.259231828`. |
| GBM spot `100`, net carry `0.03`, vol `0.20`, band `[90,110]`, history `1/2`, future `(time,multiplier)=(0.5,1),(0.5,1.10),(1,0.95)`, notional `1000`, period coupon `0.08`, `D=0.96` | Marginal CDFs and independent density integration give PV about `35.5150990077`. The tied rows' joint hit probability is about `0.2683106640`, versus `0.2151016881` from multiplying their marginals. |

The examples check contractual counting, scale and the selected
ideal GBM law. Runtime replay, host behavior and dated prices
remain pending.
[Issuer terms](../../../REFERENCES.md#range-accrual-observation-counts-and-coupon-terms).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
