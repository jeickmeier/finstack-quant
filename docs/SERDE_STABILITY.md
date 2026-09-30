# Serde Contract Policy

This policy covers public Rust types whose JSON is persisted, exchanged with
bindings, or published as a JSON Schema. The library is pre-alpha: the sole
supported contract is v1, and contract corrections are made in place before
release.

## Source of truth

Every contract follows one direction:

```text
Rust runtime serde type -> JsonSchema derive -> registry -> generated artifact
```

A schema root must be a real `Serialize + Deserialize + JsonSchema` type used
by runtime code. A `*Wire` type is permitted only when serialization or
deserialization actually passes through that type because domain storage
cannot express the wire representation directly.

Checked-in schemas, fixtures, Python stubs, TypeScript declarations,
notebooks, and documentation are consumers of the Rust contract. They do not
override it.

## Pre-release change policy

Contract fixes replace v1 in place and propagate through every direct
consumer in the same change. Do not create a new version, compatibility alias,
deprecated wrapper, alternate key, migration function, or missing-marker
fallback.

A contract change is complete only when all of these agree:

- Rust field and enum names;
- serde keys and tags;
- derived JSON Schema assertions;
- constructors, builders, tests, examples, and canonical fixtures;
- Python snake_case APIs and stubs;
- WASM camelCase APIs, facade, and declarations;
- parity contracts, notebooks, benchmarks, and documentation.

Old spellings and unsupported markers may appear only in focused rejection
tests.

## Marker policy

Namespaced contracts use a required typed marker:

```json
{"schema":"finstack_quant.<contract>/1"}
```

Numeric contracts use only:

```json
{"schema_version":1}
```

`finstack_quant_core::wire::SchemaVersion` accepts exactly the JSON integer
`1`. It rejects a missing field, strings, zero, and every unsupported integer.
No root uses a bare `version` field.

The maintained root inventory and artifact links are in
[`CONTRACTS.md`](CONTRACTS.md).

## Naming policy

- Persisted keys, enum tags, and enum values use snake_case.
- Rust and Python names are identical snake_case.
- WASM exposes the corresponding camelCase name.
- Use `frequency`, `day_count`, and `business_day_convention`.
- Use `*_currency`, `*_bp`, `as_of_spreads`, and `vol_surface_id`.
- Use `credit_curve_id` for every hazard-curve reference: instrument
  dependencies, function inputs, and outputs that echo them (for example the
  cashflow envelope). It always names a `HazardCurve`, never a risky discount
  curve.
- Fix names at their canonical Rust source. Never retain a bad name through an
  alias or binding-only rename.

Externally standardized labels such as ISO currency codes, rating symbols,
and agency codes keep their prescribed spelling.

## Representation policy

| Concept | JSON representation |
|---|---|
| Date | `YYYY-MM-DD` string with JSON Schema `format: date` |
| Decimal | string matching `^-?\d+(\.\d+)?([eE][+-]?\d+)?$` |
| Schema revision | integer constrained to exactly `1` |
| Enum/tag | canonical snake_case string or tagged object |
| Rust/Python API | canonical snake_case |
| WASM API | corresponding camelCase |

Exact decimal fields reject JSON numbers. Floating-point fields remain JSON
numbers and must reject non-finite values at serialization and semantic
validation boundaries.

Day-count labels are exactly `act_360`, `act_365f`, `act_365l`, `nl_365`,
`30_360`, `30e_360`, `30e_360_isda`, `act_act`, `act_act_isma`, and
`bus_252`.

## Object strictness

Input and configuration structs use `#[serde(deny_unknown_fields)]`. Optional
fields are explicit `Option<T>` values; omission and `null` are equivalent only
when the runtime serde type says so. Collection defaults are used only when
omission is a documented canonical behavior.

Open content is limited to named extension maps such as metadata. An open map
does not make its containing object open.

The three pricing-override maps are distinct fields:
`instrument_pricing_overrides`, `metric_pricing_overrides`, and
`scenario_pricing_overrides`. They default to empty when omitted, are omitted
when empty, and reject `null`.

## Serialize-only public output views

Some public result and binding-view types are intentionally one-way outputs.
They are computed from canonical inputs and are never accepted as persisted
request documents. The current public inventory contains 19 one-way types:

- attribution: `ReturnContributionResult`, `InstrumentContribution`,
  `GroupContribution`, `FactorContribution`, and
  `BenchmarkRelativeContribution`;
- model factor-risk views: `PositionEsContributionView` and
  `ParametricEsDecompositionView`;
- allocation outputs: `WeightAllocationResult`, `StrategyAllocation`, and
  `AllocationDiagnostics`;
- scenario views: `ScenarioRevalueView` and `ScenarioPnlView`;
- sensitivity wire form: `SensitivityMatrixJson` (bidirectional, no schema);
- statement-analysis outputs: `CreditAssessmentPoint` and `CreditAssessment`;
- calibration validation views: `CalibrationValidationReport`, `DependencyGraph`, and
  `DependencyNode`;
- optimization output: `PortfolioOptimizationResult`.

This list is based on effective trait support and is enforced by
`uv run python -m scripts.serde_audit`. Adding or removing a one-way output
requires updating the audit classification and this inventory together.

## Schema ownership

Each schema-owning crate has one sorted registry. A registry entry owns:

- the runtime Rust type;
- artifact path;
- unique `$id`;
- title and description;
- deterministic examples.

The shared emitter may add `$schema`, `$id`, title, description, and examples.
It may also apply rewrites that leave the asserted contract identical:

- externalize equivalent `$defs` references, and remove definitions that
  become unreachable as a result;
- collapse a single-branch `oneOf` into that branch, when the wrapper carries
  no assertion of its own. schemars emits a one-variant enum this way, and the
  wrapper is not free: a validator reports a failing `oneOf` at the union node,
  so an error inside the branch is reported against the whole subtree with the
  instance attached rather than against the field that failed.

It may not add, remove, or weaken validation assertions. Every rewrite above
is assertion-preserving by construction and is unit-tested as such; a rewrite
that cannot be shown equivalent does not belong in the emitter.

Production contract code must not contain manual `JsonSchema`
implementations, `json_schema!`, `schema_with`, handwritten unions, schema
patchers, generators that read previous output, or schema-only
`serde_json::Value` projections.

## Generation guarantees

Every generator supports:

- `--list` for its complete sorted inventory;
- `--write` for the only mutating path;
- `--check` for byte comparison without edits;
- `--output-root` for isolated generation.

Generated JSON uses UTF-8, LF endings, a final newline, recursively sorted
object keys, fixed IDs and examples, and no timestamps, absolute paths,
environment values, or randomness. Write mode removes extra files only inside
validated owned schema roots. Check mode fails on missing, extra, or
byte-different artifacts and never edits the tree.

## Acceptance checks

The contract gate requires:

- one registry entry per checked-in schema and one schema per entry;
- unique `$id` values and resolvable references;
- v1-only schema paths;
- positive examples that deserialize and validate;
- negative tests for unsupported markers, old keys and tags, decimal numbers,
  `null` override maps, and unknown fields;
- zero schema patchers, aliases, migration helpers, retired names, or malformed
  replacement spellings in production and direct consumers;
- identical byte digests from two independent temporary generation trees;
- a clean second write and a passing non-mutating check.

Run:

```text
mise run rust-gen-schemas
mise run rust-check-schemas
mise run rust-serde-audit
mise run gen-check
```

The full release-facing gate is `mise run all-fmt`, `mise run all-lint`,
`mise run all-test`, `mise run rust-doc`, `mise run python-doc`, and
`mise run wasm-doc`.
