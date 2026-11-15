# STATEMENTS-CONTRACTS-016: Persisted model loading, schema publication and error taxonomy

**Decision status:** Accepted for the full Statements wire admission, schema and error contract (2026-11-15).

**Implementation status:** Reconciliation pending. The audited entry points apply different loading limits and permit hashing before validation.

**Validation status:** Existing source and regression evidence mapped. Independent wire, identity, schema and cross-host qualification remains pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Scope inventory](README.md).

## Accepted decision for Statements wire admission, schemas and errors

1. Use one bounded Rust loading and admission path for Rust, Python and WASM. Require `schema_version` as the exact integer token `1`. Reject duplicate JSON fields, unknown fields in closed objects and invalid numbers. Apply the shared 64 MiB encoded input limit, JSON nesting depth limit of 96 and limit of 256 retained diagnostics, along with the existing period and formula bounds. Complete runnable-model validation before returning the model. [Runnable model validation](01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness) · [Formula bounds](04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation).
2. Use exact tagged `Money` with a decimal-string amount and currency, and exact stochastic seeds as canonical decimal strings on the wire. Compute the canonical `c2` SHA-256 content identity for the validated immutable model. Track evaluation-run identity, input snapshot and applicable cutoff as separate provenance. [Money representation](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity) · [Exact seeds](03-forecast-parameter-contract.md#accepted-decision-for-typed-forecasts-and-validation-staging) · [Canonical content identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity) · [Evaluation provenance](14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).
3. Admit the canonical instrument forms for `Bond`, `ConvertibleBond`, `RevolvingCredit`, `TermLoan`, `InterestRateSwap`, `CapFloor` and `Swaption`. Apply the settled waterfall prepayment and PIK eligibility contracts. [Capital-structure eligibility](../statements-capital/01-instrument-construction.md#accepted-decision-for-opt-in-waterfall-prepayments) · [PIK eligibility](../statements-capital/08-pik-and-carry-state.md#accepted-decision-for-contractual-pik-elections-and-carried-claims).
4. Publish schemas from the canonical Rust types and maintain Python and WASM parity. The financial model fixture must be runnable and contain a value node and a formula node.
5. Distinguish validation, lookup and computation errors across host interfaces. Preserve the structured error code, cause and location.

## Audited input domain

Persisted models carry required SchemaVersion and canonical typed nodes, periods and supported capital-structure instruments. Named loaders combine structural decoding with semantic validation; schema generation and content hashing remain separate operations. The statements error taxonomy deliberately distinguishes missing data/references, validation failures and computation failures.

## Audited algorithm and controls

FinancialModelSpec::from_json and from_value deserialize with unknown-field rejection on the model and nested principal contract types, then run semantic validation. from_slice_strict delegates byte/structure/version limits to the core ContractDescriptor, maps semantic failures to the bounded contract diagnostic, and returns an otherwise empty ValidationReport after success. SchemaVersion is the core versioned wire type; constructors use its CURRENT value. Ordinary derived Serde use alone does not invoke this semantic method.

content_hash delegates to core canonical serialization of the current model. It does not first validate or normalize it. Inferred value_type fields inserted by semantic validation are part of the serialized model, so hash timing relative to validation matters.

Capital-structure wire dispatch accepts exactly Bond, ConvertibleBond, RevolvingCredit, TermLoan, InterestRateSwap, CapFloor and Swaption. Unsupported InstrumentJson variants error. Validation delegates waterfall checks, rejects selected PIK targets that are swaps/options, and rejects any bond/convertible/hedge/option in a structure with a Sweep/MandatoryPrepayment/VoluntaryPrepayment rung. The economics of those rungs remain owned by the capital-structure scope.

With json-schema enabled, the audit counted ten artifacts: check suite, forecast parameters, financial model, normalization config/result, statement result, metric registry, Monte Carlo config/results and rollup specification. The current registry declares 13 artifacts, including bond convention parameters, swap parameters and swap conventions. A thin CLI delegates rendering and checking to the core schema generator. [Current schema registry](../../../../finstack-quant/statements/src/schema.rs).

Error::kind maps NodeNotFound, MissingData and RegistryNotFound to NotFound; CircularDependency and CapitalStructure to Computation; Build, FormulaParse, Eval, CurrencyMismatch, Period, InvalidInput, Forecast, Registry and Serde to Validation. Core errors preserve their kind. Conversion back to core retains currency/cycle detail, maps lookup cases to InputError::NotFound, capital-structure failures to Internal, and other cases to Validation.

## Limitations and alternatives

Schema validity is not timeline, dependency, forecast-domain or accounting validity. A canonical hash proves a specific payload, not that it was validated, evaluated with particular market data or approved. Strict load limits apply only to the strict entry point.

The schema's financial_model_examples helper claims in its source comment to illustrate value/formula nodes, but constructs an empty node map. This is a documentation/example discrepancy, not a financial calculation. The schema registry declaration order is also not its comment's stated path-sorted order; the shared renderer owns publication ordering.

The accepted load and identity contract requires reconciliation across entry points and evaluation provenance.

## Evidence

[Model loaders and instrument contract](../../../../finstack-quant/statements/src/types/model.rs), [schema registry](../../../../finstack-quant/statements/src/schema.rs), [schema generator](../../../../finstack-quant/statements/src/bin/gen_schemas.rs), [error taxonomy](../../../../finstack-quant/statements/src/error.rs), [canonical hash tests](../../../../finstack-quant/statements/tests/canonical_contract.rs) and [schema contract tests](../../../../finstack-quant/statements/tests/schema_contract.rs) provide representation, round-trip and generated-schema evidence. Inline model tests inspect unsupported instrument conversion and waterfall eligibility. These tests were not run; no schema generation was performed.

The period-close FX direction is already agreed; the capital-structure register owns reconciliation of the conflicting flow-date builder language.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
