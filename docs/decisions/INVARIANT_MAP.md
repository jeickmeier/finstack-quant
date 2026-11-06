# Invariant-to-decision map

This map covers every normative clause in [INVARIANTS.md](../../INVARIANTS.md),
including requirements in prose and tables outside **Required** lists. The
catalogue retains the operational wording; this map identifies the corresponding
accepted decision and its precise scope.

**Accepted** means an agreed decision covers the stated rule. The review of
invariant-detail decisions 0042–0049 is complete. Acceptance does not approve
source-observed alternatives outside their stated scope or certify complete
enforcement. Coverage here is documentary traceability, not proof of
implementation or numerical validation; financial methodology adoption remains
separate.

Each row names a distinct rule or an inseparable set of conditions. Row IDs
support review; the linked catalogue section supplies the full wording. A change
to a clause updates its row and corresponding decision together. Evidence-only
descriptions, API names used as examples, and bibliography entries are not
additional decisions; their associated requirements and evidence standards are
mapped below. Open implementation, reconciliation and verification work remains in
[FOLLOW_UPS.md](FOLLOW_UPS.md#invariant-decision-review).

## 1. Money and model numerics

Source: [representation](../../INVARIANTS.md#11-representation) and
[arithmetic and aggregation](../../INVARIANTS.md#12-arithmetic-and-aggregation).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| M01 | Model curves, rates, volatilities, correlations, returns, greeks, optimizers and paths may use `f64`. | [0020] | Accepted |
| M02 | New accounting, ledger, settlement, regulatory-reporting and margin-dispute paths use Decimal/Money without an intermediate `f64` round-trip; existing gaps do not permit new ones. | [0020] | Accepted |
| M03 | Wrapping an `f64` result in Money supplies currency and Decimal storage, not Decimal-exact preceding arithmetic. | [0020] | Accepted |
| M04 | Preserve sub-minor-unit precision; Money construction is not implicit ISO quantization, and currency-scale rounding is explicit at its required boundary. | [0020] | Accepted |
| M05 | Reject non-finite monetary inputs. | [0020] | Accepted |
| M06 | Use Decimal where rounding affects contractual amounts, posted balances, settlements, regulatory submissions or disputes. | [0020] | Accepted |
| M07 | Analytical/model PV, risk and totals may aggregate in `f64` with documented conversion boundaries and rounding. | [0020] | Accepted |
| M08 | For floating-point totals, use the shared Neumaier primitives for mixed-sign cash flows, large dynamic ranges or potentially material floating-point error. OnlineStats serves online mean/variance, not compensated totals; required monetary paths stay Decimal. | [0042]; [0020] governs Decimal paths | Accepted |
| M09 | Validate finiteness before Kahan/Neumaier accumulation; compensation cannot repair non-finite values. | [0042] | Accepted |

## 2. Determinism and floating point

Source: [tiers](../../INVARIANTS.md#21-reproducibility-tiers),
[RNG](../../INVARIANTS.md#22-random-number-generation),
[map order](../../INVARIANTS.md#23-map-ordering),
[reductions](../../INVARIANTS.md#24-reductions), and
[comparisons](../../INVARIANTS.md#25-comparisons).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| D01 | Every public stochastic API declares its tier and supported target/execution conditions. | [0021] | Accepted |
| D02 | Same-target reproducibility does not imply cross-target bit identity; stronger promises require demonstrated supported conditions. | [0021] | Accepted |
| D03 | Host `f64` comparisons use explicit justified tolerances unless supported cross-target bit identity is demonstrated. | [0021], [0014] | Accepted |
| D04 | Recapture numerical baselines on their originating target and record the relevant build/runtime/execution conditions needed to interpret or reproduce them. | [0043] | Accepted |
| D05 | Result structure, keys, ordering, error categories and failing-input identity agree across hosts. | [0021], [0005] | Accepted |
| D06 | Ordered parallel maps select the lowest-index failing item independently of target/thread count; hash iteration cannot choose results, reductions or errors. | [0043]; [0021] governs consistency | Accepted |
| D07 | Library RNG seeds are explicit; do not use `thread_rng()` or `rand::random()`. | [0021] for explicit seeds; [0043] for the concrete restriction | Accepted |
| D08 | Primary-engine bit identity at fixed declared inputs/configuration requires splittable per-path streams, path-count/chunk-configuration partitions independent of worker/thread count, fixed merge order and disabled adaptive CI stopping. Default chunking depends only on path count; explicit chunk size stays fixed for comparison. | [0043] | Accepted |
| D09 | Philox alone is insufficient; non-splittable sequences do not qualify for that parallel tier. Sobol/Halton observations are scoped evidence, not broader RNG approval. | [0043] | Accepted |
| D10 | Public serialization/file/iterator/snapshot/golden map order uses IndexMap populated in deterministic logical order or sorted BTreeMap; hash iteration or worker completion must not determine insertion order. | [0043] | Accepted |
| D11 | Internal hash maps are permitted only where their iteration order cannot escape. | [0043] | Accepted |
| D12 | Reductions promising bit identity across supported modes/thread counts use deterministic partitions and a fixed merge tree at fixed declared inputs/configuration; statistics and compensated accumulators remain inside that order for the promised tier. | [0043] | Accepted |
| D13 | Compensation improves accuracy without independently guaranteeing bit reproducibility. | [0042] | Accepted |
| D14 | Do not replace cancellation, dynamic-range and materiality analysis with an arbitrary term-count threshold. | [0042] | Accepted |
| D15 | Approximate comparisons name their tolerance semantics, including the applicable absolute, relative or domain scale. | [0042]; [0014] governs justification | Accepted |
| D16 | Exact numerical equality is limited to intended IEEE semantics, including zero sentinels and the same deterministic operation; bit-reproducibility tests compare to_bits representations rather than ordinary equality. | [0042] | Accepted |
| D17 | Do not cite a nonexistent public comparison helper. | [0038] | Accepted |

### 2.6 Portfolio evaluation execution

Source: [portfolio execution](../../INVARIANTS.md#26-portfolio-evaluation-execution).
[0022] accepts the shared ownership and state policy; [0046] accepts
the detailed execution conditions. Implementation and verification remain open.

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| P01 | Valuation, attribution, scenarios, batches, replay and factor-stress ordinary pricing enter one request executor; adapters do not own another pricing loop, assembler, cache or scheduler. | [0022], [0046] | Accepted |
| P02 | Immutable market and portfolio/instrument states are registered separately; changed markets, dates or instruments receive distinct identities. | [0022], [0046] | Accepted |
| P03 | Each immutable market state has fresh rate and hazard recalibration caches; option clones share only within that state, never across states or requests. | [0046] | Accepted |
| P04 | Job identity includes the exact metric menu, strict-risk and base-currency policy. Report structural inapplicability; strict risk fails applicable calculations. Empty metrics remain PV-only, best effort cannot satisfy strict risk, and rejected cross-currency inputs are checked before FX lookup. | [0022], [0046] | Accepted |
| P05 | An explicitly supplied immutable prior may seed a new request only with compatible profiles, portfolio shape and pricing configuration, an established parent-to-updated-market relationship, and an authoritative change set. Otherwise value afresh. | [0046] | Accepted; configuration and originating-market compatibility checks remain open |
| P06 | Separate native repricing from base-currency refresh. Changed conversion inputs refresh every reused position; proven unchanged inputs need no redundant refresh. Without an authoritative FX path manifest, any changed FX quote invalidates all FX-dependent instruments. | [0046] | Accepted |
| P07 | One portfolio execution wave chooses at most one states/positions/factors Rayon axis; nested portfolio axes are forbidden. | [0046] | Accepted; instrument-internal threading coordination remains open |
| P08 | Collect indexed parallel results before a serial logical-order scan, preserving position/state order, first error and Neumaier aggregation order. | [0046], [0043] | Accepted |
| P09 | Evaluation state/caches are request-local; no global, thread-local, process-wide, persistent or cross-request valuation cache. Immutable decoded inputs, static immutable registries and UI presentation caches are outside the market-dependent financial-cache scope. | [0022], [0046] | Accepted |
| P10 | Python may retain typed borrow guards outside detached closures; native closures capture only Rust state without requiring whole-portfolio clones. | [0046] | Accepted |
| P11 | Copy required Python strings before detaching for native computation, JSON work and materialization; Python object extraction/construction remain attached. | [0046] | Accepted; shared JSON copy compliance remains open |
| P12 | Migrated workflows delete superseded compute paths, legacy/new switches, dormant fallback engines and duplicate schedulers. | [0004], [0022], [0046] | Accepted |
| P13 | Financially distinct reducers may remain but route ordinary position valuation through the shared executor. | [0022], [0046] | Accepted |

## 3. Sign and perspective conventions

Source: [perspectives](../../INVARIANTS.md#31-canonical-perspectives) and
[boundary conversions](../../INVARIANTS.md#32-required-boundary-conversions).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| S01 | Name the financial perspective and convert explicitly when crossing perspectives. | [0007], [0044] | Accepted |
| S02 | Preserve all five conceptual meanings: entity/holder receipts, portfolio contributions, non-negative statement magnitudes, additive roll-forward activity and collateral held by the bank. These labels need not all be Rust types. | [0044] | Accepted |
| S03 | Dietz contributions are positive; XIRR/MWR investor contributions are negative; conversion negates the amount. | [0044] | Accepted |
| S04 | Real-estate CapEx schedules take positive outflow magnitudes, the pricer negates once, and validation rejects already-negative schedule inputs. | [0044] | Accepted |
| S05 | Retained-earnings dividends default to non-negative magnitudes subtracted by the identity; an explicitly selected signed convention adds them directly. Never infer the convention from the value or apply `abs()`. Corkscrew equity reductions are negative activity. | [0044] | Accepted; dividend enforcement remains open |
| S06 | Roll-forward increase/decrease buckets are positive magnitudes converted to positive/negative activity respectively. | [0044] | Accepted |
| S07 | SA-CCR collateral/NICA are signed net amounts positive when held by the bank; margined replacement cost is `max(V-C, TH+MTA-NICA, 0)`. Total recognized adjusted collateral C already includes NICA; explicit non-overlapping components prevent double counting. | [0044] | Accepted; public-field reconciliation and full margined/unmargined methodology qualification remain open |
| S08 | CDS-option Call denotes payer protection and Put receiver protection. | [0044] | Accepted |
| S09 | Positive portfolio quantity is long; attribution gains are positive for that holder. | [0044] | Accepted |
| S10 | Sensitivity signs follow the applicable MetricId contract. | [0010], [0007], [0044] | Accepted |
| S11 | Binding documentation preserves Rust polarity. | [0005], [0007], [0038] | Accepted |
| S12 | Avoid asset-class-ambiguous long/short aliases for PayReceive; use pay/receive or an explicit protection side. | [0044]; [0003] governs canonical naming | Accepted |

## 4. Date, day-count, and time conventions

Source: [clock roles](../../INVARIANTS.md#41-clock-roles),
[discount bridge](../../INVARIANTS.md#42-discount-factor-bridge), and
[date boundaries](../../INVARIANTS.md#43-date-boundaries).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| C01 | Keep discount, projection, accrual and option/volatility clocks distinct; do not infer one from another. | [0045] | Accepted |
| C02 | Obtain exact relative discount factors using curve dates and the curve's base date/day count. | [0045] | Accepted |
| C03 | Locate dates using the projection curve's clock; express forwards on the contractual index basis using its accrual fraction. | [0045] | Accepted |
| C04 | Accrual follows the instrument/leg day count. | [0045] | Accepted |
| C05 | Option/volatility time follows an explicit calibration date-to-expiry mapping, including reference date and time basis, through metadata or a documented model contract. No universal fallback; defaults need operation-specific evidence. | [0045]; [0008] governs defaults | Accepted; metadata representation and missing-metadata behavior remain open |
| C06 | Dated-window CAGR defaults to Actual/365 Fixed without redefining frequency annualization, XIRR or instrument clocks. | [0008] | Accepted |
| C07 | Bridge a relative curve DF to a single model clock with `r=-ln(DF)/t` for finite positive DF and t; use DF directly when possible. The identity preserves that factor, not entire model dynamics; expiry and invalid-input behavior remain explicit. | [0045] | Accepted; helper enforcement and caller adoption remain open |
| C08 | Keep valuation, curve base, trade, spot/settlement, fixing and payment date roles distinct. | [0045] | Accepted |
| C09 | Rebase seasoned curves using relative date-based discount factors `D(b,T)/D(b,v)`. | [0045] | Accepted |
| C10 | Settlement applies the contractual spot lag, calendars, business-day convention and end-of-month rule. | [0045] | Accepted |
| C11 | Fixing requirements follow an explicit valuation/publication cutoff. Required historical fixings use configured observations; missing required fixings fail without silent curve projection. | [0045] | Accepted; same-day fixing reconciliation remains open |
| C12 | Credit survival and discount may use different day counts, bridged with date-based survival probabilities and DFs. | [0045] | Accepted |
| C13 | Published numerical error bands require a reproducible benchmark/golden setup and recorded execution evidence with revision, command, inputs, comparator, error units and relevant execution conditions. Distinguish measured error, sampling uncertainty and tolerance; a named setup is not a domain-wide bound. | [0045], [0048], [0014], [0040] | Accepted |

## 5. Error handling

Source: [lints](../../INVARIANTS.md#51-enforced-lints),
[failure behavior](../../INVARIANTS.md#52-required-failure-behavior), and
[indexing](../../INVARIANTS.md#53-indexing).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| E01 | Preserve safe Rust and production panic restrictions; named current lint settings are enforcement evidence, not complete coverage. | [0036], [0034] | Accepted |
| E02 | Explicitly enumerate project-owned error variants where meaning is assigned, including categories, codes, required diagnostics and conversions. Owner classification forces review of additions; propagation and canonical delegation need no duplicate tables. Respect cross-crate non-exhaustive matching and document required third-party unknown-error boundaries; the current lint alone does not enforce this contract. | [0049]; [0006] governs error meaning | Accepted; classification, adapter and enforcement coverage remain open |
| E03 | Intentional panic assertions in tests/examples/benchmarks do not permit recoverable library failures to panic. | [0036] | Accepted |
| E04 | Return recoverable domain errors with diagnostic context and preserve structured meaning through centralized host adapters. | [0036], [0006] | Accepted |
| E05 | Document the actual public errors callers handle; do not invent nonexistent error types. | [0038], [0006] | Accepted |
| E06 | Caller-controlled shapes/indices require checked access or explicit preconditions; direct kernel indexing requires a locally established, tested invariant. | [0036] | Accepted |

## 6. Test discipline

Source: [test discipline](../../INVARIANTS.md#6-test-discipline).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| T01 | Protect behavior/invariants with regressions that fail without the fix and identify the failure mode. | [0039] | Accepted |
| T02 | Expected financial results and tolerances are independently justified and protected. | [0014], [0016] | Accepted |
| T03 | Test each promised execution mode/thread-count class; tolerance checks do not establish bit identity. | [0021] | Accepted |
| T04 | Scoped and full checks use shared definitions and the canonical workflow. | [0034] | Accepted |
| T05 | Golden metadata identifies provenance, methodology/version, valuation date, conventions, inputs, tolerances with rationale and expected units for each case. Explain genuinely inapplicable fields; method, inputs, units and tolerance rationale remain required. | [0048]; [0014] governs independence | Accepted |
| T06 | Cases may inherit fixture, test-header or golden-suite README metadata, with explicit case overrides and traceable applicability. | [0048] | Accepted |
| T07 | Rust/Python may share goldens without requiring Rust crate test-directory storage; shared runs, location and metadata completeness do not establish independent reference sources or correctness. | [0048] | Accepted |

## 7. Public API, bindings, and deprecation

Source: [host parity](../../INVARIANTS.md#71-host-surface-parity) and
[deprecation](../../INVARIANTS.md#72-deprecation-policy).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| H01 | Selected financial capabilities have corresponding Python/WASM interfaces with explicit justified host exceptions; not every internal Rust function needs exposure. | [0005] | Accepted |
| H02 | Host names follow canonical Rust naming. | [0003] | Accepted |
| H03 | Coordinated changes update affected Rust, registration, stubs/exports, bindings/facade declarations, examples and parity contracts together. | [0003], [0005], [0038] | Accepted |
| H04 | Zero backlog and meaningful behavioral coverage are protected independently of editable surface checks/ceilings. | [0005], [0016] | Accepted |
| H05 | Approved development replacements remove obsolete callable paths, subject to authorization of established-contract changes. | [0004], [0015] | Accepted |
| H06 | Decide final-release compatibility before the first final release; its support window/deprecation/migration choices remain open. | [0004] | Accepted requirement to decide; choices remain open |

## 8. Credit factor model schema versioning

Source: [credit-factor schema](../../INVARIANTS.md#8-credit-factor-model-schema-versioning).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| W01 | Require CreditFactorModel's typed schema field with CreditFactorModelSchema::CURRENT identifying `finstack_quant.credit_factor_model/1`; reject missing, malformed and unsupported markers during deserialization, without first-error priority or a schema_version alias. | [0047]; [0012] governs wire meaning | Accepted |
| W02 | The Rust wire root and schema reject unknown root fields; the marker identifies the exact contract. | [0047] | Accepted |
| W03 | Preserve the v1 schema file and published URI independently of Rust ownership moves. | [0047] | Accepted |
| W04 | Plain deserialization does not establish full semantic validity; consumers validate the current state, or use a validating loader, before financial use and after relevant mutation. | [0041], [0047] | Accepted; consumer enforcement remains open |
| W05 | Python/WASM JSON loaders and calibration entry points retain semantic validation. Ordinary from_json and resource-limited from_slice_strict have distinct loading/reporting contracts. | [0047], [0041], [0005] | Accepted |
| W06 | Authorized development root changes replace v1 in place, propagate to all consumers/artifacts and retain no compatibility branch or alternate marker; final-release policy remains separate. | [0004], [0012], [0047] | Accepted |
| W07 | Only explicitly open nested types omitting both Rust/schema closed-object restrictions accept new keys without a root version bump. | [0047] | Accepted |
| W08 | CreditCalibrationDiagnostics accepts and discards unknown keys; known fields retain requiredness and meaning, including computational fold_ups. New fields affecting computation require coordinated semantic changes. | [0047] | Accepted; fold_ups validation coverage remains open |
| W09 | Field removals, type/meaning changes, required-field additions and root additions originate in canonical Rust serde types, regenerate v1 deterministically and propagate throughout the workspace. | [0012], [0047] | Accepted |

[0047] resolves the former field/failure-timing discrepancy in favor of required
typed `schema` and marker rejection before successful deserialization. Semantic
validation coverage and guidance reconciliation remain in [FOLLOW_UPS.md](FOLLOW_UPS.md#reconciliation-items).

## 9. Authority and enforcement

Source: [catalogue preamble](../../INVARIANTS.md) and
[authority](../../INVARIANTS.md#9-authority-and-enforcement).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| G01 | Accepted ADRs own agreed policy/scope/rationale; detailed rules and API documentation have designated canonical homes. | [0013], [0034] | Accepted |
| G02 | Related links do not expand approval; existing requirements remain unless specifically superseded, and a proposed mapping does not independently change them. | [0013], [0015] | Accepted |
| G03 | Canonical Rust wire types, parity ledgers and derived schemas retain their designated structural ownership. | [0012], [0005], [0013] | Accepted |
| G04 | Types, lints, configuration and tests supply evidence, not independent authorization to redefine contracts. | [0013], [0016], [0034] | Accepted |
| G05 | A specifically accepted decision resolves a conflict within its scope; otherwise preserve competing evidence and obtain an authorized scoped decision. | [0013], [0015] | Accepted |
| G06 | Code, generated results and passing self-modified tests cannot authorize changed requirements. | [0014], [0015], [0016] | Accepted |
| G07 | Evidence names supporting sources and covered conditions; policy acceptance, implementation observation and executed validation remain distinct. Source inspection or test presence does not establish a passing run or complete enforcement. | [0048]; [0013], [0038] govern accurate documentation and [0026] defines methodology statuses | Accepted |
| G08 | Outstanding implementation, reconciliation and verification belong in FOLLOW_UPS, with catalogue limitations where needed for use. | [0034], [0013]; [0026] for methodologies | Accepted |
| G09 | Contracts and their checks require independent protection. | [0016] | Accepted |

## 10. References

Source: [references](../../INVARIANTS.md#10-references).

| ID | Retained invariant | Corresponding decision | Coverage |
| --- | --- | --- | --- |
| R01 | SIMM implementation and goldens pin the actual methodology and effective parameters including overrides. Use a published calibration tag where available, otherwise exact edition and immutable source/parameter identity; "current" and wire-format versions are insufficient. | [0048]; [0026] governs methodology evidence | Accepted |
| R02 | Model modules and golden fixtures identify the exact applicable edition, calibration, convention and benchmark, with explained inapplicability and effective parameter provenance. Citations do not adopt or authorize methodology upgrades; no new runtime metadata API is required. | [0048]; [0014], [0026] govern evidence | Accepted |

Bibliography entries support these rules and the relevant financial decisions;
listing a source does not adopt every method or regulatory scope it describes.

[0003]: standards/0003-canonical-public-names.md
[0004]: project/0004-development-compatibility.md
[0005]: standards/0005-python-wasm-parity.md
[0006]: standards/0006-structured-errors.md
[0007]: financial/conventions/0007-units-and-sign-conventions.md
[0008]: financial/conventions/0008-financial-defaults-and-cagr.md
[0010]: financial/valuations-framework/0010-metric-identifiers.md
[0012]: standards/0012-serialized-contracts.md
[0013]: project/0013-documentation-sources.md
[0014]: project/0014-financial-reference-evidence.md
[0015]: project/0015-authorized-change-scope.md
[0016]: project/0016-independent-contract-protection.md
[0020]: financial/conventions/0020-numerical-representation-and-rounding.md
[0021]: standards/0021-reproducibility-guarantees.md
[0022]: architecture/0022-portfolio-evaluation-state.md
[0026]: financial/governance/0026-methodology-decisions-and-inventory.md
[0034]: standards/0034-code-standards-authority.md
[0036]: standards/0036-safe-rust-and-recoverable-failures.md
[0038]: standards/0038-substantive-api-documentation.md
[0039]: standards/0039-behavior-and-invariant-tests.md
[0040]: standards/0040-measured-performance-decisions.md
[0041]: standards/0041-type-invariants-and-validation-boundaries.md
[0042]: standards/0042-numerical-accumulation-and-comparisons.md
[0043]: standards/0043-deterministic-ordering-and-monte-carlo-execution.md
[0044]: financial/conventions/0044-financial-perspectives-and-boundary-conversions.md
[0045]: financial/conventions/0045-financial-clocks-and-date-boundaries.md
[0046]: architecture/0046-portfolio-execution-operational-contract.md
[0047]: architecture/0047-credit-factor-wire-and-validation-contract.md
[0048]: project/0048-reference-provenance-and-version-pinning.md
[0049]: standards/0049-explicit-public-error-matching.md
