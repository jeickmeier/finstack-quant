# Simplicity Auditor — Reference

This file contains deeper heuristics and patterns to use when running a simplicity audit. Prefer the smallest change that produces a durable reduction in surface area and duplication.

## Duplicate detection heuristics (practical)

### Textual / structural signals

- Repeated blocks with the same variable names or the same comment/doc wording.
- Same sequence of operations repeated (parse → validate → normalize → compute → format).
- Multiple modules implementing the same algorithm with small variations (often parameter defaults).
- Same “domain concept” represented by multiple types with overlapping fields.

### “Near-duplicate” signals (harder to spot)

- Two functions differ only by:
  - one extra parameter that always takes a default at call-sites
  - one validation check
  - one error mapping step
  - ordering of steps that doesn’t change semantics
- Same function exists in multiple layers (core + wrapper + bindings), each “adding convenience”.

### Cross-language duplication (Rust ↔ Python ↔ WASM/TS)

- Binding layers replicate business logic (not just marshaling/shape conversion).
- Multiple APIs exist because of binding constraints (prefer fixing binding shape over duplicating logic).
- Public surface differs across languages for the same capability (“two libraries in one” smell).

## API smells (simplification targets)

- **Many entry points** for the same job (no clear canonical path).
- **Configuration explosion**: nested config structs with many optional fields for common usage.
- **Boolean parameters** that switch “mode” (prefer enum with explicit variants, or split into two distinct capabilities if semantics differ).
- **Stringly-typed options**: magic strings controlling behavior.
- **Leaky abstractions**: public types exist mainly to support internal implementation detail.
- **Versioned names**: `_v2`, `_ex`, `_advanced` (prefer one API with clear semantics + migration).
- **Inconsistent error strategy** for the same capability (panic vs `Option` vs `Result`).
- **Wrapper towers**: layers of forwarding functions that obscure the real entry point.

## Consolidation patterns (preferred)

### 1) Collapse into a canonical function + private helpers

- Pick one public entry point per capability.
- Move all variants into:
  - a private helper function
  - a private module
  - a small internal enum representing a behavior choice
- Keep public signature stable and obvious.

### 2) Make “safety behavior” part of the canonical API

If “try_*” variants exist mostly for validation:

- Prefer one canonical API that validates inputs and returns a `Result` (and prefer the name `new` over `try_new` when this is a constructor you’re converging toward).
- If a truly infallible API is needed, make it a thin wrapper that is *unambiguously safe* and justified (ideally by taking validated/typed inputs rather than “hoping” inputs are correct).
- Only keep both `new` and `try_new` if the distinction is genuinely semantic (infallible vs fallible) and improves clarity; otherwise, collapse to a single `new` pathway and delete the `try_*` variant.

### 3) Replace families of convenience APIs with a single ergonomic default

- If `do_x_with_config` is the only one used correctly:
  - make config optional with good defaults; keep one entry point.
- If `do_x()` and `do_x_with_config()` both exist:
  - ensure `do_x()` calls the canonical implementation with explicit defaults.
  - consider deprecating one if it causes confusion.

### 4) Normalize naming and capability boundaries

- Choose one naming convention and enforce it across modules.
- Prefer capability-centric modules (one place to find the “one way” API).
- Avoid “utility grab bag” modules becoming de facto public API; keep helpers private unless truly general.

### 5) Remove “feature shadow APIs”

If APIs exist only because of feature flags or historical layering:

- Prefer a single API whose behavior is feature-dependent internally.
- Ensure documentation clearly states feature effects.

## Refactor sequencing (low-risk default)

1. **Lock behavior**: add golden tests / invariant tests for each capability.
2. **Merge internals first**: unify duplicate logic behind the scenes without changing public APIs.
3. **Collapse public surface**: deprecate and migrate; keep old paths calling the canonical one.
4. **Delete**: remove deprecated APIs and dead code; simplify docs/examples.

## “Don’t add complexity” guardrails

- Do not introduce a new abstraction unless it **removes more code than it adds** (net negative LOC or clear cognitive reduction).
- Do not add new public types to “organize” complexity; keep complexity private.
- Do not keep wrappers “for convenience” if they:
  - hide important semantics
  - create ambiguity about the preferred entry point
  - diverge from canonical behavior over time

## Deprecation strategy (public libraries)

- Add deprecation warnings with clear “use X instead” guidance.
- Provide mechanical migrations (search/replace guidance).
- Keep deprecations long enough for downstream migration (align to release cadence).
- Avoid “soft forks” where both APIs must be maintained indefinitely.
