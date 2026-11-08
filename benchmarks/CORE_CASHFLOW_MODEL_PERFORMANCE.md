# Core, cashflow and model benchmark performance

The campaign retains three changes: I1's market-map presence guard, I2's borrowed fixing history and I3's accrual-query bounds. Comparing C0 with the **measured source (attempt 02)** qualifies **23 latency cases and three peak-RSS cases**, with exact recorded financial outputs. Retaining I1 explicitly accepts an adverse typed-volatility control: **16.8985% slower** under complete confirmation. This is a workload-specific tradeoff, not a clean no-regression result or evidence of a superior unknown workload mix. [Final assessment][final], [tradeoff decision][tradeoff]

I5's reverse factor-tail optimization was rejected after two reproduced accepted-input finite-to-NaN regressions. Production LMM mathematics is restored to C3. I4 bridge preparation and I6 Heston phase sharing closed at profiling without production candidates; their prospective gains remain unmeasured. Earlier accepted valuation P1–P6 work is already in C0 and receives no new gain claim. Its artifacts/plans and credit-calibrator ownership remain unchanged. [Fixed plan][plan], [I5 rejection][i5]

**Validation complete:** `mise run all-fmt` and `mise run all` passed on quality attempt 04, including **16,362 Rust tests and 13,980 Python tests**, plus WASM, UI, generation, lint and dependency checks. Its before/after audit found no native source or protected-file changes. The [compact machine record][machine] is **475,289 bytes**, below the 500,000-byte budget; SHA-256: `bdaa88380444940f4e0393b50f3165e88b391f1f8d5071789760b51e5d0a4843`. [Quality receipt][quality04]

## Benchmark repairs and checked coverage

B1 repaired all 21 Cargo-declared core/cashflow/model targets. Valid-case errors now fail, selected operations receive checked preflight outside timing, complete meaningful outputs are consumed, and unrelated fixtures are excluded from filtered processes. Repairs address canonical ladder classifications, effective Bus252/ISMA/stub/date axes, FX isolation and misleading timing boundaries. A full-rank nonlinear LM case replaces rank-one fitting, with an explicit degenerate control; LMM dimensions and MC estimator/path counts are stated accurately. A deliberately wrong result exited 101, and source was restored exactly. B1 passed **293 cases**: core 180, cashflows 60, models 53. This stage made no production optimization or performance claim. [B1 receipt][b1]

B2 repaired Python/Node references, batching, warmup, cleanup and raw result retention. Both B2 and fresh F1 host validation cover **22 Python and 22 Node rows**. F1 retained eight processes: two validity and six observation processes, yielding 132 timing-row observations. Direct cashflows host rows and Node MC rows are absent; Node exercises raw bindgen. Different host boundaries prevent a binding-overhead or cross-language speed claim. The Python Gaussian-PD preflight guard was corrected before accepted timing to the existing 1e-11 gate; failed validity evidence and unchanged inputs/reference remain archived. [B2 receipt][b2], [F1 hosts][hosts]

B3 froze bounded fixtures, identities, numerical bounds and controls before candidates. Baseline and measured-source native smoke each check **370 cases across 27 targets**, including affected consumers. Thirteen release numerical tests across nine executables produce 17 JSON files. Fifteen C0 profiles were captured separately from clean timing. C0's missing base-correlation serialization arm was repaired before timing, without changing calculation or bounds. Stage counts overlap; they are not additive totals of unique workloads. [B3 receipt][b3], [Measured-source release receipt][release]

The measured source is C3 plus the LMM regression test and its integration include, with **3,920 native source entries** pinned. Its 17 numerical files match C0 byte-for-byte except **seven predeclared root `source_label` spans** in FI exports. Core/Vanna/factor files have no exceptions. Financial values, configuration, errors and record hashes remain exact. The restored LMM contributes no accepted reassociation or numerical exception. Passing these records does not prove universal numerical correctness. [Measured-source numerical comparison][numerics]

