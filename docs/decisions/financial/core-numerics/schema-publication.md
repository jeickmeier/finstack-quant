# CORE-NUMERICS-26: Canonical schema publication, compact authoring views and bounded validation

- Decision status: **Accepted — complete schema/publication/compact-view package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Production evidence inspected/located, not executed**. Independent schema/float counterexamples and read-only registry/transform/publication/diagnostic/caller reviews are reported below; they do not qualify production generation, filesystem operations or financial admission.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`schema/*` and `bin/gen_core_schemas.rs`: Rust-derived JSON Schema generation, current structural externalization, deterministic artifact synchronization, current LLM projection and union-error expansion. Schema support is feature-gated; validation additionally requires `jsonschema-validate`. This machinery describes/validates data shapes, not financial model correctness. The accepted replacement preserves original schema graphs and removes unsupported equivalence/provider-subset claims.

## Accepted decision: canonical schemas, compact views and bounded publication

Accepted on **2026-11-08** after whole-record review. Inherit accepted
[0012's Rust-owned contracts](../../standards/0012-serialized-contracts.md),
[024's exact wire/content identity](numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity),
[025's loading/report boundaries](contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
and [0049's error ownership](../../standards/0049-explicit-public-error-matching.md).
Runtime, actual generator/host/schema reconciliation and independent qualification
remain pending; observed source behavior is retained separately below.

1. **Keep Rust-owned schemas with explicit dialect and contract direction.**
   Use SchemaSettings::draft2020_12().for_deserialize() explicitly for one
   canonical wire-admission schema per registered type, instead of assuming
   schemars defaults and overwriting a dialect header afterward. Keep existing
   Serialize/Deserialize/JsonSchema capability requirements for these registry
   roots; those traits do not prove shape/runtime/semantic equivalence. Registered
   artifact roots remain schema objects, with boolean subschemas preserved.
   Input/Output/Component describe catalogue roles, not separate generator modes
   or proofs every serialized output satisfies every admission rule. Intentional
   output-only views outside this registry remain under their own accepted
   contracts. Do not introduce shadow host schemas or an extra schema hierarchy.

2. **Validate metadata and independent identity axes at every public boundary.**
   Require nonblank authored titles/descriptions, normalized owner-relative
   paths and exact absolute fragment-free versioned schema IDs. Assign declared
   root title/description metadata deliberately; reject conflicting preexisting
   root $id/$schema rather than silently rebasing a different graph/dialect.
   Preserve nested IDs/anchors/scopes from the owning Rust schema and reject
   unsupported declared resource dialects. IDs are symbolic offline resource
   identities, not claims that a web host serves them or immutable content hashes.
   Schema-index version1, family version directories, wire/snapshot versions,
   compact-view version and canonicalc2 remain separate axes. Generation/drift
   agreement does not authorize an unreviewed contract change.

3. **Preserve local derived graphs and remove generic externalization.**
   Retain each type's local $defs, recursion, applicator placement, reference
   targets and literal data. Remove ExternalSchemaDefinition, generic
   externalize_schema_definitions/validation_view, reachability pruning,
   with_packager callbacks and canonical single-oneOf collapse. All current
   packagers only externalize definitions; keep actual owner constraints in
   their Rust JsonSchema/serde implementation. Remove orphan sharing tables/
   aliases, retaining separately registered component artifacts where currently
   useful. Do not replace local definitions by remote IDs or introduce a schema
   equivalence/graph-rewriting engine. Larger duplicate definitions are an
   explicit accepted size tradeoff for a smaller authority boundary. The
   completeness guarantee is a closed offline corpus, not that every file stands
   alone: preserve any owner-authored cross-document references and require their
   targets in that corpus, without introducing new external references.

4. **Use one immutable offline corpus without overwrite or retrieval fallback.**
   Generate each registered artifact once per corpus build, validate before
   publication, and fail the whole corpus on duplicate IDs, unsupported dialects,
   malformed resources or required reference-resolution failures. Never build
   the map with last-wins insertion or hand the compiler duplicate URI entries.
   Register original resources with the existing jsonschema dependency; preserve
   URI/base/anchor/dynamic-reference semantics in that maintained resolver.
   Install an explicit deny-all fallback retriever so Cargo feature unification
   cannot enable HTTP/file fetching. No current-data lookup, partial-corpus
   fallback, custom URI resolver or live reload. Build in private local stages:
   capture all graphs/metadata/examples, assemble the complete resource snapshot,
   validate final examples/owner admission against that same local snapshot, then
   publish or cache success. Never call the public cached corpus/schema-validation
   path recursively from an initializing example/owner callback. Add no public
   unchecked rendering route or second corpus authority. Cache actual corpus success/
   failure with the existing process-lifetime pattern; cached labels do not
   authenticate financial inputs or certify a standard's complete implementation.

5. **Require unambiguous exact artifact selection.** Validate single-artifact
   metadata at direct generate; enforce global ID and owner-namespace path
   uniqueness at index/corpus/find boundaries that have the full inventory, not
   only CLI entry. Keep exact full-ID, owner-relative-path and
   complete-filename selectors where supported; require one match. No match is
   NotFound, multiple matches an explicit ambiguity/Validation failure. Reject
   blank selectors and partial suffixes; never select the first match, normalize
   IDs, infer a domain or substitute a fuzzy hint. Catalogue arrays retain their
   declared stable order; index rows use the declared deterministic ordering.

6. **Publish exact declared examples without generic financial quantization.**
   Remove stabilize_floats, twelve-significant-digit rounding, negative-zero
   normalization and rounded-parser fallback. Capture examples through accepted
   024 checked serialization before raw nonfinites can become null; preserve
   admitted finite bits, strings, integers and explicit field sentinels. Author
   deterministic literals/constructor inputs. Replace computed pricing examples
   that cannot meet the publication promise with owner-authored qualified values,
   not universal rounding. Each final example must pass the final canonical
   schema/offline corpus and its actual typed owner admission/semantic checks;
   a pre-transform T deserialization alone proves neither. Reject bad examples
   before any file write. Examples illustrate admitted shapes, not financial
   defaults, parameter completeness, market calibration or pricing accuracy.

7. **Keep one deterministic publication renderer distinct from content hashing.**
   Explicitly sort object keys recursively by decoded UTF-8 bytes using the
   existing Value operation, independent of preserve_order feature unification.
   Preserve arrays, finite numeric forms, signed zero and Unicode/string data.
   Publish pretty JSON with two-space indentation, LF and one final newline;
   all promised string/byte host views and files use that same renderer. Object
   views promise structure, not identical host reserialization bytes. Canonical
   c2 compact bytes/hashes remain a separate accepted identity contract. Dependency
   or owner changes affecting schemas/examples/bytes require reviewed expected
   changes; do not infer cross-platform equality from discarded significant digits.

8. **Retain explicit CLI modes and complete preflight.** Require exactly one
   write/check/list, with one optional output-root and no unknown/repeated flags.
   List validates/sorts the inventory without rendering callbacks or filesystem
   mutation. Check is read-only and reports deterministic changed/missing/extra
   owned outputs. Before write, render/validate the entire intended schema/index/
   associated owned fixture set and inventory all destinations/deletions. Reject
   duplicate paths, overlapping roots, ancestor/file-directory collisions and
   misplaced index outputs. Use specific nonoverlapping roots below schemas/;
   index ownership is explicit and its metadata file stays below schemas/.
   A separate existing fixture synchronizer has an explicit nonempty suffix and
   owner root; no schema command acquires an unrelated directory implicitly.

9. **Enforce filesystem containment and truthful I/O failures.** Resolve the
   explicit base once for Check/Write; it must already exist as a directory.
   Creating validated owned subdirectories after preflight is permitted, creating
   a missing base is not. List need not inspect the base filesystem. Allow
   legitimate links in the chosen base/ancestors during resolution, then reject
   symlinks/special files in owned roots, intermediate directories, output/index
   leaves and stale-file candidates. Validate normalized relative paths by
   components, not string prefixes. Also require canonical authored raw relative
   spelling: slash-separated nonempty segments, no dot/dotdot/empty segments,
   backslashes, drive/prefix forms or leading/trailing separators. Component
   normalization must not admit a/./b or a//b aliases. The selected output base
   remains a native path resolved once. Inventory without following links; keep
   unrelated suffixes/files and every path outside declared ownership. Propagate
   permission/read/metadata errors instead of treating them as missing or using
   read().ok() to proceed with overwrites. Require a quiescent/exclusively used
   output tree; these checks are not a race-proof or hostile-filesystem sandbox.
   No new approval step or generalized filesystem security framework.

10. **Stage complete files and publish the index last.** After all preflight,
    stage changed files in their destination directories using the existing
    maintained temporary-file dependency, then atomically replace each file.
    Preserve existing regular-file permissions and normal nonexecutable creation
    policy for new artifacts; clean temporary files. Staging failure leaves
    original artifacts unchanged. Replace artifacts first, remove only inventoried
    owned stale matching-suffix files afterward, prune only empty owned nested
    directories, and publish the index last. A commit-phase I/O failure may leave
    mixed complete files; return failure and require check/reconciliation before
    treating that set as published. No full-set transaction, concurrent-reader,
    crash durability or power-loss guarantee, rollback/archive framework, or
    successful partial-publication claim is selected.

11. **Make index and byte-count facts refer to the rendered set.** Reuse the
    once-rendered documents/bytes for files, index rows and comparisons; record
    actual byte lengths with accepted024 count admission where annotated.
    Catalogue kind/summary/domain/path/ID describe the registered artifact;
    they do not approve model choice, defaults or live data. Keep version1 index
    semantics unless its own contract changes, without a global version bump
    caused merely by c2/config changes. Querying a partial or conflicting corpus
    fails rather than publishing a misleading complete index. List output remains
    artifact-path inventory and does not claim the index was rendered/validated.

12. **Replace the lossy llm profile with a compact authoring view.** Retain the
    useful second view under the explicit profile name **compact**, alongside
    canonical, and rename project_llm to compact_schema. Remove the llm spelling,
    LlmProfile/inline-budget/resolver knobs and old constants without aliases.
    One fixed technical profile shortens schema description strings to at most
    240 Unicode scalar values, with a marked ellipsis included in that cap;
    no token/context-window/provider compatibility or total-document-size claim.
    Full canonical descriptions remain authoritative for units/conventions.
    Keep the original $id solely to preserve reference-base meaning; label the
    root x-finstack-view=compact-v1 and x-finstack-canonical-id with its source ID.
    Require an untagged canonical source; reject preexisting reserved view/source
    labels or an already compact view rather than overwrite extension data.
    Do not publish/register it as a second canonical resource under that ID or
    silently substitute it for the canonical artifact. No new envelope/class.

13. **Limit compaction to known schema description locations.** Traverse
    schema-valued keywords and the values of schema maps such as properties/
    patternProperties/$defs/dependentSchemas, not those maps' literal member
    names. Visit combinator/prefixItems schema arrays and recognized single-
    schema positions; preserve boolean subschemas. Never reinterpret objects
    inside const/enum/default/examples, unknown keyword payloads or opaque
    extensions as schemas. Preserve IDs, anchors, dialect declarations, references,
    graph topology, defaults, examples and every assertion. No enum folding,
    tuple position/tail rewrite, forced required/additionalProperties, nullable
    widening, format/uniqueItems removal, ref hoisting/inlining, pruning or handle
    substitution. Provider-specific subsets/placeholders require an actual owner
    integration outside this generic library; no hydrator is invented here.

14. **Keep canonical validation explicit and the compact view non-authoritative.**
    Validate against the original registered schema and offline corpus with
    explicit Draft202012 settings and the existing dependency. Enable its
    supported standard-format checks; retain unsupported format labels as
    annotations and state they are not validated or substitutes for type/range
    or owner checks. Freeze/review this compiler profile with the locked
    dependency; reject unsupported declared dialects/required vocabularies rather
    than relabeling or silently falling back. Do not validate a compact-tagged
    document as the canonical authority. Accepted024/025 lossless/bounded ingress
    precedes generic Value conversion for actual text/native APIs. Schema
    acceptance cannot enforce original duplicate/integer-token spelling, exact
    Decimal representability or all cross-field/financial constraints; actual
    typed owner validation is still required before usable domain admission.

15. **Use the root compiled validator's errors without branch reconstruction.**
    Delete lifted-branch compilation, hand-followed reference locations, tag/
    closest-branch selection and scalar-const-union message replacement. Keep
    original canonical validation context and dependency-produced instance paths;
    a more general but factual union error is acceptable. Do not manufacture a
    branch explanation with changed reference/evaluation scope or hide a multiple-
    matching-oneOf failure. Use existing compiled-context diagnostics, not another
    resolver/equivalence engine. Compile/resolution failures are real failures,
    not an empty successful error list or a successful weaker validation path.

16. **Reuse accepted025 bounded reports with independent validity.** Replace
    the separate Failure/Vec-only schema diagnostic surface and its serializers
    by the existing ValidationReport/Diagnostic authority. Determine canonical
    validity independently of retained detail, including cap0; preserve fatal
    status/truncation under limits/restoration/merges. Retain bounded original
    root errors in producer order with escaped pointers, stable schema-failure
    codes/Structure phase and truthful context. Full dependency prose need not
    be byte-stable. Diagnostic caps do not bound validation, regex or callback
    work; trusted registered schemas and caller resource budgets remain required.
    Preserve central NotFound/Validation/Computation distinctions and causes
    under0049; no Python catch-all ValueError or duplicated host classifier.

17. **Reconcile actual registries, generators, hosts and feature boundaries.**
    Update all five domain packagers, component tables, umbrella corpus/index/
    find/render/validate paths, generation CLIs and owned fixture synchronizers.
    Existing Python/WASM schema.get/profile/validate APIs must agree on canonical/
    compact names, renderer guarantees and complete report shapes; update stubs,
    declarations, docs/examples, parity ledger and approved generated expectations
    in the later implementation slice. Keep Rust authority, existing feature
    gates and clear unsupported-feature errors; never skip validation silently
    when jsonschema-validate is absent. No old llm/externalizer/Failure aliases,
    provider integrations, new host math, migration or compatibility schemas.

18. **Qualify the actual outputs and limits independently.** Cover schema-
    versus-instance locations, literal/property keyword names, anchor/scope and
    unevaluated applicators, duplicate IDs/ambiguous selectors, exact boundary/
    signed-zero/sentinel examples, final example admission, preserved compact
    assertions, offline failures, format limits, root-validity/cap0 reports and
    oneOf zero/multiple matches. Publication evidence must cover byte parity,
    all-feature map ordering, list/check nonmutation, symlink/special-file/error
    preflight, staged writes/index-last and failure cleanup/partial-commit limits.
    Drift/roundtrips and shared-writer equality are not independent conformance,
    financial adoption or complete JSON-Schema implementation proof. This
    review changes no runtime, dependency, generated artifact, test, schema
    fixture, host or filesystem publication.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Property literally named description / const object with description or oneOf | Preserve the field/data; it is not a schema annotation/applicator at that location. |
| `$ref` beside unevaluatedProperties | Preserve original placement/evaluation scope; no allOf splitting. |
| Local anchor referenced by `#node` | Retain its definition and original reference; no limited-name pruning. |
| Duplicate corpus ID / two matching filenames | Fail explicitly; no overwrite or first-match selection. |
| Conflicting reserved compact labels / already compact source | Reject; never overwrite opaque data or recursively compact a view. |
| Relative output path a/./b or a//b / nonexistent output base | Reject raw path aliases / reject Check/Write base; no silent creation. |
| Open-unit example nextDown(1), confidence nextUp(.5), float -0 | Preserve admitted values/sign; no twelve-digit normalization. |
| Required reference unavailable offline | Resolution failure; no network fetch or successful weaker schema. |
| Existing symlink output or unreadable owned target | Preflight failure before publication; do not follow/treat as missing. |
| Staging failure / later commit I/O failure | Original files unchanged / explicit failure with possible mixed complete files, requiring check. |
| Native/string schema views | One declared renderer for promised bytes; ordinary object reserialization has structural guarantees only. |
| canonical / compact / llm profile | Canonical authority / labeled description-only view / reject retired spelling. |
| Optional nonnullable field / heterogeneous tuple | Preserve omission/null distinctions and exact prefix/tail constraints in both views. |
| Duplicate string const branches in oneOf | Preserve oneOf semantics; no enum folding that makes an invalid multiply matching instance valid. |
| Invalid payload with diagnostic cap0 | Fatal report true, no retained detail, truncation true; never empty-list success. |
| Valid schema shape with invalid financial cross-field state | Typed owner still rejects; schema success is not domain admission. |

## Observed choice and rationale

Schemars derives canonical schemas from Rust/serde types. Shared definitions are externalized only after an expanded assertion view matches. The LLM projection intentionally relaxes some canonical constraints and requires separate canonical validation. Keeping these two surfaces distinct is necessary to avoid treating a generation aid as the authoritative contract.

## Observed algorithm

1. A SchemaArtifact generates Rust schema, sets versioned ID/dialect/title/description, applies its packager, collapses a single-object oneOf only when all sibling keys are annotations, and appends example values. Examples serialize actual values and round floating examples to 12 significant digits; this affects examples only.
2. Externalization removes annotations/IDs/defs for comparison, recursively expands local references including suffixes, marks recursive revisits symbolically and combines ref siblings using allOf. Only matching assertion trees may replace local refs with external URIs. Prune definitions unreachable from root references afterward.
3. Generator CLI requires exactly one of write/check/list. Validate normalized relative paths, non-overlapping owned schemas roots and unique IDs/paths; render pretty JSON plus newline in deterministic ordered maps. Check compares expected bytes/missing/extra generated files. Write updates changed files, deletes extra matching-suffix files under owned roots, and removes empty nested directories. No schema generation was run in this audit.
4. LLM projection defaults to 16 KiB per-definition inline budget and 240 description characters. Resolve external refs in sorted order; inline small schemas, replace oversized ones with typed descriptive handles, preserve unresolved refs, rewrite known refs and prune unreachable definitions. Oversized local definitions become handles too (only when budget>0).
5. Fold eligible string-const unions to enum, shorten Rustdoc descriptions while preserving selected standards citations, drop nonportable keywords including format/uniqueItems/default, describe omitted defaults, replace fixed tuples by a fixed-length homogeneous/union items schema, and make every object property required with additionalProperties=false. This tuple rewrite loses positional type constraints; handle substitution loses nested validation constraints.
6. Diagnostic expansion handles anyOf/oneOf failures to depth four, follows references, prefers branches whose const tags match, otherwise selects the failing branch with fewest failures, and prefixes nested instance pointers. If resolution/building fails, retain the original error. This is a heuristic explanation of canonical validation failure, not altered acceptance logic.

## Observed selection, controls and conventions

Canonical generation and LLM projection are explicit APIs. Output-root and profile budgets are caller controls; artifact registry chooses domain types/examples. `max_inline_bytes=0` skips local oversized substitution but makes resolved nonempty external documents exceed the external-inline budget; it is not a uniform unlimited-budget convention.

## Observed limitations, failure and fallback

The current structural assertion-view comparison is not proof of logical equivalence: its all-object traversal can strip a property literally named description or change const data; ref-sibling allOf splitting changes annotation-sensitive unevaluated behavior. IDs/anchors are resolution semantics, and limited-name pruning can remove anchored definitions. Canonical oneOf collapse shares the literal-data traversal problem and can invalidate incoming pointer targets. External typed comparison does not verify the actual published document at the target URI. LLM projection is lossy, can retain unresolved refs, hoists colliding definitions first-wins and preserves the canonical ID on a changed contract; forced required fields do not actually gain the nullable alternatives claimed in comments. No handle hydration consumer was found. Example twelve-digit rounding can move valid boundary values out of their domain and loses negative zero. Corpus insertion/selector first-match and map-feature-dependent bytes need explicit authority. Filesystem walking follows directory symlinks; direct writes/read().ok() and index-first paths do not establish containment, atomic files or complete publication. Lifted diagnostic branches can lose their original schema context; unbounded Vec details cannot establish cap-independent fatality. Canonical Rust/schema/runtime validation equivalence still requires independent qualification.

## Observed alternatives and tradeoffs

Publishing canonical schemas directly retains constraints but can exceed downstream generation-tool limits. Projection trades completeness for size/shape compatibility; it must not replace canonical validation. Strict registry path validation and deterministic drift checks are implemented; no remote schema retrieval or automatic semantic validation is added by this module.

## Evidence

- [Registry](../../../../finstack-quant/core/src/schema/registry.rs), [externalization](../../../../finstack-quant/core/src/schema/externalize.rs), [projection](../../../../finstack-quant/core/src/schema/llm.rs), [generator](../../../../finstack-quant/core/src/schema/generator.rs), [diagnostics](../../../../finstack-quant/core/src/schema/diagnostics.rs), [schema tests](../../../../finstack-quant/core/src/schema/tests.rs).
- `packaging_rejects_name_only_shape_substitution` covers assertion mismatch; `packaging_compares_recursive_typed_definition_graphs` covers recursive comparison; `projection_rewrites_tuples_and_drops_non_portable_keywords` covers intentional projection loss; `generator_renders_once_and_preserves_unowned_files` covers publication/drift boundaries. These are representation/transform invariants and do not prove every generated schema matches all domain constructors. JSON Schema dialect is explicitly draft 2020-12 in source; external standards conformance was not independently assessed here.

Successful schema validation does not approve a financial methodology.

- [Schema settings/registry/examples](../../../../finstack-quant/core/src/schema/registry.rs),
  [current union diagnostics](../../../../finstack-quant/core/src/schema/diagnostics.rs),
  [umbrella corpus and hosts' authority](../../../../finstack-quant/src/schema.rs)
  and [serialized-contract policy](../../standards/0012-serialized-contracts.md)
  establish actual source/caller boundaries. All five unique with_packager
  callbacks only externalize definitions; no separate owner assertion was found
  there. Existing llm profile APIs are active, but no provider/placeholder
  materialization consumer was found. Tests were inspected/located, **not run**;
  no schema generator or filesystem publication was executed.
- The [JSON Schema 2020-12 Core specification](https://json-schema.org/draft/2020-12/json-schema-core)
  defines resource IDs, reference scope, applicators and unevaluated annotations;
  the [Validation vocabulary](https://json-schema.org/draft/2020-12/json-schema-validation)
  distinguishes assertions, instance-valued constants and annotations. These
  semantics motivate preserving original graphs, not a claim of general
  equivalence from structural expansion. Pinned [schemars1.2.2 settings](https://docs.rs/schemars/1.2.2/schemars/generate/struct.SchemaSettings.html)
  expose explicit draft2020_12/for_deserialize and warn that default dialects
  may change. Locked jsonschema0.28.3 already supports explicit draft/resources/
  retriever/format settings and original-context validation; these capabilities
  were inspected in the pinned dependency, not inferred from newer API docs.
  Existing tempfile3.27 is available in the workspace; its generator feature/
  dependency scope must be reconciled if used in production tooling.

- Independent **2026-11-08** checks used the separately installed Python
  jsonschema4.26.0 Draft202012Validator, not production Rust/host/generator
  APIs: **17 acceptance assertions** and a missing-anchor resolution failure
  covered literal/property keyword names, ref/unevaluated scope, anchor pruning,
  duplicate-oneOf/enum differences, open tuple tails and optional/nonnullable
  properties. Standard-library nextafter/format checks demonstrated both
  nextDown(1)→1 and nextUp(.5)→.5 under twelve-digit rounding, plus the signed-
  zero distinction. These are bounded counterexamples/reference targets, not
  full conformance, final production-example admission or filesystem evidence.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
