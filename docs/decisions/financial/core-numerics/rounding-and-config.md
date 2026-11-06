# CORE-NUMERICS-23: Rounding policy, zero tolerances and effective configuration

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`config.rs`, `embedded_registry.rs`, `math/units.rs` and metadata types: rounding/scale policy, generic numerical tolerance, override registries and result stamps. Monetary arithmetic itself is in the separate core-conventions slice; these settings do not automatically change every numerical kernel.

## Choice and rationale

Bankers rounding is the enum default; alternative modes are AwayFromZero, TowardZero, Floor and Ceil. Currency ingest and output scales are distinct. Absolute money-zero tolerance is derived from output precision, while generic zero tolerance is separately configured. Embedded registries give reproducible defaults with an explicit complete override.

## Algorithm

1. Validate currency-specific scale overrides in 0..28. Output scale falls back to currency decimals; ingest falls back to max(6,currency decimals). Generic epsilon defaults to 1e-10 and its constructor/serde boundary requires finite positive value.
2. `RoundingContext::is_effectively_zero(x,Money(ccy))` tests `|x|≤0.5*10^(−output_scale)`; Generic tests `|x|≤generic_epsilon`. Neither is relative to notional.
3. Configuration extensions require at least two lowercase/digit/underscore identifier segments followed by a `v` and one or more decimal digits. Values remain generic JSON; domain builders validate their contents.
4. Embedded registry load parses/builds once and caches either result or error. Config loading uses a matching extension value as the whole document, reparses/rebuilds it per call, and otherwise clones the cached embedded registry. Invalid override errors; there is no fallback to embedded on override failure.
5. Result metadata stamps non-monetary engine mode `f64`, rounding context version 1, package version, no timestamp and serial by default. Overlay takes request rounding/numeric mode, preserves existing optional execution stamps where present and ORs parallel flags.
6. Shared conversion constants are one basis point=0.0001, 10,000 bp per decimal unit and 100 percent units per decimal unit.

## Selection and conventions

Currency overrides and extensions are caller-supplied. Schema mode/parsing does not auto-select financial conventions. A result stamp says what was provided to stamping; it does not prove every calculation consumed it. Generic absolute epsilon and fixed `math::ZERO_TOLERANCE` are distinct sources even though both default to 1e-10.

## Limitations, failure and fallback

`ToleranceConfig` has a public field, so programmatic construction can bypass its constructor; serialization rejects invalid values. Zero tests on NaN are false. The source rationale says a $1M notional would match within $0.0001 at 1e-10, but the implemented generic check is absolute, not a notional-scaled relative test. Config key version validation is syntactic, with no version migration.

## Alternatives and tradeoffs

Currency-specific scale override versus ISO defaults and explicit registry override versus bundled data are supported. Per-kernel tolerances remain explicit caller settings; treating one epsilon as universal relative pricing accuracy is not implemented.

## Evidence

- [Configuration and inline tests](../../../../finstack-quant/core/src/config.rs), [registry implementation/tests](../../../../finstack-quant/core/src/embedded_registry.rs), [configuration integration tests](../../../../finstack-quant/core/tests/infrastructure/config.rs), [units](../../../../finstack-quant/core/src/math/units.rs).
- `config_extension_takes_precedence`, `validation_failure_is_propagated`, and `tolerance_config_rejects_invalid_values_and_roundtrips` cover override and tolerance boundaries; configuration integration cases locate defaults/stamping evidence; these do not establish universal downstream policy enforcement. [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md) is the existing representation decision and retains only its stated accepted scope.

No registry metadata or override presence constitutes approval of its numerical assumptions.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