The **delivered source (03)** subsequently corrects six diagnostic JSON literals in two test exporters. Production and benchmark sources remain unchanged. A fresh manifest, complete 370-case smoke and all 13 release exports are complete. The strict equivalence proof passes: **all 27 benchmark binaries are byte-identical**, with matching build/loader metadata and complete resolved-feature rows; **15 numerical files are whole-byte identical**, and two differ only under the exact metadata transformations for history descriptor keys/derived hashes and bridge type labels. This is a separate delivery proof, not a rewrite of the measured source's C0 comparison or a replacement timing campaign. [Delivered source manifest][delivered-source], [equivalence proof][equivalence]

## Measurement and selection

Native short timing uses 10 samples, 1-second warmup, 2-second requested measurement and 10,000 resamples, with three independent processes per revision, matched Rayon 8/mode settings and alternating revision order. Processes run serially without competing builds/profilers; source-pinned executables are built and archived before timing. Requested windows may extend.

Recorded hardware is Mac17,6 arm64 with 18 logical CPUs and 128 GiB RAM, running macOS 27.0. The compiler is Rust 1.97.1 / LLVM 22.1.6 for `aarch64-apple-darwin`, using the archived bench profile and resolved features. The dirty tree at recorded HEAD `827ebb75b9ec42a43906cf3a98c4b67834396f6f` is identified by source-byte archives and manifests; HEAD alone is not the source identity. Full compiler, environment, feature and binary hashes are retained in the [native request][native-request] and compact record.

A material latency claim needs unchanged accuracy, at least 10% aggregate median reduction, **all three paired reductions**, and **all three descriptive marginal-endpoint lower bounds** reaching 10%. Confirmation uses each case's predeclared original settings: normally 50 samples/3-second warmup/5-second requested measurement/10,000 resamples; the selected LMM caplet uses 30 seconds. Complete confirmation controls a selected case regardless of outcome. Protocols are not pooled, repetitions substituted, majority rules used or favorable retries selected. [Selection validation][selection]

The measured-source campaign completed **450 short latency, 42 confirmation and 66 RSS processes**. The selected assessment contains exactly **seven whole-confirmation rows plus 68 short rows**, covering 75 unique latency cases. Twenty-three qualify: I1 eight, I2 three and I3 twelve. This final edge is **C0→F1 only**; historical predecessor comparisons remain separate. Every paired point and endpoint was reviewed. Clock audits are clear, and dedicated/main post audits pass for all 18 unused numerical-test executables. Actual timing binaries remain guarded per process. Clock clearance establishes recorded metadata/proxy coherence, not uninterrupted execution or acceptance by itself. [Confirmation review][confirmation-review], [final assessment][final]

Criterion 95% intervals describe within-process uncertainty; process ranges and standard deviations describe between-process variability. Ratios of marginal endpoints are descriptive, not calibrated ratio confidence intervals. Profiles include callback setup/preflight/calibration and provide no operation-only percentage or Amdahl bound. Incremental percentages are not added, and no weighted-user-workload result is inferred.

RSS uses macOS `time -l` bytes in separate isolated processes: selected initialization, one checked same-case preflight and one checked operation, with matching result lifetime/capture. A claim requires at least 20% median reduction **and every paired reduction**. RSS does not measure allocation counts or copied bytes. OS authorization prevented Instruments allocation capture, so those fields remain null. Private mutation requests, owning-map identity and traversal counts are separate diagnostic facts. Sobol points are dependent; numerical uncertainty uses supported independently scrambled replicates rather than unjustified IID error bars. [Allocation limitation][alloc]

## I1: retain with the explicit typed-control cost

The small presence guard avoids curves-map COW on generic surface/price/series misses. It preserves family precedence, first errors, ordered bumps, credit rebinding and APIs. Thirteen private work cases verify actual mutation requests and owning-map sharing; complete core/Vanna/factor records remain exact.

