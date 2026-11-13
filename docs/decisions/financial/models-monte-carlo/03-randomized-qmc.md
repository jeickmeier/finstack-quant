# MODELS-MONTE-CARLO-03: Whole-path Sobol points, Brownian bridge and replicate uncertainty

- Decision status: **Accepted — complete whole-path Sobol pricing and replicate diagnostics package, 2026-11-11**.
- Implementation status: Existing whole-point driver observed. The accepted equal blocks, checked bridge delivery, replicate diagnostics and caller reconciliation are pending.
- Validation status: Source, selected tests and primary references inspected. Independent budget, seed, bridge and replicate examples checked. Runtime, host and pricing qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for whole-path Sobol pricing and replicate diagnostics

1. **Select a dedicated single-factor GBM pricing mode.** Require an
   explicit sampling mode with an effective `u64` root seed, replicate
   count, points per replicate and either chronological normals or
   Brownian bridge construction. Admit the single-asset, single-factor
   `GbmProcess` with the exact transition on a positive-horizon grid.
   Retire the paired Sobol/bridge booleans and the implicit bridge,
   replicate count and pricing defaults on this route. Reject
   unsupported model, scheme and mode combinations before drawing,
   including parallel or antithetic Sobol pricing and a Sobol construction
   choice on a pseudorandom route. Keep the dedicated Sobol run serial
   and unpaired. [Exact GBM owner](04-lognormal-diffusions.md),
   [accepted primary engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

2. **Use equal, complete power-of-two blocks.** Require an integer
   replicate count `2 <= R <= 16` and `M = 2^k` points per replicate
   for integer `k >= 0`. A single point per block is admissible,
   with no variance-reduction claim. Check `M <= 2^32`,
   `R*M <= 10,000,000`, count/byte arithmetic and allocations before
   starting. The existing maximum of 16 bounds per-replicate setup
   work. It implies no statistical target. `R` need not be a power
   of two. Start a fresh accepted core generator for each replicate
   at index zero, consuming every
   point from `0` through `M-1` in natural binary order. Process
   replicate indices then point indices in ascending order. No
   unequal remainder, skip, padding, thinning, rounding of a
   supplied budget or data-dependent stopping. [Accepted core Sobol](../core-numerics/sobol-sequences.md),
   [Sobol block design](../../../REFERENCES.md#owen-2021-sobol-first-point).

3. **Pin the deterministic seed and auxiliary uniform assignment.** With
   root `s` and zero-based replicate `r`, select the accepted
   prefix-hash permutation as `Some((s XOR S) + r*W mod 2^64)`,
   where `S=0x534f424f4c534352` and `W=0x9e3779b97f4a7c15`.
   Preserve `Some(0)` literally and remove the observed remapping of
   zero to one. The odd increment gives distinct labels in the admitted
   run. Permutations or projected coordinates can still coincide.
   Give point `j` the checked flat trajectory ID `r*M+j` and use
   it as the Philox stream ID under key `s XOR 0x535044485f415558`
   for any required payoff uniforms. Fix uniform demand and node
   order at admission, including any time-zero demand. Use no Sobol
   coordinates for payoff uniforms and add no hidden random draws.
   Record this layout for replay. A changed budget can change auxiliary
   addresses. Salted keys and distinct labels alone give no
   independence or cross-run namespace guarantee.
   [Accepted Philox profile](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

4. **Assign one complete normal point to the actual path grid.** The
   checked Sobol dimension equals the number of admitted positive
   intervals and must be in `1..=40` after exact event placement.
   Use core's midpoint and checked inverse normal on every coordinate.
   Chronological construction assigns coordinate `i` to interval
   `i`. Keep the private precomputed-path adapter with checked
   cursor, shape and finite values. Over-consumption fails at the
   request and under-consumption fails at path completion. No scalar
   Sobol fallback, automatic coarsening, skipped event or hidden
   coordinate. [Canonical time grid](../core-numerics/time-grids.md),
   [accepted Sobol profile](../core-numerics/sobol-sequences.md).

5. **Use the accepted bridge ordering and compute standardized
   increments directly.** For bridge construction, use the first
   coordinate as the standardized terminal increment and the
   remaining coordinates at the accepted terminal-first, left-before-right
   depth-first index midpoints. At a split `l < m < r`, set
   `a=t[m]-t[l]`, `b=t[r]-t[m]`, `L=t[r]-t[l]`. For parent
   standardized increment `p` and midpoint innovation `z`, pass
   `sqrt(a/L)*p + sqrt(b/L)*z` left and
   `sqrt(b/L)*p - sqrt(a/L)*z` right. The leaf values are the
   chronological standardized increments. This is the same ideal
   conditional Gaussian law as the accepted level construction.
   Evaluate the weights as `sqrt(a)/sqrt(L)` and
   `sqrt(b)/sqrt(L)`, with checked signed sums using actual knots,
   preserving representable small contributions and checking the
   requested increments. Subtraction of separately rounded levels
   cannot be the only source of increments. No floor or silent
   substitution when a required result is unresolved. Keep this
   increment computation private. Bridge construction is a change
   of Gaussian coordinates. Contractual barrier and lookback
   monitoring use their own methods.
   [Accepted bridge law](../core-numerics/pseudorandom-and-bridge.md),
   [Brownian paths and QMC](../../../REFERENCES.md#sobol-blocks-brownian-paths-and-replicate-diagnostics),
   [barriers](18-barrier-monitoring.md), [lookbacks](19-lookback-monitoring.md).

6. **Reuse the checked path, event and discount contract.** Bind the
   exact GBM transition and all contractual events to the stored
   knots. Scheduled carry uses both stored endpoints and diffusion
   uses the actual adjacent interval. Preserve the owner's measure,
   fixings and equivalent settlement amount. Apply one positive
   deterministic Money discount per path and validate leg currency
   and projection. Reuse the checked path loop without its IID
   estimator. Report a failed path with replicate, point and actual
   stage/node context. Discard failed runs and keep optional capture
   from changing the simulation or reduction. Capture population and
   IDs refer to the `R*M` trajectories. Capture policy and
   diagnostic resource limits stay with record 28.
   [Accepted pricing path contract](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation),
   [capture owner](28-capture-and-simulation-contracts.md).

7. **Use replicate means consistently and report descriptive spread.**
   Let `Y[r]` be the checked mean of the `M` discounted values in
   replicate `r`. Report the checked equal mean of the `R`
   replicate means as price, with `R` replicate units, `M` points
   per replicate and `R*M` simulated trajectories. Report the
   sample standard deviation of the replicate means with divisor
   `R-1` under a descriptive replicate-spread field. A constant
   observed set of means gives zero spread. Keep sampling standard
   error, relative standard error and `ci_95` unavailable on this
   route. Remove the current replicate Student-t calculation and
   the overloaded point-level dispersion field from this result.
   The fixed root maps deterministically to all labels, and the
   prefix-hash profile establishes neither the independent
   randomizations required for replicate inference nor unbiased
   integration under the continuous model. Spread can be zero
   even for a biased estimate. No zero or NaN placeholder for
   unavailable inference. Use the accepted numerical and Money
   boundaries and fail unresolved required outputs.
   [Sample statistics](../core-numerics/sample-statistics.md),
   [replicate inference references](../../../REFERENCES.md#sobol-blocks-brownian-paths-and-replicate-diagnostics).

8. **Reconcile actual callers and keep specialized estimators separate.**
   Preserve plain Sobol pricing for compatible payoffs. Reject a
   Sobol request on the pooled-point Asian paired-moments route,
   control-variate pricing or unsupported Greek inference before
   drawing. The observed pooled moments do not supply replicate
   uncertainty. Reconcile the Asian owner and the existing LRM
   rejection without silently substituting plain pricing or
   pseudorandom sampling when a specialized operation was selected.
   Expose the explicit pricing mode, actual serial execution, seed,
   grid, construction, block shape and result availability through
   existing Rust, Python and WASM pricing surfaces and valuation
   diagnostics. Reject a conflicting total-count override.
   Preserve exact seed transport and checked wire
   restoration. Keep the point generator, adapter and generic capture
   route Rust-only. Dedicated risk, control, payoff and monitoring
   records own their economics and qualification.
   [Asian payoff](17-vanilla-and-asian.md), [finite-difference Greeks](25-finite-difference-greeks.md),
   [likelihood-ratio Greeks](26-likelihood-ratio-greeks.md), [control variates](27-control-variate.md).

## Observed choice and domain

`PathDependentPricer` offers explicitly selected randomized Sobol pricing for GBM payoffs. One Sobol point supplies the entire Gaussian path. `with_sobol(true)` also enables Brownian-bridge construction; a subsequent override can disable it. Pseudorandom pricing remains the registry default. The bridge flag alone has no effect in the Philox branch.

## Observed algorithm and controls

Dimension is steps times factors without a bridge, or steps with a single-factor bridge. Reject zero/excessive core Sobol dimension, parallel Sobol and antithetic Sobol. Split the requested path budget among `min(16,num_paths)` replicates. The first remainder replicates receive one extra path. Scramble seeds combine root seed, a domain salt and a Weyl multiple, then replace zero by one. Draw an entire normal point and optionally construct Brownian levels on actual grid times to recover standardized increments `(W[i+1]-W[i])/sqrt(dt[i])`. Payoff uniforms use a separate salted path-indexed Philox stream. Use exact GBM and ordinary payoff/capture machinery.

The reported mean weights all individual paths equally. Standard error and Student-t half-width come from unweighted replicate means; displayed path count and standard deviation refer to individual paths. This distinction is financially material: the CI is across scrambles, not an i.i.d. interval over Sobol points.

## Observed failure behavior

No power-of-two size is imposed. For unequal replicate sizes, reported
mean and uncertainty use different weighting. The private path adapter
uses unchecked slicing. The bridge uses differences of rounded levels.
The paired Asian moment route pools individual Sobol points and exposes
those moments to the control-variate caller. Capture retains the generic
expected-count hash sample.

## Independent examples and qualification

| Example | Expected behavior under the accepted contract |
| --- | --- |
| `R=3,M=4` | Three complete four-point prefixes and 12 trajectories. Reject `R=1`, `R=17`, `M=3` or `3*2^22` trajectories. |
| Discounted pairs `(0,2),(2,4),(4,6)` in three equal blocks | Replicate means `1,3,5`, price `3`, sample replicate spread `2`, three replicate units and six trajectories. Sampling SE and CI are unavailable. |
| Observed 17-path split, first two points worth 16 each, other 15 points zero | Observed price center `32/17` and unweighted replicate center `1` differ. The accepted mode rejects the unequal block allocation. |
| Root `s=S`, replicate zero | Use `Some(0)`. The old mapping aliases this label to the same label as `s=S XOR 1` for replicate zero. |
| Core scramble seeds `3595` and `8148` in dimension one | Both give the same first-coordinate hash seed `0x1af797e1`, hence the same entire coordinate permutation. This example concerns the hash family across seeds. |
| Synthetic replicate value equal to the low bit of its label, with `R=2` | The odd Weyl increment makes the bits opposite for every root. The combined mean is always `1/2`, while the sample replicate spread is `1/sqrt(2)`. Distinct labels and nonzero spread do not establish a sampling SE. |
| Knots `[0,1,4]`, coordinates `(2,0)` | Chronological mode delivers `(2,0)`. Bridge mode corresponds to levels `[0,1,4]` and delivers `(1,sqrt(3))`. |
| Knots `[0,1,1+2^-52]`, ideal normals at midpoint indices `0xc0000000,0x80000011` | Both terminal-side levels round to the same binary64 value. The standardized final increment is approximately `-1.6266731e-10` and is available from the normalized split. The example uses two individually admitted coordinates. |
| Integrand `1{u < 2^-34}` | Continuous uniform integral is positive. Every admitted midpoint is at least `2^-33`, so every replicate returns zero. |

Independently qualify exact point and seed replay, the bridge's signed
orthogonal splits and irregular-grid covariance, exact fixing and carry
placement, discount/currency errors, draw accounting, capture transparency,
and existing host shapes and availability. Compare bias, convergence and
replicate dispersion across multiple roots with independent prices on
smooth and discontinuous payoffs, including those using auxiliary
uniforms. These checks do not imply universal price accuracy, improvement
over chronological paths or reliable confidence coverage.

## Evidence

The source includes `price_with_sobol`, `fill_sobol_increments` and
`SobolPathStream`. Existing tests include a seeded error-bar diagnostic,
an irregular-grid bridge, dimension rejection and multiple-path capture.
These are evidence about the observed contracts.
[Path-dependent pricing](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs),
[exact transition](../../../../finstack-quant/models/src/monte_carlo/discretization/exact.rs),
[Python pricers](../../../../finstack-quant-py/src/bindings/models/monte_carlo/pricers.rs),
[WASM pricing](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs),
[Asian control caller](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs),
[FX diagnostics](../../../../finstack-quant/valuations/fx/src/fx_barrier_option/pricer.rs).

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
