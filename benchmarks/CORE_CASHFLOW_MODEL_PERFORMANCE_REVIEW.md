# Core cashflow and model benchmark performance review

November 7, 2026. Review and planning only. Production code and existing benchmarks remain unchanged.

The current suites need a validity pass before they can establish a performance baseline. Across 21 declared native targets, 284 of 285 cases completed smoke execution. The cashflow calendar-year ladder failed on obsolete kind strings. Other completed cases can hide errors or observe only an uninformative part of the result. No new latency, allocation or memory improvement has been measured.

The six primary investigations are mixed-market context copying, floating fixing-history copies, repeated accrued-interest queries, Brownian bridge preparation, LMM drift contraction and Heston probability traversal. Their benefits are hypotheses to test, not promised gains.

## Scope and retained evidence

Source identity is the inspected working tree at HEAD 5140b499ccec38035a86cf78b21692c449e9d472, including the staged, accepted valuation optimizations. HEAD alone does not identify these bytes. The audit checked 803 source/configuration/fixture files before and after execution, with no changes. Existing staged valuation work, unrelated financial-documentation edits and all existing plans were preserved.

| Crate | Declared targets | Cases | Smoke completed | Failed |
| --- | ---: | ---: | ---: | ---: |
| Core | 14 | 176 | 176 | 0 |
| Cashflows | 2 | 59 | 58 | 1 |
| Models | 5 | 50 | 50 | 0 |
| Total | 21 | 285 | 284 | 1 |

Each original target ran serially through the existing scoped mise task in smoke mode, with Rayon 8 and a fresh CRITERION_HOME. The runner retained exit codes, selected identities, successful-row counts, merged logs and input hashes. The ladder panic stopped its target; a separate filtered run executed all eight later registered cases. The failed original target remains failed. Smoke durations are execution metadata, not timing observations.

- [Complete case inventory](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/case-inventory.json).
- [Evidence pins](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/review-evidence.json), [original ledger](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/smoke-ledger.json), [supplemental result](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/supplemental-hot-paths-result.json).
- Detailed source reviews: [core](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/core-source-review.md), [cashflows](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/cashflow-source-review.md), [models](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/model-source-review.md), [protocol and existing host rows](/Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/review-2026-11-07/protocol-review.md).

No matched standalone baseline for these three suites was found in the inspected existing Criterion artifacts. The completed valuation campaign supplies consumer history and ownership; it does not substitute for a new core/cashflows/models baseline.

## Findings before performance work

| Priority | Confirmed issue | Required action |
| --- | --- | --- |
| High | The ladder benchmark maps cashflows to principal/coupon strings that CFKind rejects, then unwraps the error. Reproduced exit 101 at cashflow_hot_paths.rs:570. | Fix fixture labels to the existing canonical kinds; do not restore obsolete aliases. Check native principal, coupon and supplied-PV totals before timing. |
| High | Cashflow aggregate_by_period returns an unchecked Result, WAL discards errors with ok(), and both repeated-accrual variants turn every failed query into zero. Core forward batches retain Results without unwrapping. | Make valid-case errors fatal in every measured iteration; retain explicit negative tests separately. |
| High | All four Bus252 tenor cases use identical start/end dates, selecting the empty-interval return. ISMA frequency labels use the same reference coupon period. Stub cases use regular quarterly endpoints and therefore generate no actual stub. | Build fixtures that exercise each named axis and assert the resulting dates/reference periods/year fractions. Old fast-path numbers are invalid baselines for the intended workloads. |
| High | Brownian bridge observes only its terminal point; LMM observes x[0], frozen from time zero; Volterra observes only its first output, which contains no far-field convolution. Rough Heston observes variance only; migration observes dimensions rather than content. | Make the full meaningful result opaque and check untimed path/state/matrix content. This is weak observability, not proof that a compiler removed work. |
| Medium | Expression evaluation observes only output length; context bump checks only existence; schedule iteration clones an owning iterator and counts it. Sobol/correlation observe one coordinate. | Consume actual outputs. Keep validation outside timing and preserve genuine allocating/drop boundaries. |
| Medium | LM's supposed dense problem has coefficients sin(i*c)*cos(j*c), hence rank one. Ok(LmSolution) can still mean StepTooSmall or MaxIterations. | Add a representative full-rank nonlinear fit and an explicitly named degenerate control; check termination, residuals and statistics. |
| Medium | FX's pinned quote is shadowed by a global quote; cross-currency warmup populates the triangulation cache. Calendar classification is a warm year-cache query. | Split and name actual cold, warm, pinned and observed workloads; assert provenance and expected rates. Do not claim an uncached path from a warm lookup. |
| Medium | LMM varies both forward count and step count, so the total pairwise workload is roughly cubic; its element count suggests a simple tenor sweep. LSMC uses antithetic pairs, so 5,000 independent estimators mean 10,000 physical paths. | Separate dimension and horizon axes; record actual work dimensions and estimator/physical-path counts. |
| Medium | Cashflow labels 5y_40cf and 10y_80cf do not describe the source-derived 22/42 output-row expectations. Aggregation reports 120 inputs although only 20 fall into its reporting windows. NPV includes extraction and validation; merge includes deep input cloning. | Freeze actual counts and honest boundaries during validity work. Do not compare changed boundaries as an optimization. |
| Medium | The three README files name nonexistent baseline/compare tasks. Models claims one target despite five; cashflow group coverage is incomplete. Model host docs advertise WASM MC pricing rows absent from the harness. | Update source-derived documentation and use supported commands. Host sampling, validation, handle cleanup and persistence need their own scoped repair. |

