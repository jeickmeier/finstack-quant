# CORE-NUMERICS-23: Explicit rounding configuration, local tolerances and truthful result metadata

- Decision status: **Accepted — complete rounding/configuration/metadata package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent exact Decimal/key-grammar examples and read-only configuration/registry/metadata/caller reviews were completed on 2026-11-08; these do not certify production consumption, replay or financial accuracy.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`config.rs`, `embedded_registry.rs`, `math/units.rs`, the shared `math/mod.rs::ZERO_TOLERANCE` authority and metadata types: rounding/scale policy, generic numerical tolerance, override registries and result stamps. Monetary arithmetic itself is in the separate core-conventions slice; these settings do not automatically change every numerical kernel.

## Accepted decision: explicit rounding config, local tolerances and producer metadata

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host/schema reconciliation and independent numerical/financial/provenance
qualification remain pending; observed behavior is retained below.

1. **Apply already accepted monetary rounding authority without reopening it.**
   Inherit [012](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
   and [0020](../conventions/0020-numerical-representation-and-rounding.md):
   explicit financial scale/mode, no universal RoundingMode default, rename
   AwayFromZero to MidpointAwayFromZero without an alias, and no implicit six-
   decimal ingest. Missing currency ingest override means no ingest rounding;
   an override requires an explicitly selected mode. Output scale/minor-unit
   and nearest-even presentation defaults are display choices, not stored-value,
   equality, exact-zero or wire-amount rules. These are accepted prerequisites,
   not new votes on financial mode or payment rounding stages.

2. **Keep existing config types with coherent guarded state and snapshots.**
   RoundingPolicy mode is optional; empty ingest/output maps and no mode form a
   valid unrequested default configuration. Make rounding fields private and
   validate construction, atomic updates, serialization/restoration and consuming
   boundaries: scales 0..28; nonempty ingest map requires Some(mode). A failed
   update changes nothing. Keep checked CurrencyScalePolicy and ConfigExtensions,
   canonical get_* views and guarded setters, without a new policy hierarchy.
   `get_ingest_scale(currency)` returns Option; `get_output_scale(currency)`
   retains the frozen currency-minor-unit presentation fallback. RoundingContext
   is a private immutable snapshot with the same mode/maps and fixed **version 2**;
   remove its default/tolerance fields. Reject other versions, unknown/obsolete
   fields and duplicate serialized currency keys. No v1 migration or aliases.
   A snapshot records declared controls; producing it does not prove use.

3. **Remove presentation-derived monetary zero classification.** Delete
   ZeroKind::Money, money_epsilon and RoundingContext::is_effectively_zero rather
   than retaining a money-like f64 branch. Money exact zero remains Decimal
   equality under accepted 012. A nonzero cash/principal amount cannot disappear
   because of output scale. Half a display quantum neither defines economic
   zero nor describes rounding-to-zero across the five modes. No substitute
   universal half-unit/materiality helper or hidden settlement quantization.

4. **Retire universal epsilon authority, not justified local tolerances.**
   Remove ToleranceConfig, generic_epsilon, ZeroKind::Generic, the remaining
   shared zero classifier and `math::ZERO_TOLERANCE`. Remove their config/wire/
   host fields and old defaults. Every actual consumer must select exact zero
   where that is its contract, or its own justified comparison with named units,
   absolute/relative meaning and scale. No blanket replacement with zero, a
   copied 1e-10 or a new generic tolerance framework. Existing accepted algorithm
   controls and protected independently justified tolerances remain governed by
   [0042](../../standards/0042-numerical-accumulation-and-comparisons.md) and their
   owners. This also reaches statements comparison/integer/window/forecast
   controls, statements analytics, CDS/CDS-option kernels and bond/inflation
   metrics; their local rules must be reviewed before replacement. In particular,
   this does not adopt exact statements equality through the back door of
   constant removal. This package does not settle every unreviewed threshold.

5. **Reconcile economic cash and attribution decisions explicitly.** Revolving
   credit must retain nonzero draws, principal, interest and posting effects
   until its explicitly selected financial rounding boundary; remove current
   default-display-zero suppression. Attribution/portfolio denominator
   availability and convexity filtering need their owner-specific mathematical
   and reporting rules, not generic f64 or money-zero metadata. Preserve exact
   cash state and accepted accumulation. Reporting omission/materiality and
   near-zero ratio admission require explicit owner decisions; do not silently
   use a presentation scale or transplant an old global constant.

6. **Keep opaque extension transport with exact syntax and duplicate rejection.**
   Preserve the actual key grammar: at least two nonempty ASCII lowercase/
   digit/underscore segments, then `.v` plus nonempty decimal digits. v0/v01
   and digit/underscore-only segments remain syntactically valid; no semantic
   version meaning, known-namespace claim, trimming or case normalization is
   inferred. Reject duplicate serialized extension keys before map/JSON-value
   materialization loses them. Programmatic insert remains an explicit atomic
   replacement returning the prior value. Nested payload values remain opaque
   JSON; unknown valid namespaces are transported, not falsely marked consumed.

7. **Leave extension content and merge policy with actual owners.** A consuming
   owner validates its exact key, payload version/schema, units and financial
   meaning. A key suffix need not equal a document's schema version. Preserve
   independently specified merge/replacement rules: margin overlay and optional
   sensitivity settings are not universally whole-document replacement. Invalid
   consumed configuration must fail under that owner's rules, not silently fall
   back to defaults. Presence, syntactic validity or unused fields do not prove
   an override was validated/applied. No generic extension migration or overlay
   framework is selected.

8. **Simplify embedded loading to one fixed builder and no generic override.**
   Bind one canonical `fn(D)->Result<T>` in each EmbeddedJsonRegistry constructor
   with its fixed embedded text/label; `load()` takes no builder. Remove per-load
   validator arguments, unused Clone bound, extension_key and load_from_config.
   Accepted ratings record 019 already removes the sole production user of that
   generic override path; do not preserve it speculatively or readopt retired
   rating datasets/defaults. Existing domain extension owners keep their own
   explicit contracts. No compatibility load wrapper, alternate validator or
   registry-selection framework.

9. **State embedded cache authority and failure limits honestly.** The same pure
   document-only deterministic builder governs each loader instance. Cache its
   embedded Ok or returned Err in the existing OnceLock; subsequent load calls
   preserve that result/cause without hidden retry, repair or current-data fetch.
   A panic is not a cached returned error; owners must use fallible builders.
   Resolved cached data must be immutable by its concrete domain design; generic
   Clone/Sync alone never proves deep isolation or economic provenance. No live
   registry mutation/reload, new trait hierarchy or certification of the bundled
   financial assumptions is introduced.

10. **Separate requested controls from claimed applied rounding.** Keep request
    configuration with the existing request/provenance owner. In ResultsMeta,
    replace `rounding` with `rounding_applied: Option<RoundingContext>`. None is
    unrecorded/unqualified, not proof no rounding happened. Some is supplied only
    by the producer that can identify its actual effective IO policy and scope;
    it does not assert every map entry or numerical kernel consumed the config.
    Mixed policies need owner provenance rather than a fabricated common stamp.
    No new requested-versus-applied envelope/framework or implicit config use.

11. **Make fresh result metadata contain only known facts.** Change the helper
    to `results_meta()` without cfg. Fresh metadata records the compiled
    non-monetary numeric profile `f64` and current core package version; applied
    rounding, FX policy, execution and timestamp are unrecorded. ResultsMeta's
    Default delegates to those same facts-only semantics. Change parallel
    to Option<bool>: None unknown, Some(false) recorded serial, Some(true) recorded
    parallel for the producer's declared scope. Keep optional FX/time/version
    fields, with nonblank supplied labels and the producer's actual timestamp.
    No automatic clock read or invented serial/FX/rounding fact. Optional None
    writes as absent and restores from absence/null as unrecorded. Make metadata
    fields private/checked; reject unknown fields, unsupported numeric labels and
    invalid snapshots. Restored historical version/time facts stay historical.

12. **Remove request overlays and universal metadata merges.** Delete
    overlay_request and any generic boolean-OR/first-nonempty/request-replaces-
    producer merge. Producers/enclosing workflows record only facts they own;
    request declarations cannot overwrite actual configuration/build/FX/execution
    facts or fill unknown application with a claimed default. Contradictory known
    facts for the same scope error; different child scopes retain their actual
    owner provenance. Do not force one aggregate context or collapse unknown
    parallel execution to false. No generic merge/conflict-resolution engine.

13. **Reconcile producing workflows and replay limits, not just stamp wording.**
    Pass actual effective policy through producers that apply monetary rounding;
    if usage is unverified, leave application metadata unrecorded. Fix the
    valuation registry's compute-first/request-overlay path rather than merely
    renaming its claim. Do not infer an applied FX policy merely from metadata
    on available curves; identify the conversion policy actually used.
    Configuration, profile labels and package version do not
    identify complete producing builds, resolved registry contents, input clocks,
    financial assumptions or all replay dependencies. Existing
    [0025](../../project/0025-auditable-results-and-replay.md) owns immutable
    historical inputs/choices and missing-evidence disclosure. Do not substitute
    current embedded/default data on replay or introduce library-managed archives.

14. **Keep unit constants literal and dimensionally scoped.** Retain one
    definition each for ONE_BASIS_POINT=0.0001 decimal rate,
    BASIS_POINTS_PER_UNIT=10000 and DECIMAL_TO_PERCENT=100. These conversion
    factors do not set observation frequency, rounding, notional scaling or
    price/probability tolerance. Accepted [018](../core-conventions/018-rate-units.md)
    retains checked Rate/Bps conversion and unit-aware text boundaries; no new
    implicit conversion, quantization, currency arithmetic or numeric mode.

15. **Reconcile existing hosts/wire/docs in one later implementation slice.**
    Keep existing FinstackConfig/RoundingMode hosts, canonical Rust/Python/WASM
    getters and validated optional mode/scale semantics. Remove ToleranceConfig,
    its factory/getter/constructor parameters, tolerance schema fields and old
    mode spellings without aliases. Reconcile version-2 snapshot, optional
    applied/parallel result metadata, guarded setters, strict duplicate/unknown
    input handling, generated declarations, parity ledger, wire examples and
    all registry/attribution/revolver/valuation consumers. Add no new host class,
    binding math, migration or compatibility serializer.

16. **Qualify the actual boundaries independently.** Cover exact nonzero money
    versus display, all mode/scale/ingest combinations, invalid atomic updates/
    restored state/duplicates, removed zero/tolerance consumers, extension syntax
    versus domain content/unused keys, fixed builder/cache Ok/Err and immutable
    data, unknown versus recorded producer facts and same-scope conflicts.
    Bounded Decimal/grammar examples, source inspection and located tests do not
    qualify runtime ingestion, parameter consumption, financial materiality or
    replay. This review changes no runtime, tests, fixtures, registry data,
    dependencies, generators, schemas or hosts.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Default config, missing currency ingest override | Preserve admitted Money precision; no financial mode or six-decimal rounding inferred. |
| Ingest override with absent mode / scale 29 | Invalid policy/update; previous config unchanged. |
| Output-only scale override with no financial mode | Valid presentation control; no ingest or wire-amount rounding. |
| Exact USD .004, output scale 2 versus 3 | Same nonzero amount/cash effect; no output-dependent economic-zero classification. |
| USD .005 MidpointAway / -.001 Floor / .009 TowardZero at scale 2 | Financial round gives .01 / -.01 / 0; one inclusive half-quantum test cannot define all three. |
| Snapshot version 1 or obsolete tolerance field | Reject; only the selected version-2 snapshot, without migration. |
| Extension `a.b.v0` / `1._.v01` | Syntax valid, financial schema/version/consumption not established. |
| Duplicate serialized extension/currency keys | Input error before last-wins materialization; deliberate API replacement remains explicit. |
| Embedded registry called from different consumers | Same constructor-bound builder/cache result; callers cannot select validators. |
| Former generic registry override method | Removed; rating explicit-data and other domain extension policies retain their owners. |
| Fresh results_meta() | Known library profile/version; applied rounding/FX/parallel/time unrecorded. |
| Metadata parallel absent / false / true | Unknown / recorded serial / recorded parallel, without implicit OR. |
| Request policy differs from producing computation | Do not relabel applied policy; use actual producer facts or disclose unrecorded application. |
| Missing historical registry/build evidence | Report replay limits; do not load today's defaults as historical proof. |

## Observed choice and rationale

Bankers rounding is the enum default; alternative modes are AwayFromZero, TowardZero, Floor and Ceil. Currency ingest and output scales are distinct. Absolute money-zero tolerance is derived from output precision, while generic zero tolerance is separately configured. Embedded registries give reproducible defaults with an explicit complete override.

## Observed algorithm

1. Validate currency-specific scale overrides in 0..28. Output scale falls back to currency decimals; ingest falls back to max(6,currency decimals). Generic epsilon defaults to 1e-10 and its constructor/serde boundary requires finite positive value.
2. `RoundingContext::is_effectively_zero(x,Money(ccy))` tests `|x|≤0.5*10^(−output_scale)`; Generic tests `|x|≤generic_epsilon`. Neither is relative to notional.
3. Configuration extensions require at least two lowercase/digit/underscore identifier segments followed by a `v` and one or more decimal digits. Values remain generic JSON; domain builders validate their contents.
4. Embedded registry load parses/builds once and caches either result or error. Config loading uses a matching extension value as the whole document, reparses/rebuilds it per call, and otherwise clones the cached embedded registry. Invalid override errors; there is no fallback to embedded on override failure.
5. Result metadata stamps non-monetary engine mode `f64`, rounding context version 1, package version, no timestamp and serial by default. Overlay takes request rounding/numeric mode, preserves existing optional execution stamps where present and ORs parallel flags.
6. Shared conversion constants are one basis point=0.0001, 10,000 bp per decimal unit and 100 percent units per decimal unit.

## Observed selection and conventions

Currency overrides and extensions are caller-supplied. Schema mode/parsing does not auto-select financial conventions. A result stamp says what was provided to stamping; it does not prove every calculation consumed it. Generic absolute epsilon and fixed `math::ZERO_TOLERANCE` are distinct sources even though both default to 1e-10.

## Observed limitations, failure and fallback

`ToleranceConfig` has a public field, so programmatic construction can bypass its constructor; serialization rejects invalid values. Zero tests on NaN are false. The source rationale says a $1M notional would match within $0.0001 at 1e-10, but the implemented generic check is absolute, not a notional-scaled relative test. Config key version validation is syntactic, with no version migration.

## Observed alternatives and tradeoffs

Currency-specific scale override versus ISO defaults and explicit registry override versus bundled data are supported. Per-kernel tolerances remain explicit caller settings; treating one epsilon as universal relative pricing accuracy is not implemented.

## Evidence

- [Configuration/state/stamping](../../../../finstack-quant/core/src/config.rs),
  [embedded loader](../../../../finstack-quant/core/src/embedded_registry.rs),
  [unit constants](../../../../finstack-quant/core/src/math/units.rs) and
  [shared zero constant](../../../../finstack-quant/core/src/math/mod.rs) establish
  observed behavior. Located override/tolerance/scale/stamp and
  [configuration integration tests](../../../../finstack-quant/core/tests/infrastructure/config.rs)
  were inspected/located, **not executed**. Existing public state, nominal zero
  claims and request overlays do not prove applied-policy authority.
- Accepted [012](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
  already settles the five modes, absent ingest override, explicit financial
  mode, precision and presentation boundaries. Pinned
  [rust_decimal strategy documentation](https://docs.rs/rust_decimal/1.43.0/rust_decimal/enum.RoundingStrategy.html)
  distinguishes midpoint-away from directed-away behavior; no new mode is
  inferred from its naming. [OnceLock initialization](https://doc.rust-lang.org/std/sync/struct.OnceLock.html#method.get_or_init)
  supports one initializer/cached-result analysis; it does not authenticate
  builder purity, economic data or deep immutability.
- Independent **2026-11-08** exact Decimal examples checked 135 mode/zero-region
  boundary cases over scales 0/2/28, nonzero USD .004 under changed display scale,
  midpoint/floor/truncation counterexamples and minimum-Decimal versus half-
  quantum limits. Separate bounded examples checked absolute-versus-relative
  epsilon units, exact extension-key grammar and duplicate raw key rejection.
  These did not call production config/registry/metadata or host APIs.
- Actual zero consumers include [revolving-credit cashflow generation](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/cashflow_engine.rs),
  [attribution results](../../../../finstack-quant/attribution/src/result.rs),
  [convexity filtering](../../../../finstack-quant/attribution/src/metrics_based/rates.rs)
  and [portfolio attribution](../../../../finstack-quant/portfolio/src/attribution.rs).
  Generic source units differ; existing reporting/financial ownership cannot be
  replaced by one exact-zero or epsilon recipe. The cash suppression and unit
  concerns are static findings, not executed financial impact reproductions.
- Fixed-zero-constant reconciliation additionally reaches
  [statements evaluation](../../../../finstack-quant/statements/src/evaluator/formula.rs),
  [statements backtesting](../../../../finstack-quant/statements-analytics/src/analysis/backtesting.rs),
  [CDS pricing](../../../../finstack-quant/valuations/credit/src/cds/pricing/engine.rs),
  [CDS-option quadrature](../../../../finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs)
  and [bond quote conversion](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/compute.rs),
  alongside other forecast/vintage/scenario and inflation consumers. These links
  locate owner review work, not newly selected equality or pricing thresholds.
- [Valuation registry stamping](../../../../finstack-quant/valuations/core/src/pricer/registry.rs)
  computes before applying request metadata; actual policy consumption must be
  reconciled. [Ratings](../core-conventions/019-credit-ratings.md)
  already retires the only production generic config-override loader consumer.
  [Margin registry](../../../../finstack-quant/margin/src/registry/mod.rs) and
  [sensitivity settings](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/config.rs)
  have separate owner merge policies. Existing
  [Python](../../../../finstack-quant-py/src/bindings/core/config.rs),
  [WASM](../../../../finstack-quant-wasm/src/api/core/config.rs) and
  [parity ledger](../../../../finstack-quant-py/parity_contract.toml) identify
  host removal/state/docs work; no production bindings were rebuilt here.

Configuration and metadata presence are declarations or producer claims,
not universal numerical accuracy, source authority or complete replay proof.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
