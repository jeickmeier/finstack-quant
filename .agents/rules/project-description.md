---
trigger: always_on
description:
globs:
---

# Finstack Quant

Deterministic financial computation library: a Rust workspace (Decimal money,
currency-safe arithmetic, stable serde wire formats) with Python (PyO3) and
WebAssembly (wasm-bindgen) bindings. All logic lives in Rust; the bindings
convert types, build wrappers and map errors.

Crate map, dependency direction and the cross-cutting invariants:
[`/ARCHITECTURE.md`](../../ARCHITECTURE.md). Numerical, determinism, sign, date
and error rules: [`/INVARIANTS.md`](../../INVARIANTS.md).
