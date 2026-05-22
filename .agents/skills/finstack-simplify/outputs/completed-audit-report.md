# Audit Report: `finstack::example`

**Scope:** `finstack/example/src/module/`
**Bindings in scope:**
- `finstack-py/src/bindings/example/module.rs` (exists)
- `finstack-wasm/src/api/example/module.rs` (missing)
**Auditor:** finstack-simplify / Phase 1 (read-only)

## Executive Summary

The module exposes two public construction paths for the same capability and one binding wrapper that adds no semantic value. Highest-leverage move: collapse callers onto the canonical Rust constructor and delete the wrapper-only pathway.

## Surface Area Inventory

**Capability - construct model**

- Canonical entry point: `Model::builder(id)`
- Alternate pathways found:
  - `build_model(id, config)` - wrapper-only free function
  - Python helper that validates inputs before calling Rust

## Findings

### F1 - [Category: parallel-api]

**Files:**
- `finstack/example/src/module.rs`
- `finstack-py/src/bindings/example/module.rs`

**What:** Two public Rust entry points construct the same model with equivalent semantics.

**Why it's slop:** Callers must choose between names that do not encode different behavior.

**Proposed fix:** Keep the builder, delete the free function, and route bindings to the builder.

**Invariants touched:** parity

**Impact:** H
**Risk:** M
**Tier:** 2

## Binding Drift

**Structural drift:**
- Python exposes a helper not present in Rust or WASM.

**Logic drift:**
- Python performs validation that belongs in Rust.

**Parity contract impact:**
- Update any exposed constructor names.

## Next Steps

Proceed to Phase 2 to plan the consolidation slice.