Eight final latency rows qualify: Money at 8/64/256/1024 curves improves **29.041/48.738/79.063/95.079%**, surface256 **85.924%**, series256 **44.546%**, Vanna1024 **69.572%**, factor-delta1024 **83.730%**. Money8 supplies a small-context result; 1024-curve consumers remain stress cases. These targeted workflows justify retention under the plan's explicit tradeoff rule. [Typed-control review][typed-review]

The final typed-vol256 **whole confirmation** is adverse: aggregate **145.333→169.892 ns**, **16.8985% slower** (+24.559 ns). All three paired points are slower:

| Pair | C0→F1 median ns | Slowdown | Descriptive reduction endpoints |
| --- | ---: | ---: | --- |
| 1 | 145.333→160.097 | 10.159% | [−16.253%, +1.248%] |
| 2 | 141.971→178.353 | 25.627% | [−29.734%, −22.204%] |
| 3 | 165.074→169.892 | 2.919% | [−9.144%, +0.665%] |

The worst observed pair adds 36.382 ns; this is not an upper bound. The third pair does not establish a uniform over-10% penalty, but cannot clear the adverse aggregate or other pairs. Cause and broader incidence remain unresolved. Static tracing shows the typed `VolBucketPct` route directly gets, scales and reinserts the surface, bypassing the generic guard. That route observation does not establish identical emitted code or dismiss measured end-to-end timing as noise. [Tradeoff decision][tradeoff], [typed-control review][typed-review]

The historical I1 confirmation remains **INCONCLUSIVE**, separately: aggregate 135.272→146.956 ns, 8.637% slower; paired reductions +1.058/−23.589/−3.931%, including 131.523→162.548 ns with descriptive slowdown 22.057–26.122%. F1 neither pools with nor rewrites it. No further favorable rerun is used.

The actual-curve control also costs **4.298%** in F1, with all paired points slower and worst adverse endpoint 8.924%. Vanna64's **7.891%** aggregate improvement is unqualified. Retention claims neither typed-heavy superiority, a clean control pass nor an overall unknown-mix gain. [Core statistical review][core-review]

## I2 and I3: retained complete workloads

I2 borrows read-only fixing history from the build context instead of cloning it per floating piece. Complete build/compile/market-resolution/projection/validation remains timed. Twenty history records remain exact. Earlier C1→C2 comparisons qualified three complete term builds with 13.141–33.797% latency reductions and three RSS cases; final cumulative results below are fresh C0→F1 observations. Small-history/one-piece and overnight **latency** rows remain unqualified. Overnight memory benefits do not imply latency gains. [I2 disposition][i2]

I3 narrows original-order accrual slices using ordered payment dates and immutable suffix minima of whole-coupon starts. It retains the active-results Vec, lazy calendar/day-count validation, window-before-formula error precedence, accumulation order and public API. Twelve final rows qualify: prebuilt repeated queries and early/middle/late bands, each at 120/240/480 periods. Earlier C2→C3 prebuilt improvements were 23.221–53.665%; final cumulative results are separate. No per-call rebuild, 60-period query or whole-PV consumer qualifies. [I3 disposition][i3]

Representative cumulative medians are medians of three independent process medians. All listed rows pass every materiality criterion; only the marked term-history row uses whole confirmation.

| Workload | C0 µs | F1 µs | Reduction |
| --- | ---: | ---: | ---: |
| Money bump, 8 curves | 0.325 | 0.231 | 29.041% |
| Vanna, 1024 curves | 14.367 | 4.372 | 69.572% |
| Factor delta, 1024 curves | 11.505 | 1.872 | 83.730% |
| Term build, 16 pieces/2500 extra history; confirmation | 50.329 | 43.460 | 13.649% |
| Term build, 16 pieces/10000 extra history | 64.294 | 42.597 | 33.748% |
| Term build, 64 pieces/2500 extra history | 81.441 | 55.871 | 31.397% |
| Prebuilt accrual, 120 periods | 19.367 | 15.012 | 22.487% |
| Prebuilt accrual, 480 periods | 129.129 | 60.309 | 53.296% |
| Middle accrual band, 480 periods | 132.929 | 63.842 | 51.973% |

