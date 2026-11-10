# Credit calibrator performance implementation plan

Plan only; no optimization has been implemented. Prepared against `master` at `2270da9537` on November 5, 2026. Implement the slices below in order, keeping each independently reviewable and leaving a working, validated product after each slice. Treat this file as the fixed implementation brief; track execution status separately.

Source: [performance review and retained measurements](/Users/jeickmeier/.codex/visualizations/2026/11/06/01a1af16-062a-70e0-8cd4-923795ede5aa/credit-calibrator-performance-review.md).

## Scope and numerical contract

Start with base correlation, then hazard curves, then credit-vol surfaces. The prior isolated-step measurements were 8.61 s for five base-correlation knots, 51.45 ms for five hazard pillars, and 0.578 ms for three SABR expiry slices. They establish priorities, not performance budgets; capture fresh baselines before implementation.

Preserve these throughout:

- Finite-name pricing, probability clipping, correlation smoothing, recovery writedown, settlement/day-count rules, and clean/dirty conventions.
- Existing solver tolerances, iteration settings, quadrature budgets, tail partitions, failure rules, and calibration acceptance policies. The default bootstrap tolerance remains `1e-12`; base-correlation final validation remains `1e-3` of tranche notional; the Gaussian capped-expectation integration budget remains `1e-10` of pool notional.
- Sign-change evidence, approximate-knot reporting, final repricing, monotonicity/arbitrage checks, and deterministic results/error selection.
- SABR's nine starts, order, strict score tie-breaking, per-quote acceptance, and current LM stopping tolerances.
- Independent financial references and their tolerances. Generated UI fixtures establish integration consistency, not independent numerical correctness.

Pure preparation/caching refactors should reproduce the existing arithmetic bit-for-bit on the same target/build; check with `to_bits()`. Analytic identities, solver changes, and shared quadrature can legitimately change rounding. Those slices must satisfy the existing independent numerical checks and the operation-specific bounds fixed in S0. Native/WASM parity uses the repository's justified cross-target tolerances, not an invented universal bit-identity requirement.

Do not retain obsolete production implementations, compatibility layers, or configurable old/new paths. Keep pricing logic in valuations and reusable mathematical kernels in core/models. Prepared pricing objects crossing from valuations into calibration need a narrow, documented public Rust interface with private state; they do not need new Python/WASM wrappers merely because Rust visibility is public.

## Acceptance procedure for every slice

1. Record the starting revision and cleanly identify unrelated work. Run the affected scoped checks and the slice's benchmark before editing.
2. Implement one change and its meaningful correctness coverage. Use existing dependencies and APIs before adding helpers or packages.
3. Run scoped formatting/lint/tests for the owning crates and directly affected consumers. Run each added benchmark so its target is actually compiled.
4. Compare against both the previous accepted slice and S0 using identical inputs, release settings, machine, and cache conditions. Run timing comparisons sequentially, without another benchmark/build competing for resources. Keep raw samples, confidence intervals, input hashes, revision/toolchain, residuals, and work counts in the implementation's benchmark record.
5. Include preparation and cache construction in end-to-end timing. Measure prepared reuse separately, clearly labeled. A cache-hit microbenchmark alone cannot establish a calibration improvement.
6. Accept only when numerical/error contracts hold and a repeatable improvement appears in the affected workload without a material regression elsewhere. Repeat inconclusive comparisons; do not label noise or fewer evaluations alone a speedup. Do not keep performance-specific complexity without demonstrated benefit. Instrumentation/preparation foundation slices may be neutral, but must enable the next measured slice without introducing a regression.

Use the existing Criterion task with a longer measurement window when necessary, for example:

```sh
FQ_BENCH_SAMPLE_SIZE=10 FQ_BENCH_WARM_UP_TIME=3 FQ_BENCH_MEASUREMENT_TIME=10 FQ_BENCH_NRESAMPLES=10000 mise run rust-bench-crate -- finstack-quant-calibration credit_calibration
```

`credit_calibration` is the new focused target created in S0. Keep before/after Criterion output separately because subsequent runs overwrite its `new` directory. Report runtime and work counts independently; do not add overlapping percentage gains.

## S0 — Establish permanent base-correlation benchmarks and accuracy gates

