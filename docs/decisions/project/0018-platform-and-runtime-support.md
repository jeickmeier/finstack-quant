# ADR 0018: Explicit platform and runtime support

**Status:** Accepted

**Decision date:** 2026-11-04

**Category:** Project policy

## Decision

Maintain an explicit support matrix for Rust, Python, WASM, and UI consumers.
Distinguish build-only, smoke-tested, and fully tested targets. Dropping a
supported target or raising a promised minimum version requires explicit
agreement.

Retain the following Python wheel build coverage as the initial baseline,
with its verification qualifications visible:

| Target | CPython versions | Verification level in the inspected workflow |
| --- | --- | --- |
| Linux x64 | 3.12, 3.13, 3.14 | Build, installation, and installed-wheel smoke checks. |
| Linux arm64 | 3.12, 3.13, 3.14 | Build-only; no native runtime verification. |
| macOS arm64 | 3.12, 3.13, 3.14 | Build, installation, and installed-wheel smoke checks. |
| Windows x64 | 3.12, 3.13, 3.14 | Build, installation, and installed-wheel smoke checks. |

Exact browser and Node version commitments still need specification. A
development-tool version pin alone does not establish the consumer support
policy. Upgrading tools is ordinary work when consumer commitments remain
unchanged.

## Rationale

A dependency or build change can silently narrow portability while the main
CI target stays green. Compiling an artifact does not establish that it runs
correctly, and host-language semantic parity does not establish platform
coverage.

## Examples

| Change or claim | Treatment |
| --- | --- |
| Upgrade a dependency while preserving the agreed matrix | Ordinary implementation work. |
| Drop Python 3.12 to accommodate a new dependency | Requires explicit agreement. |
| Describe a cross-compiled wheel as runtime-verified | Violates the evidence requirement. |
| Introduce browser-only globals into an interface supporting Node | Violates its supported-runtime contract. |
| Upgrade a development tool without changing consumer requirements | Ordinary tooling work. |

## Enforcement

- Compare packaging metadata and CI matrices against approved commitments.
- Test installed artifacts in representative consumer environments, not only
  imports or builds inside the repository checkout.
- Report build-only and smoke-tested coverage accurately; define the required
  evidence before describing a target as fully tested.
- Preserve the explicitly agreed baseline until a change is authorized under
  [ADR 0015](0015-authorized-change-scope.md).

## Implementation notes and references

The [artifact workflow](../../../.github/workflows/build-artifacts.yml)
defines the wheel matrix and explicitly excludes Linux arm64 from native
installation/smoke checks. This records workflow coverage, not the outcome of
a newly executed build or a guarantee that every target has passed full tests.

[Python metadata](../../../pyproject.toml) declares its interpreter minimum.
The [WASM guide](../../../finstack-quant-wasm/README.md) describes browser and
Node usage, and [mise.toml](../../../mise.toml) pins development tools. The
complete consumer matrix, including explicit browser/Node version guarantees
and evidence levels for the other surfaces, remains follow-up work.
