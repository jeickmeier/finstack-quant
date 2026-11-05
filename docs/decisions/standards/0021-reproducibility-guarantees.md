# ADR 0021: Explicit and tested reproducibility guarantees

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Code standards

## Decision

Every public stochastic API declares its reproducibility guarantee. Seeds and
relevant execution settings are explicit. Optimizations preserve that
guarantee; weakening it requires explicit agreement.

| Tier | Promise |
| --- | --- |
| Bit-reproducible | Identical numerical bits across the specifically supported execution modes and thread counts. |
| Seed-reproducible | Repeatable results for the documented seed, platform, and execution settings. |
| Statistically reproducible | Tested statistical properties, without promising identical paths or numerical bits. |

Native and WASM numerical results agree within justified, operation-specific
tolerances. Cross-target bit identity is promised only where demonstrated.
Result keys, ordering, error categories, and the identity of a failing input
remain consistent across hosts, with the agreed host representations.

## Rationale

Random streams, execution order, and floating-point reduction order can change
results without changing a public signature. A reproducibility promise must
name the conditions it covers. Cross-target numerical equivalence and
thread-count independence are distinct guarantees with different evidence.

## Examples

| Change or comparison | Treatment |
| --- | --- |
| Run a Monte Carlo calculation promising thread-count independence with one and eight threads | Preserve its promised results. |
| Change the random generator or reduction order and quietly weaken the tier | Prohibited; investigate and obtain agreement for a changed guarantee. |
| Compare native and WASM numerical results | Use justified operation-specific tolerances unless cross-target bit identity is demonstrated. |
| Claim bit identity from tests exercising only one target | Insufficient evidence for a cross-target promise. |
| Parallel evaluation changes which failing input is reported first | Violates the consistent error-selection contract. |

## Enforcement

- Use repeatability tests for the documented seed, target, and execution
  configuration.
- Compare thread counts and execution modes where the API promises identity.
- Exercise cross-host numerical behavior and deterministic result/error
  ordering under the relevant conditions.
- Protect reference expectations and numerical tolerances under
  [ADR 0014](../project/0014-financial-reference-evidence.md).
- Retain behavioral parity under [ADR 0005](0005-python-wasm-parity.md) without
  turning it into an unsupported blanket bit-identity promise.

## Implementation notes and references

[INVARIANTS.md](../../../INVARIANTS.md), section 2, already describes these
tiers, per-target qualifications, explicit seeds, and the prerequisites for
the main Monte Carlo engine's serial/parallel bit identity. The
[Monte Carlo tests](../../../finstack-quant/models/src/monte_carlo/engine/tests.rs)
provide relevant execution checks; their presence does not establish every
cross-target or API-wide promise.

The [models README](../../../finstack-quant/models/README.md) still makes a
blanket native/WASM bit-identity claim. Replace that guidance with the precise
target and execution guarantees supported by evidence. This accepted ADR
resolves the intended policy; the documentation reconciliation and any missing
verification remain pending.