**Change:** Move the existing hazard/base-correlation benchmark entries out of [the general calibration benchmark](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/benches/calibration.rs) into a focused `credit_calibration` Criterion target, registered in the calibration manifest. Preserve the existing full-plan measurements and add isolated calibration using actual prerequisite-calibrated markets. Adapt the review's temporary harness into benchmark support rather than maintaining a separate ad hoc runner. Assert `report.success` and valid outputs; do not benchmark unchecked `Result`s or failed fits.

**Workloads:** Begin with the existing 125-name, five-knot CDX example; add curated ten-year, thin-tranche, high-detachment/recovery-sensitive, distressed, boundary-correlation, seasoned and noncontiguous-attachment cases. Keep a representative heterogeneous and Student-t pricing control. Avoid an unnecessarily large Cartesian product. Expand hazard/vol workloads before their respective phases.

**Numerical gates:** Retain [the independent finite-pool reference](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_tranche/pricing/tests.rs:2394), which currently requires error below `2e-10`, and its provenance. Before changing algorithms, record raw capped loss/recovery expectations, period-level losses/writedowns, fixed-correlation PV, final quote residuals, calibrated knots, selected off-grid prices, and relevant correlation sensitivities. Derive case-specific tranche-PV bounds from existing pool error budgets, tranche width and discount/cashflow coefficients; do not choose a tolerance after seeing a candidate's error. Use sensitivity-informed knot comparisons: near-flat objectives require repricing and off-grid checks, not a misleading blanket tight knot threshold. The loose `1e-3` final fit flag alone is insufficient.

**Benchmark evidence:** Record objective calls separately from commit validation/final repricing; the previous five-knot baseline had 177 solver calls plus ten additional repricings. Collect integrand/PMF counts in untimed diagnostic runs or test-local instrumentation without adding production counters to every node. Correct the unchecked-success cases in [tranche pricing benchmarks](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/benches/cds_tranche_pricing.rs).

**Done when:** All chosen success cases genuinely succeed, negative controls retain their failures, the independent references remain intact, and isolated/full-plan baselines are reproducible. No pricing change in this slice.

## BC1 — Remove exact zero and nonbinding-cap integrals

**Files:** [expected_loss.rs](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_tranche/pricing/expected_loss.rs), tranche pricing tests.

**Change:** In the homogeneous one-factor Gaussian kernel, resolve zero caps and nonbinding caps analytically using the same effective clipped default probability. A nonbinding cap has expectation `exposure × effective_PD`. Resolve loss and recovery independently so only genuinely capped components require quadrature. Perform required pool/input validation before the shortcuts. Preserve all other copula/model rules.

**Checks:** Existing independent finite-pool pins, zero recovery, boundary PD, super-senior and seasoned recovery tests; add exact cap identities at and just around the exposure bound, recovery endpoints, small/large pools and invalid inputs. Check thin-tranche PV and final calibration against S0's accuracy gates.

**Benchmark:** Isolated and full-plan five-knot base correlation; ordinary equity/mezzanine pricing; high-detachment pricing where recovery still binds. Record eliminated integral/PMF calls, without assuming a proportional wall-time gain.

## BC2 — Prepare tranche pricing once, using one shared pricing kernel

**Files:** [pricing engine](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs), expected-loss/pricer modules, [base-correlation target](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/src/targets/base_correlation.rs).

**Change:** Add an opaque valuations-owned prepared tranche object containing validated immutable market/contract inputs, coupon periods, dates, default probabilities/thresholds, default timing, discount weights, upfront and realized-state inputs. Its evaluation accepts trial correlations/the trial correlation curve, not an unrelated mutable market. Route ordinary pricing and calibration through the same calculation; remove superseded duplicated setup. Cache only invariants in this slice—continue evaluating both raw attachment/detachment sides.

**Checks:** Bitwise equivalence for unchanged evaluation order; PV decomposition, projected schedules, settlement/rolled coupons, nearly wiped-out structures, curve rebasing, missing index and invalid date errors. Document the cross-crate Rust API with complete argument/error semantics and keep its state non-serializable.

**Benchmark:** Preparation alone, repeated evaluations, and whole calibration including preparation. Record memory retained per quote and ensure it is bounded by the schedule/pool size.

## BC3 — Reuse fixed attachment expectations within a knot solve

**Files:** The BC2 prepared object and base-correlation target.

**Change:** Retain one attachment-side vector of raw capped loss/recovery expectations inside each prepared quote. Reuse it only when the actual effective cap, resolved/smoothed attachment correlation and immutable inputs match. Recompute when the trial changes the attachment correlation through interpolation, including diagnostic bumps. A bounded last-value cache is sufficient; do not accumulate all trial correlations or add a global cache.

