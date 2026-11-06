# VALUATIONS-FRAMEWORK-20 — Generated builder defaults and validation boundaries

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in both production files of the valuations macros crate; individual financial defaults reside in the instrument declarations.
- Validation status: Source inspected; product builder use located; no tests/builds executed and no dedicated macro test suite located in this crate.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and algorithm

`FinancialBuilder` generates a no-argument builder for named-field structs. It rejects enums/unions and unnamed fields with compile errors. It recognizes an optional field by the final type-path segment named `Option`, and independently treats a field named `attributes` as optional. Every other field is required unless its declaration provides a builder default expression.

Required builder storage is `Option<T>`. `Option<T>` fields retain that type; an `attributes` field of another type also receives `Option<T>` builder storage. Setters store the supplied value; an optional field gets both `field(T)` and `field_opt(Option<T>)`, so the latter can explicitly clear a prior value. The macro derives Default for the builder and does not prefill financial defaults on construction.

At `build`, required fields without defaults fail with a validation error naming the builder and missing field. Required fields with `#[builder(default)]` use `Default::default`; an explicit `#[builder(default = expression)]` uses that expression. These are emitted with **eager `unwrap_or(expression)`**, so the expression evaluates even if a value was explicitly set. Optional `Option<T>` fields carry through unchanged, and optional non-Option `attributes` uses `unwrap_or_default`. Although the parser collects default annotations for all fields, annotations on these optional fields are not applied by the assignment branch. In particular, an optional field remains `None` unless set even if annotated with a nonempty default expression. This is observed generation behavior; no current product-specific mispricing from such an annotation was reproduced here.

After constructing the complete value, call the explicit struct-level `#[builder(validate = expression)]` validator if supplied; otherwise call `Instrument::validate_invariants`. The custom validator **replaces**, rather than supplements, that default call. Propagate its error and return the value only on success. The macro does not invoke the full pricing/market validation lifecycle. Field documentation is flattened into setter argument text; it is not an economic constraint or a generated numerical validator.

## Controls, limitations and alternatives

Selection is compile-time through the derive and builder attributes, rather than a runtime configuration choice. Financial defaults such as recovery, calendar, compounding or model settings are selected by each consuming declaration, so accepting a builder output does not prove its default method has been adopted. The generated code neither infers missing curves nor chooses a model based on present market data.

Lazy default evaluation, explicit rejection/support of default annotations on optional fields, and compile-fail tests for unsupported attribute/type shapes are alternatives. An eager fallible/panicking default expression could affect a builder even when the user supplied the field; this is not a reproduced product defect.

## Evidence

[financial_builder.rs](../../../../finstack-quant/valuations/macros/src/financial_builder.rs) owns parsing, classification, setter generation, default selection and validation ordering. [lib.rs](../../../../finstack-quant/valuations/macros/src/lib.rs) is the procedural-macro entry point. Consumer examples include the explicit convention default in [repo types](../../../../finstack-quant/valuations/rates/src/repo/types.rs) and the custom terms validator in [range-accrual types](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/types.rs). These source uses establish active selection, not independent behavioral test coverage. No external financial standard governs this code-generation contract.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
