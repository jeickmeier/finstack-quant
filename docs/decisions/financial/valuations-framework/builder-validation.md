# VALUATIONS-FRAMEWORK-20 — Generated builder defaults and validation boundaries

- Decision status: **Accepted — complete checked generated financial builders package, 2026-11-14**.
- Implementation status: Macro and caller reconciliation pending. Observed behavior below describes the existing derive and consumer paths.
- Validation status: Source, call sites and isolated compile and execution probes reviewed. Product, full Rust and host qualification pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked generated financial builders

1. **Keep a small derive contract.** Support nongeneric named-field structs
   and the existing no-argument `T::builder()` with consuming named setters
   and fallible consuming `build()`. Defer completion to `build()`. Fluent
   setters assign values and permit replacement in any order. The public
   construction entry is `T::builder()`. Keep setter argument documentation
   tied to field units, meaning and omission behavior.

2. **Classify fields by declared type and annotation.** Recognize a direct
   standard `Option<T>` written as `Option<T>`, `core::option::Option<T>`
   or `std::option::Option<T>`, including fully qualified forms, with one
   type argument. Verify the declared option against the standard type.
   Optional storage starts at `None`. `field(value)` sets `Some(value)`
   and `field_opt(None)` clears the field. Type aliases get ordinary
   field handling and confer no implicit optionality. Reject an unrelated
   qualified type named `Option`. Fields outside the optional forms
   require a value unless explicitly defaulted. Remove the `attributes`
   name exception. Where the product accepts omitting metadata, declare
   the empty `Attributes` value explicitly in the owning contract.

3. **Reject unsupported or ineffective declarations.** Accept one
   field-level `#[builder(default)]` or `#[builder(default = expression)]`
   on a non-option field, and one struct-level `#[builder(validate = path)]`
   where the selected standalone terms validator applies. Reject defaults
   on `Option<T>`, the obsolete `optional` marker, unknown, misplaced,
   duplicate or malformed keys, unsupported struct shapes and generated
   setter name collisions such as `build` or a field colliding with
   another field's `_opt` setter. Report the offending declaration at
   compile time. Use Rust type checking for expressions and validator
   signatures.