Cache before tranchelet subtraction, interim floors, time-monotonicity repair, joint caps and width normalization. Never reuse a previous tranche's final `ElWdPoint` vector as an attachment contribution. Cross-quote detachment-to-attachment sharing is deferred unless a later measurement justifies the additional matching logic.

**Checks:** Prepared cached/uncached bitwise parity; contiguous and noncontiguous ladders; attachment interpolated between a previous and current knot; changed-correlation diagnostic bumps; repeated/reordered evaluations; all S0 repricing/arbitrage gates.

**Benchmark:** Per-knot raw attachment evaluation counts and isolated/full-plan calibration. Standard fixed attachments should be computed once per distinct attachment state, not once per trial. Measure noncontiguous cases as a regression control.

## BC4 — Replace bisection-first convergence with safeguarded interpolation

**Files:** [bootstrap helper](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/src/solver/helpers.rs), bootstrap tests, [core Brent solver](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/solver.rs) only as needed.

**Change:** Reuse the existing Brent implementation after the current authoritative scan establishes a valid bracket. Its current entry point reevaluates endpoints and may stop in x-space: introduce the smallest shared evaluated-bracket/convergence interface needed to reuse known endpoint values and retain bootstrap's residual-space acceptance. Preserve invalid-interior handling, first-error/best-candidate diagnostics, sign-change classification and no-bracket/approximate behavior. An x-space termination must never silently count as f-space convergence. Remove the superseded bisection-first implementation once equivalent safeguards are in the shared path.

**Checks:** Core solver tests; bootstrap signed-zero, invalid-boundary, multiple-root selection, tangent/approximate, exact-initial-root, penalized-domain and f-space convergence regressions. Because this is shared code, run the whole calibration crate's tests, including discount, forward, hazard and Student-t consumers. Keep all configured tolerances unchanged.

**Benchmark:** Base/hazard evaluation counts and end-to-end time, plus representative discount/forward and global-calibration controls. Count every actual callback, including endpoint and post-solve checks.

## BC5 — Precompute binomial terms invariant to conditional PD

**Files:** [core distributions](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/distributions.rs), tranche [conditional expectation kernel](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_tranche/pricing/sensitivities.rs), prepared pool state.

**Change:** Precompute `ln(N-k) - ln(k+1)` once per fixed pool size and reuse it in the existing log recurrence. Keep normalization, clipping, output-buffer reuse and arithmetic ordering intact. Share one core implementation rather than duplicating a binomial algorithm in valuations. Do not introduce a mode-centered recurrence in this slice.

**Checks:** PMF mass, moments, degenerate probabilities, NaN/error behavior, tails, large N and existing finite-binomial-versus-LHP regression. Require same-build bitwise parity where only identical constant expressions have been hoisted, and retain independent tranche-reference checks.

**Benchmark:** PMF evaluations across representative N/PD values, expected-loss pricing, isolated/full-plan calibration, including one-off preparation cost. Keep only if the remaining workload benefits after BC1–BC4.

## BC6 — Share Gaussian paired quadrature evaluations

**Files:** [tranche integration](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_tranche/pricing/integration.rs), [core integration](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/integration.rs) and tests.

**Change:** Extend the existing adaptive Simpson implementation with a minimal two-component path sharing nodes/conditional PMFs. Both components must independently meet their original summed error budget. Preserve the conditioning-axis partition, omitted-tail allowance, deterministic accumulation and maximum-depth failure. Use it for one-factor Gaussian cases where both components need integration; retain the separate established Student-t, RFL and multifactor rules. Avoid a general-purpose vector integration framework.

**Checks:** Independently known integrals; components with very different scales/localized transitions; one component failing tolerance; non-finite callbacks; high-correlation boundaries; independent tranche loss and recovery references. A combined norm must not conceal an inaccurate small component.

**Benchmark:** High-detachment and recovery-sensitive pricing, both-component integrand counts, and the standard calibration control. Benefits overlap BC1, so judge incremental gains after all earlier slices.

## H1 — Reuse prepared CDS valuation in hazard calibration

**Files:** [CDS pricing engine](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds/pricing/engine.rs), [hazard target](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/src/targets/hazard.rs), credit repricing tests.

