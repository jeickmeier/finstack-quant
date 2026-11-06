# ADR 0036: Safe Rust and recoverable library failures

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

Project-authored library and binding code uses safe Rust. Invalid caller
inputs, market data, model state, and ordinary numerical failures return
meaningful typed errors instead of panicking.

Forbid unsafe code in the project's library and binding sources. This rule
does not claim that third-party dependencies contain no unsafe internals.

Preserve the production panic restrictions. Production code must not use
`unwrap`, `expect`, or explicit panics to handle recoverable failures.
Caller-controlled indices and shapes require checked access or explicit
validation. Numerical kernels may index directly where locally established,
tested invariants ensure valid bounds.

Errors identify the attempted operation, failed condition, and relevant input
or identifier. Bindings preserve their meaning through the centralized
adapters under [ADR 0006](0006-structured-errors.md). Operation-specific
policies for permitted missing data and undefined results remain governed by
[ADR 0009](../financial/conventions/0009-missing-and-invalid-data.md).

Panic-based assertions remain appropriate in tests, examples, and benchmarks
where intentional. That target distinction does not permit a library called
by those targets to panic on an ordinary input or computation failure.

This decision defines required behavior. Passing the existing lints alone
does not prove every path is free of input-triggered panics.

## Rationale

A computation library must let its callers handle invalid inputs and failed
calculations without losing control of the host process or request. Safe Rust
protects the project's memory-safety boundary; checked inputs, established
bounds, and meaningful errors address recoverable runtime failures that safe
Rust and panic-family lints alone cannot prevent.

## Examples

| Situation | Treatment |
| --- | --- |
| A caller supplies an out-of-range index or incompatible array shape | Return a meaningful error through checked access or validation. |
| A solver fails to converge | Return a computation error with diagnostic context. |
| A kernel indexes arrays after locally validating their dimensions | Allowed when the bounds invariant is established and tested. |
| An input-dependent constructor assertion rejects malformed data by panicking | Replace the recoverable failure path with an appropriate typed error. |
| Add an unsafe block with a `SAFETY` comment to project library code | Violates the source-code prohibition; a comment does not create an exception. |
| An established dependency uses unsafe internally behind its safe interface | Not prohibited merely by this rule about project-authored sources. |
| A test assertion panics because the observed result is incorrect | Appropriate test failure behavior. |

## Enforcement

- Retain unsafe-code prohibitions on library and binding sources and the
  applicable production panic-family lints, using the shared standards under
  [ADR 0034](0034-code-standards-authority.md).
- Review paths reachable from caller-controlled inputs for unchecked indexing,
  shape assumptions, input-dependent assertions, and other panic sources that
  the existing lints do not comprehensively detect.
- Test invalid inputs, boundary shapes, and ordinary numerical failures through
  the affected public APIs. Verify returned error meaning and context, including
  the corresponding host behavior where exposed.
- Establish and test the local bounds invariants relied on by direct indexing
  in numerical kernels.
- Keep assertions in test, example, and benchmark targets distinct from
  production error handling. Changes to protected checks follow
  [ADR 0016](../project/0016-independent-contract-protection.md).

## Implementation notes and references

- [INVARIANTS.md](../../../INVARIANTS.md), section 5, specifies recoverable
  failure behavior and identifies the limits of current indexing enforcement;
  the outstanding audit and regression work is tracked in
  [FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
- [Workspace configuration](../../../Cargo.toml) denies unsafe code.
- The [core library](../../../finstack-quant/core/src/lib.rs),
  [Python bindings](../../../finstack-quant-py/src/lib.rs), and
  [WASM bindings](../../../finstack-quant-wasm/src/lib.rs) forbid unsafe code
  and apply production panic-family lints with deliberate test allowances.
- [Rust standards guidance](../../../.agents/rules/rust/code-standards.md)
  is governed by this policy. Guidance reconciliation is tracked in
  [FOLLOW_UPS.md](../FOLLOW_UPS.md#reconciliation-items).

Recording this policy does not establish complete panic-path coverage or change
the existing implementations and checks.