4. **Resolve defaults only on omission.** First report the first missing
   required field without a default in declaration order, with the builder
   and field in the existing validation error. With those fields present,
   resolve omitted defaults once in declaration order. A bare annotation
   uses the type's `Default` and an explicit expression supplies the
   declared type. A fallible expression may propagate its ordinary
   construction error through the omitted branch. Supplied values skip the
   default even when zero, false or empty, and still undergo structural
   validation. Defaults need a documented owner-approved omitted meaning,
   such as empty metadata or absence of overrides where admitted. Require
   financial terms explicitly or obtain them from a deliberately selected,
   qualified product profile before handing its resolved terms to the builder.
   Keep market access and economic inference out of generated default
   resolution. See [Rust Option evaluation](https://doc.rust-lang.org/std/option/enum.Option.html#method.unwrap_or),
   [convention selection](convention-selection.md) and
   [shared contract controls](shared-contract-controls.md).

5. **Validate the complete value through one structural route.** For an
   instrument, call `Instrument::validate_invariants` once after assembly.
   The owning implementation composes the shared, nested and product-specific
   checks it needs. Reserve `validate = path` for standalone terms that
   lack an Instrument implementation. Require a callable of
   `fn(&T) -> finstack_quant_core::Result<()>` shape and check declaration
   usage across the deriving crates. Propagate errors and publish only
   after the selected validator succeeds. Keep validation based on declared
   terms. Do not add field-name heuristics. Pricing separately checks the
   actual request, date, selected model, market inputs and overrides under
   its lifecycle. A public pricing-capable ingress still applies its
   required admission checks. See [valuation lifecycle](pricing-lifecycle.md).

6. **Align construction, restore and host surfaces.** Existing Rust, Python
   and WASM constructors and published restore paths apply the same omission,
   required-field and structural rules for the same payload. Required values
   must be supplied. An admitted optional absence carries `None`, and
   explicit null clears a nullable field where the wire contract admits it.
   When a product requires a deliberate election, its required term
   expresses that choice, including an explicit no-selection case where
   supported. A nonnullable default applies only to omission. Use the
   canonical Rust owner for checked state and the existing host error
   mapping. Keep optionality and published default metadata aligned across Rust declarations,
   schema, Python stubs, WASM declarations and examples. Update the existing
   macro documentation and verify real consumers across the split product
   crates. See [current macro documentation](../../../../finstack-quant/valuations/macros/README.md)
   and [current consistency check](../../../../finstack-quant/valuations/tests/default_attribute_consistency.rs).

## Observed behavior

## Choice, scope and algorithm

`FinancialBuilder` generates a no-argument builder for named-field structs. It rejects enums/unions and unnamed fields with compile errors. It recognizes an optional field by the final type-path segment named `Option`, and independently treats a field named `attributes` as optional. Every other field is required unless its declaration provides a builder default expression.

Required builder storage is `Option<T>`. `Option<T>` fields retain that type; an `attributes` field of another type also receives `Option<T>` builder storage. Setters store the supplied value; an optional field gets both `field(T)` and `field_opt(Option<T>)`, so the latter can explicitly clear a prior value. The macro derives Default for the builder and does not prefill financial defaults on construction.

At `build`, required fields without defaults fail with a validation error naming the builder and missing field. Required fields with `#[builder(default)]` use `Default::default`; an explicit `#[builder(default = expression)]` uses that expression. These are emitted with **eager `unwrap_or(expression)`**, so the expression evaluates even if a value was explicitly set. Optional `Option<T>` fields carry through unchanged, and optional non-Option `attributes` uses `unwrap_or_default`. Although the parser collects default annotations for all fields, annotations on these optional fields are not applied by the assignment branch. In particular, an optional field remains `None` unless set even if annotated with a nonempty default expression. This is observed generation behavior; no current product-specific mispricing from such an annotation was reproduced here.

After constructing the complete value, call the explicit struct-level `#[builder(validate = expression)]` validator if supplied; otherwise call `Instrument::validate_invariants`. The custom validator **replaces**, rather than supplements, that default call. Propagate its error and return the value only on success. The macro does not invoke the full pricing/market validation lifecycle. Field documentation is flattened into setter argument text; it is not an economic constraint or a generated numerical validator.

## Controls, limitations and alternatives

Selection is compile-time through the derive and builder attributes, rather than a runtime configuration choice. Financial defaults such as recovery, calendar, compounding or model settings are selected by each consuming declaration, so accepting a builder output does not prove its default method has been adopted. The generated code neither infers missing curves nor chooses a model based on present market data.

The accepted decision requires lazy defaults, rejection of defaults on optional fields, and compile-time diagnostics for unsupported declarations. The isolated probes below reproduce the derive behavior. Product-level qualification remains pending.

## Evidence

[financial_builder.rs](../../../../finstack-quant/valuations/macros/src/financial_builder.rs) owns parsing, classification, setter generation, default selection and validation ordering. [lib.rs](../../../../finstack-quant/valuations/macros/src/lib.rs) is the procedural-macro entry point. Consumer examples include the explicit convention default in [repo types](../../../../finstack-quant/valuations/rates/src/repo/types.rs) and the custom terms validator in [range-accrual types](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs). These source uses establish active selection, not independent behavioral test coverage. No external financial standard governs this code-generation contract.

## Independent review evidence

- A temporary build of the current two-file proc macro was linked into
  isolated Rust consumers. A failing default ran with an explicit field
  value and masked a later missing required field. Probes also reproduced
  an ignored unknown struct key, ignored optional and `attributes`
  defaults, last duplicate winning, a custom validator replacing an
  instrument invariant, and `_opt(None)` clearing a previously supplied value.
- Five compile-fail consumers exercised a tuple struct, unknown field key,
  a generated setter collision, a generic struct and a foreign `Option`
  type. The last three failed in generated Rust. The accepted decision requires
  direct declaration diagnostics for unsupported shapes and name collisions.
- Separate Rust control-flow checks exercised the proposed ordering.
  An explicit value bypasses the fallback. A missing required field
  returns before evaluating a default. An omitted approved default evaluates
  once and can propagate its original error. An assembled invalid value
  returns the structural error.
- The two active custom hooks belong to nested range-accrual and futures-option
  terms. The containing instruments have their own invariant hooks.
  A rate futures option can require the DV01 transformation at its wrapper
  boundary even when its standalone option terms validate.
  [Range-accrual types](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs),
  [futures-option terms](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs),
  [futures-option wrapper validation](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs).
- Repo and CapFloor have non-optional `attributes` fields without a wire
  default on the field. Their generated builders currently supply empty
  metadata by name. These declarations give a concrete omission case for
  construction and restore parity. [Repo](../../../../finstack-quant/valuations/rates/src/repo/types.rs),
  [CapFloor](../../../../finstack-quant/valuations/rates/src/cap_floor/types.rs).

Workspace, real consumer and host tests remain pending. Qualification needs
targeted macro acceptance and rejection fixtures, validated omitted, supplied
and cleared values on actual ingress paths, the two standalone validators,
and schema and host parity across the instrument declarations.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