**Change:** Generalize the existing `CdsHazardRepriceCache` into the single shared prepared repricer used by both risk and calibration. Expose only the narrow Rust cross-crate interface required. Retain schedule, coupon DFs, upfront and clean accrued; leave integration-window preparation to H2. Ordinary valuation and the prepared path must share calculation logic.

**Checks:** Existing full-versus-cached tests, clean/dirty, upfront/par-spread quotes, AoD, settlement delays/calendars, mixed curve bases/day counts, validation errors and bootstrap/global final fits.

**Benchmark:** Extend `credit_calibration` with isolated standard/distressed/upfront and long-tenor hazard cases. Compare setup plus solve, repeated repricing, full plans and existing global-hazard controls.

## H2 — Prepare ISDA integration windows for each actual knot layout

**Files:** CDS engine, [CDS pricing helpers](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds/pricing/helpers.rs), hazard target.

**Change:** Retain integration boundaries, settlement dates, DFs and discount-rate coefficients for the fixed discount snapshot and hazard knot-time layout. Recompute survival/hazard-dependent terms for each trial. Bootstrap preparation uses each quote's actual growing prefix plus current pillar; global calibration uses its configured full layout. Reprepare when topology changes; never freeze a grid built for a shorter prefix.

**Checks:** Bitwise full/prepared equivalence with identical operation order; exact boundary insertion/sorting/dedup, 25-per-year substeps, negative discount rates, settlement mapping and error behavior. Exercise changed knot topology, curve bases/day counts, discount inputs, calendars and as-of dates.

**Benchmark:** H1 workload set, with grid/date/DF setup counts in untimed diagnostics. Compare incremental gains over H1 and total gains over S0.

## H3 — Share validated source replay across distinct hazard bumps

**Files:** [hazard recalibration/cache](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/src/recalibration/hazard.rs), provider tests.

**Change:** Add a batch-local validated-source cache beside the existing bump-result cache, reusing `KeyedOnceCache`. Identity keys include the complete source/recipe and actual identity-market dependencies and applicable replay conventions/settings; the distinct bump is excluded. Keep target-market dependencies in the bump-result key. Use content fingerprints rather than IDs/pointer identity. Preserve identity validation and no-cache-on-error behavior, including concurrent callers.

**Checks:** Same source/different bumps share one identity solve; every relevant dependency mutation invalidates; source and shocked-target markets remain distinct; zero bumps, tampered recipes, FX policies, failed computations and concurrent/reordered requests behave correctly. Check source fingerprints and error provenance rather than assuming the discount curve is the only possible dependency.

**Benchmark:** One bump, an ascending bucket sweep, reversed order and concurrent same-source requests. Include fingerprint/cache setup cost. For B distinct nonzero bumps, verify B+1 full solves instead of 2B, then separately measure wall time.

## V1 — Correct SABR work accounting and establish larger-surface baselines

**Files:** [SABR calibration](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/src/volatility/sabr/calibration.rs), [surface target](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/src/targets/vol.rs), `credit_calibration` and [sabr_slice benchmark](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/calibration/benches/sabr_slice.rs).

**Change:** Make the existing `residual_evaluations` field count actual residual invocations across starts, including rejected attempts where calls occurred. Preserve separate winning-start statistics and all fit behavior. Do not invent a replacement report field. Add isolated credit-beta surfaces with, for example, 10 expiries and 15–25 strikes, alongside the existing small fixture and ATM-pinned controls.

**Checks:** Exact work totals from controlled multi-start cases; rejected/error paths; unchanged fitted parameters, winning starts, residuals and determinism. Regenerate affected integration/report fixtures after verifying the count correction; do not change financial reference expectations.

**Benchmark:** Small and large surfaces; this telemetry slice is expected to be neutral. Establish trustworthy total work counts before changing algorithms.

## V2 — Prepare SABR strike constants once per expiry

**Files:** SABR calibration, [SABR expansion](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/src/volatility/sabr/expansion.rs), model tests.

**Change:** Prepare forward/strike/beta/time/shift-dependent constants and branch classifications shared by all starts. Keep alpha/nu/rho-dependent terms in each evaluation. Reuse the canonical formula implementation so public implied vol and fitting cannot drift. Apply preparation where valid to free-alpha and ATM-pinned fits without changing either solver.

**Checks:** Same-build value/fit parity; normal/lognormal, shifted, ATM/small-z, beta limits and parameter boundaries; independent SABR golden cases; off-grid smiles and winning-start selection.

