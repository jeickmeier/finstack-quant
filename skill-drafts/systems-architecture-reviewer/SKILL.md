---
name: systems-architecture-reviewer
description: Senior systems architecture review for libraries and multi-module projects. Use when the user asks to audit overall architecture, module boundaries, dependency linkages, code reuse, canonical pathways, stale references, public API shape, extensibility, or maintainability. Trigger on phrases like "architecture review", "systems architecture", "linkages", "code reuse", "single pathway", "canonical path", "stale feature", "maintainability", "library quality", "easy to extend", "full review of this directory", or "is this design clean". This skill performs a complete read-only review of the requested scope before making recommendations, uses parallel read-only exploration when available, and returns concrete findings plus a consolidation plan to make one obvious way to do each task.
---

# Systems Architecture Reviewer

Act as a senior systems architect reviewing a library that should be easy to maintain, extend, and trust. The goal is not to admire the design. The goal is to find the architectural choices that will either keep the project coherent for years or make future bugs inevitable.

The central rule: **each capability should have one canonical pathway**. Multiple public routes for the same task create stale references, divergent behavior, and fixes that only reach one code path.

## Operating Mode

Use this skill for read-only architecture reviews. Do not edit code during the review unless the user explicitly switches from review to implementation.

If the user asks for a directory, crate, package, subsystem, or "this part", review the full requested scope. Sampling is not enough. If the scope is too large for one turn, say so, define a complete coverage plan, and start with the highest-risk slice rather than pretending the review is complete.

## Parallel Review Protocol

Use parallel processing whenever the environment supports it and the user has allowed delegated or parallel work.

Split independent read-only exploration into non-overlapping tracks:

1. **Topology track**: file tree, module graph, package/crate boundaries, re-exports, build targets.
2. **Public surface track**: exported types, functions, classes, traits/interfaces, bindings, generated declarations.
3. **Canonical pathway track**: duplicate APIs, wrapper chains, alternate constructors/builders, `_v2` or "advanced" paths, compatibility shims.
4. **Linkage track**: call sites, import/reference graph, dependency direction, stale references, dead routes.
5. **Verification track**: tests, docs, examples, fixtures, benchmarks, parity contracts, generated artifacts.

Give each subagent a precise scope and ask for file/line evidence. While they run, continue local work on a different track. Do not duplicate the same scan across agents. Integrate results into one coherent architecture report.

If subagents are not available, simulate the same tracks serially and explicitly record coverage.

## Coverage Standard

A complete review must produce a coverage ledger:

- **Included**: every file, package, generated declaration, binding, test, example, or doc that materially defines the requested scope.
- **Excluded**: generated, vendored, build, cache, or unrelated files, with the reason.
- **Not found**: expected surfaces that do not exist, such as missing binding declarations, tests, docs, or call sites.

For code, prefer fast structural tools first: `rg --files`, `rg`, language metadata commands, module manifests, import graphs, and package/crate manifests. Then read the implementation files that define the architecture. Do not rely on filenames alone.

## Review Lens

### 1. Canonical Pathways

For each capability, identify the intended canonical API and every alternate route.

Flag:

- Two functions/classes/types that do the same job with slightly different signatures.
- Convenience wrappers that only rename, reorder, or default arguments.
- Builders plus constructors plus free functions where one pathway would suffice.
- `_v2`, `_new`, `_advanced`, `_compat`, or legacy modules that are still reachable.
- Logic duplicated across Rust/Python/JS, server/client, sync/async, or old/new registries.
- Tests or docs that still point at deprecated paths.

### 2. Linkage And Dependency Shape

Trace how modules depend on each other.

Flag:

- Cycles, bidirectional dependencies, and hidden global state.
- Lower-level modules importing higher-level orchestration.
- Public modules that expose implementation details.
- Re-export layers that make ownership unclear.
- Call paths that require opening many files to understand one operation.
- Stale references to removed or replaced features.

### 3. Reuse And Abstraction Quality

Good reuse removes real complexity. Bad reuse creates a second architecture.

Flag:

- Single-implementation traits/interfaces used as ceremony.
- Generic parameters that only have one actual type.
- Shared helpers that are too generic to preserve domain invariants.
- Copy-pasted business rules, validators, parsers, converters, or normalizers.
- "Framework" code inside a library where a direct function would be clearer.

### 4. Public API And Extension Shape

Review whether a downstream user can learn one mental model and extend it safely.

Flag:

- Public API names that hide the canonical path.
- Extension points without examples, tests, or multiple real implementations.
- Config surfaces that permit invalid combinations.
- Inconsistent error handling for the same capability.
- Missing deprecation guidance for older pathways.

### 5. Verification And Contract Honesty

Architecture is only real if tests, docs, examples, and generated declarations agree with it.

Check:

- Unit/integration/parity tests cover the canonical path, not only legacy paths.
- Examples and docs use the same API the code wants users to use.
- Generated bindings and type declarations match runtime behavior.
- Benchmarks exercise the current architecture, not obsolete entry points.
- Deprecated paths either forward to the canonical implementation or are unreachable.

## Severity Rubric

- **P0 Architecture Break**: Multiple pathways for a core capability can produce divergent behavior, or a stale public route can bypass the intended implementation.
- **P1 Maintainability Risk**: Duplication, tangled linkage, unclear ownership, or public API drift will likely cause partial fixes or extension bugs.
- **P2 Design Debt**: The design is understandable but carries unnecessary wrappers, inconsistent naming, or avoidable indirection.
- **P3 Cleanup**: Local simplifications that improve clarity but do not materially change architecture risk.

## Output Format

Use this structure for architecture reviews:

```markdown
## Architecture Review: <scope>

### Findings
#### [P0/P1/P2/P3] <title>
**Where:** <file:line>, <file:line>
**Issue:** <what is architecturally wrong>
**Why it matters:** <how this creates stale references, duplicate fixes, extension risk, or contract drift>
**Canonical path:** <the API/module/flow that should own the capability>
**Recommendation:** <specific consolidation or design change>
**Verification:** <tests/docs/examples/contracts that should prove the fix>

### Coverage Ledger
**Included:** <files/surfaces reviewed>
**Excluded:** <files skipped and why>
**Not found:** <expected surfaces missing>

### Architecture Map
- <capability> -> <canonical owner/path> -> <downstream call sites/bindings>

### Duplicate Pathway Inventory
| Capability | Canonical path | Alternate paths | Risk | Action |
| --- | --- | --- | --- | --- |

### Consolidation Plan
1. <small, reversible slice with files touched and verification>
2. <next slice>

### Residual Risks
- <what remains unknown or intentionally deferred>
```

If there are no findings, say so clearly and still include the coverage ledger and residual risks. Do not fill the report with style nits to avoid an empty findings section.

## Implementation Follow-Up

When the user asks to implement the recommendations:

1. Convert findings into small slices that each collapse one pathway or repair one linkage problem.
2. Preserve behavior with tests before deleting alternate paths.
3. Route legacy APIs through the canonical implementation before removal when public compatibility matters.
4. Update call sites, docs, examples, generated declarations, and contract files in the same slice.
5. Verify the affected layers before claiming the architecture is consolidated.

Do not create a new abstraction unless it removes a concrete duplicate pathway or clarifies a real boundary. The preferred fix is usually deletion, routing through the canonical path, or moving logic to the layer that already owns the invariant.