Whole F1 confirmations resolve the other selected adverse endpoint concerns: overnight one-piece build is 3.368% slower, all-future term build 0.271% slower; restored LMM forward80/step32/caplet controls remain unqualified without over-10% adverse points/endpoints. They do not clear typed volatility. Bond/loan whole-PV controls are unqualified, with aggregate improvements 0.492/0.139%; the loan's adverse endpoint still reaches 9.546%. Below-threshold adverse observations remain in the full selected data. [Confirmation review][confirmation-review], [final assessment][final]

I3's 480-period index adds **1,920 bytes of Date payload**, excluding Vec/allocator overhead. Its diagnostic traversal's **1,530,000 full versus 4,500 bounded row visits** are test-model counts, not measured production totals or an RSS attribution. Preparation remains charged per build, and overlapping windows may still scan fully.

The three qualified RSS results are all complete history builds; every paired reduction exceeds 20%:

| History workload | C0 median bytes | F1 median bytes | Median reduction |
| --- | ---: | ---: | ---: |
| Term, 16 pieces/10000 extra | 6,782,976 | 4,915,200 | 27.536% |
| Overnight, 16 pieces/10000 extra | 8,339,456 | 6,111,232 | 26.719% |
| Overnight, 64 pieces/2500 extra | 8,552,448 | 5,898,240 | 31.034% |

No final RSS row or pair has a 20% increase. The largest final median increase is 1.748%, largest paired increase 3.819%, both prebuilt accrual480. I1, I3 and model RSS rows have no material benefit. [Final assessment][final]

## I5: rejected despite positive ordinary timings

C5 passed the original 15,046-component scalar panel, path/price bounds, default-PV pin and scoped checks. Additional regressions then exposed previously finite supported public calls: validated finite signed loadings allow original pairwise covariance cancellation before weighting, while reverse weighted tails overflow first. Original exact +2/−2 products cancel with finite contribution scale 8. C5 returned **`[NaN,0]`** for public drift and **`[NaN,−1e160]`** for an aligned zero-shock predictor/corrector step; references/original outputs are finite **`[0,0]`** and **`[0,−1e160]`**. Both tests failed C5 and passed after byte-for-byte C3 restoration. No magnitude cap, fallback, reference or tolerance change was introduced. [Rejection review][i5-review]

The **117 latency/18 RSS** candidate observations remain rejected history, not delivered gains. Eleven ordinary rows crossed timing thresholds; the 10–80-forward sweep improved 44.894–91.186% against C3. Representative rejected results:

| Rejected C5 workload | C3→C5 median ms | C3→C5 reduction | C0→C5 reduction |
| --- | ---: | ---: | ---: |
| Full caplet, 40 forwards/2048 estimators/8 steps | 37.372→6.591 | 82.363% | 82.512% |
| Public single exercise, 20 forwards/2048 estimators | 196.508→95.314 | 51.496% | 50.823% |
| Public two exercise, same size | 196.584→96.004 | 51.164% | 50.559% |

No material RSS benefit/increase was measured. The latest rejection supersedes the earlier statistical keep recommendation. Failed executable/source overlays, passing ordinary records and failure/restoration logs remain archived. **Zero I5 gain rows are accepted**; the measured source uses restored production LMM and passes its exact numerical comparison. The delivered source changes no production LMM mathematics. [Latest I5 disposition][i5], [Delivered source manifest][delivered-source]

## Profiling closures and final validation