**Benchmark:** Formula evaluation plus full small/large surface fits, including preparation. Record both residual counts and time, because the expected gain is cheaper evaluations, not fewer evaluations.

## V3 — Supply analytic Jacobians for the free-alpha SABR fit

**Files:** SABR calibration/expansion and existing [LM analytic-derivative interface](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/solver_multi.rs).

**Change:** Derive derivatives for the existing residual formulas and transformed log-alpha/bounded-nu/bounded-rho coordinates, including supported limiting branches. Use `solve_system_with_jacobian`. Keep all starts, weights, bounds and acceptance rules. Do not silently enable an unvalidated derivative around a branch seam. ATM-pinned fitting remains its distinct current algorithm; differentiating through its alpha solve is a later separately justified slice.

**Checks:** Independent/high-accuracy finite-difference derivative comparisons over several step sizes and one-sided checks at branch boundaries; full fit residuals, parameters, bound hits, winning start, off-grid smile behavior and deterministic reruns. Analytic Jacobians can change optimizer trajectories: unchanged start count alone does not prove numerical equivalence. Investigate material selection/shape differences rather than accepting them merely because quote tolerance passes.

**Benchmark:** The same V1/V2 small and large surfaces, total residual evaluations and end-to-end time. Retain only after numerical equivalence and practical benefit are demonstrated; the existing sub-millisecond case alone does not justify added complexity.

## Separate follow-up — CDS-option premium-to-vol preparation

This adjacent workflow is not needed for already-quoted credit-vol surface fitting. After the main phases, split the invariant `ForwardCdsContext` from sigma-dependent state in [Bloomberg quadrature](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs), and prepare it once per [implied-vol inversion](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/credit/src/cds_option/pricer.rs). Recalibrate the sigma-dependent mean at every sigma trial. Verify price/IV round trips, strike/knockout/index conventions and errors; benchmark the existing `cds_option_implied_vol/implied_vol_solver` case in [cds_option_pricing](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/valuations/benches/cds_option_pricing.rs).

Deferred from this plan: relaxed tolerances, SABR first-acceptable-start termination, blanket parallelism, a different binomial recurrence, heterogeneous convolution redesign, and cross-quote attachment-vector sharing. They require separate evidence after the simpler changes land.

## Verification and final integration

For each touched package, use `mise run rust-lint-crate -- <package>` and its scoped tests. The main packages are `finstack-quant-core`, `finstack-quant-models`, `finstack-quant-valuations-credit`, `finstack-quant-calibration`, and the `finstack-quant-valuations` umbrella. Run relevant umbrella consumers after credit-pricing changes; run all calibration tests after shared bootstrap changes.

Useful commands:

```sh
mise run rust-test-filter -- finstack-quant-valuations-credit cds_tranche
mise run rust-test-filter -- finstack-quant-calibration base_correlation
mise run rust-test-crate -- finstack-quant-calibration
mise run rust-test-filter -- finstack-quant-models sabr
mise run rust-bench-crate -- finstack-quant-calibration credit_calibration
mise run rust-bench-crate -- finstack-quant-calibration calibration
mise run rust-bench-crate -- finstack-quant-calibration global_calibration
mise run rust-bench-crate -- finstack-quant-calibration sabr_slice
mise run rust-bench-crate -- finstack-quant-valuations cds_tranche_pricing
mise run rust-bench-crate -- finstack-quant-valuations cds_option_pricing
```

Run only the relevant benchmark targets for a given slice. Do not run `cargo test` or `cargo clippy` directly. Keep full-workspace checks until final integration.

Regenerate with `mise run gen-write` in the same implementation slice whenever bindings, wire/generator code, or numerical outputs represented by native UI fixtures change. Any actual host-surface change must include Python/WASM documentation, parity ledger and generated declarations together. Prepared engine machinery itself should not create new host-facing APIs.

After all accepted slices: run `mise run all-fmt`, any required generation, then `mise run all` once. That gate includes full Rust lint/test and Python/WASM/UI checks; do not repeat full `rust-lint`/`rust-test` separately. Include required public-API documentation checks for new Rust interfaces. Verify representative binding-level calibration outputs, successful fits and deterministic ordering under the existing host tolerances.

Deliver a final original-versus-final and per-slice benchmark table with raw evidence, setup boundaries, work counts, memory impact and numerical deltas. Identify deferred/rejected changes explicitly. Completion requires both preserved numerical contracts and demonstrated end-to-end improvements in the workloads each optimization targets.