Evidence locations:

- Cashflow failure and hidden errors: [hot paths](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/benches/cashflow_hot_paths.rs:340), [ladder fixture](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/benches/cashflow_hot_paths.rs:538), [repeated accrual](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/benches/cashflow_scaling.rs:220).
- Misleading date axes: [day counts](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/benches/daycount_operations.rs:46), [stubs](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/benches/schedule_generation.rs:52).
- Model output and work axes: [LMM/Volterra scaling](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/benches/mc_scaling.rs:242), [rough-Heston observation](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/benches/mc_hot_paths.rs:218).
- [LM fixture](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/benches/solver_operations.rs:23), [FX fixture](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/benches/fx_matrix.rs:20).

## Native target coverage

Case counts describe the current registrations, before validity repairs or new axes. Every target has a full source/boundary review in the linked appendices.

| Crate | Target | Cases | Main measured boundary |
| --- | --- | ---: | --- |
| Core | calendar_lookup | 10 | Five calendars, repeated 2025 holiday/business-day queries |
| Core | daycount_operations | 24 | Year fractions, context-dependent conventions and an allocated batch |
| Core | interpolation | 17 | Scalar/batch interpolation, style comparison and extrapolation |
| Core | curve_operations | 17 | DF/zero/forward/survival queries, batches and construction |
| Core | solver_operations | 8 | Brent, IRR/XIRR and numerical-Jacobian LM |
| Core | rate_conversions | 31 | Scalar, round-trip and allocated-batch compounding conversion |
| Core | cashflow_operations | 2 | Complete Money NPV on flat/shaped curves |
| Core | schedule_generation | 34 | Build, adjustment, IMM, tenor/EOM/stub and iterator operations |
| Core | expr_eval | 3 | Compile-excluded expression evaluation over prepared columns |
| Core | context_bump | 3 | Fresh copy-on-write curve bump in 8/64/256-curve contexts |
| Core | brownian_bridge | 3 | Repeated path construction on a prepared topology and uniform grid |
| Core | sobol | 12 | Stateful 1,024-point uniform/normal generation |
| Core | correlation_apply | 8 | Triangular/dense factor application at four dimensions |
| Core | fx_matrix | 4 | Direct, reciprocal, warm cross and purported pinned lookup |
| Cashflows | cashflow_hot_paths | 21 | Builds, aggregation, extraction+NPV, merge, balances, WAL, JSON and accrual |
| Cashflows | cashflow_scaling | 38 | Length, adjustment, floating-build, accrual, reporting-PV and balance sweeps |
| Models | mc_hot_paths | 7 | European/LSMC/Heston/HW pricing, least squares and rough-Heston stepping |
| Models | mc_scaling | 31 | Paths, steps, regression size, rough paths, LMM, Volterra and multi-asset correlation |
| Models | fourier_pricing | 6 | COS strip and batches of independent scalar Heston calls |
| Models | migration_matrix | 4 | Path batches and empirical transition matrices |
| Models | liquidity | 2 | Roll and Amihud estimators over prepared series |

## Six primary performance investigations

This is an implementation order based on source cost, consumer reach and bounded scope. Profiling can close an investigation without a code change. The implementation plan requires measured materiality before retaining added performance complexity.