**I4:** core kernels show repeated validation/coefficient work, but complete supported QMC profiles prominently resolve normal generation and leave bridge cost unresolved by inlining. They do not establish affirmative whole-price materiality warranting the prepared-grid API change. **I6:** both Heston scalar profiles emphasize characteristic-function preparation and do not establish affirmative whole-price opportunity from sharing only strike phase. Both close without production candidates; gains remain unmeasured, not measured below 10% or proved impossible. Final bridge/QMC/Heston controls qualify no gain. An isolated bridge8 +52.572% pair, alongside a 0.366% aggregate, supplies no candidate gain or selective replacement. [I4 closure][i4], [I6 closure][i6]

Quality01 failed because markdownlint-fix removed **three redundant blank lines in `INVARIANTS.md`**. Nonblank content is byte-identical; original/formatted files and diff are archived. That documentation-only correction changed no native source byte. Quality02 then passed all-fmt but failed gen-check's JSON/Serde residue check on six literals in two test exporters. Both failed attempts remain archived. [Documentation delta][doc-delta], [Quality02 result][quality02]

Quality03 stopped only because markdownlint-fix removed one redundant blank line from the current `INVARIANTS.md`. Substantive concurrent documentation edits were retained; every nonblank line is unchanged. Root accepted the archived blank-only diff; all 3,920 native inputs still match the delivered manifest. [Later documentation delta][doc-delta02]

After all timing, the delivered source changes only the history export keys `payment_lag_business_days`→`payment_lag_days` and `overnight_compounding`→`compounding`, plus four bridge-export type literals: two `GBM`→`gbm`, one `Asian`→`asian` and one `European`→`european`. No production or benchmark source changed. The measured archives and 17-file C0 comparison remain immutable; the separate passed equivalence proof permits carrying their results to delivery under exactly these metadata changes. Quality attempt 04 passed both canonical commands; its 3,920 native entries match the delivered manifest before and after the gate, and protected inputs remain unchanged. The completed disposition preserves all 75 selected latency rows and 11 RSS rows. A final independent arithmetic check confirms seven whole-confirmation selections, 68 short selections and the 23/3 qualification counts. The compact record retains every measurement and decision, referencing the pinned delivery provenance inventory once. [Quality receipt][quality04], [precollection validation][precollection], [machine record][machine]. [Equivalence proof][equivalence]

[plan]: /Users/jeickmeier/Projects/finstack-quant/.plans/CORE-CASHFLOW-MODEL-PERFORMANCE-PLAN.md
[b1]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/B1/completion.json
[b2]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/B2/completion.json
[b3]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/B3/completion.json
[release]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-release-driver-result-01.json
[hosts]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/host-observation-summary.json
[numerics]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-numerical-comparison.json
[final]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/disposition.json
[selection]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-selected-protocol-validation-01.json
[tradeoff]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-control-tradeoff-decision-01.json
[typed-review]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-typed-control-tradeoff-review-01.md
[core-review]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-core-statistical-review-01.md
[confirmation-review]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-confirmation-statistical-review-01.md
[alloc]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/B3/C0-allocations-01/unavailable.json
[i2]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I2/disposition.json
[i3]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I3/disposition.json
[i4]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I4/completion.json
[i5]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I5/disposition-rejected-02.json
[i5-review]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I5/C5-rejection-review-02.md
[i6]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/I6/completion.json
[doc-delta]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/post-measurement-documentation-delta-01.json
[doc-delta02]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/post-measurement-documentation-delta-02.json
[quality02]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/quality-gate-result-02.json
[delivered-source]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/delivered-source-manifest-03.json
[equivalence]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/post-measurement-test-metadata-equivalence-01.json

[native-request]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/B3/F1-native-release-02/request.json

[quality04]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/quality-gate-result-04.json
[machine]: /Users/jeickmeier/Projects/finstack-quant/benchmarks/core-cashflow-model-performance-results.json
[precollection]: /Users/jeickmeier/Projects/finstack-quant/target/core-cashflow-model-benchmarks/execution/F1-final-02/F1-final-precollection-validation-01.json