| Order | Investigation | Evidence-backed mechanism | Main measurement and correctness gate |
| --- | --- | --- | --- |
| 1 | Mixed-context bumps | Generic Curve bumps make the unrelated curves map mutable before discovering a surface, scalar or series target. | Mixed contexts with fixed target data and varied unrelated curve counts; actual finite-difference/scenario caller. Preserve target precedence, base immutability, ordered composition and credit rebinding. |
| 2 | Floating fixing-history ownership | Every compiled floating piece deep-clones a read-only historical series. Copy work grows with pieces times history length. | Full fresh/seasoned term and overnight builds with independently varied history/piece counts. Require identical flows, normalized fixings and missing-history behavior. |
| 3 | Repeated accrued-interest queries | A prebuilt AccrualIndex still scans every period and allocates an active Vec for every date. N queries therefore scan O(N squared) period predicates. | Build-plus-N-query workloads, real exercise dates and overlap/ex-coupon controls. Preserve every active segment, original summation order, query errors and calendar/day-count rules. |
| 4 | Brownian bridge grid preparation | Every path rescans the same grid and recomputes midpoint divisions/square roots; Sobol increment conversion also repeats sqrt(dt). | Core whole-path kernel plus setup-inclusive production Sobol/bridge pricing. Preserve the current supported dimension limits, shock order and every path increment; do not expand Sobol capability to improve a benchmark. |
| 5 | LMM drift contraction | Each predictor/corrector drift performs nested forward-pair/factor work. A reverse factor-tail sum can reduce per-step O(N squared times factors) to O(N times factors). | Fixed-horizon tenor sweep plus full model caplet pricing. Freeze scalar-reference/path bounds before reassociation; retain terminal-measure, dead-forward, denominator, displacement and knot rules. |
| 6 | Heston probability traversal | Two cached-probability loops compute the identical strike phase at every quadrature node. | Fuse only the phase traversal; keep separate probability and tail sums. Profile CF/grid construction separately so a kernel win must survive the full scalar-price boundary. |

Source owners: [context dispatch](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/market_data/context/ops_bump.rs:274), [floating market resolution](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/builder/pipeline.rs:280), [accrual queries](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/cashflows/src/accrual.rs:328), [bridge coefficients](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/core/src/math/random/brownian_bridge.rs:101), [LMM drift](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/src/monte_carlo/process/lmm.rs:296), [Heston probabilities](/Users/jeickmeier/Projects/finstack-quant/finstack-quant/models/src/closed_form/heston/strip_pricer.rs:155).

The typed VolBucketPct route already selects the surface map; the context investigation concerns the generic Curve dispatch. Existing DV01 scratch/revert, exact HW preparation, Asian paired traversal, Sobol state, triangular correlation, rough-Heston kernel tables, flat LSMC paths, regression input reuse, COS coefficient reuse and internal Heston strip caches are already implemented. They are baseline behavior.

## Conditional follow-ups

These are documented alternatives outside the six primary implementation slices. They require their own profile, workload justification and selection record before promotion.

- Avoid duplicate projected-fixing placeholders while retaining complete public metadata, conflicting observations and required error records.
- Stream reporting PV and resolve base survival once per call. Retain current validation of out-of-window flows, native currencies, compensated order and payment-date FIFO recovery approximation.
- Avoid many-program-piece compiler/emitter cross-products if a realistic many-piece workload demonstrates the cost.
- Reduce copied calendar year-cache payloads if emitted-code/profile evidence shows the compiler has not already removed the copies.
- Reuse LM normal equations only when real rejected-damping work is material, without duplicating the existing credit-calibrator ownership.
- Reduce LSMC matrix copies/basis reevaluation while retaining SVD, or migration/Volterra per-path setup, only after stage profiles justify them.
- Investigate expression allocation only against actual consumers. The observed composite caller uses one row and includes compilation; the current many-row benchmark does not establish a statements-wide gain.

## Host and ownership boundaries

Existing Python coverage has 10 core, seven correlation and five model rows; direct cashflows rows are absent. MC pedantic sampling is hardcoded independently of Criterion. Existing WASM model rows are Black-Scholes call/put only, use raw bindgen output, and do not substantiate the documented MC claim. Current core/model JS rows need scoped selection, meaningful warmup/output assertions, consistent handle cleanup and persisted raw results. This plan audits and repairs existing host rows; a comprehensive new cross-language programme is separate.

The completed valuation plan/status/results remain unchanged. Its accepted source is part of the new baseline, not a fresh performance candidate. Existing credit-calibrator BC4/SABR work and older cashflow correctness plans retain ownership. Current [financial follow-ups](/Users/jeickmeier/Projects/finstack-quant/docs/decisions/financial/FOLLOW_UPS.md:205) leave convention/adoption questions open. Performance work must preserve observed fixing, day-count, PIK, same-day event, ex-coupon, recovery and reporting contracts; it does not adopt a new policy.

The next step is the new [implementation plan](/Users/jeickmeier/Projects/finstack-quant/.plans/CORE-CASHFLOW-MODEL-PERFORMANCE-PLAN.md): repair validity, freeze a compact representative baseline and numerical contracts, then profile and execute one bounded investigation at a time.
