# STATEMENTS-CONTRACTS-016: Persisted model loading, schema publication and error taxonomy

**Decision status:** Draft (observed implementation; adoption pending).

**Implementation status:** Observed for the source responsibilities stated below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice README](README.md)).

## Choice and input domain

Persisted models carry required SchemaVersion and canonical typed nodes, periods and supported capital-structure instruments. Named loaders combine structural decoding with semantic validation; schema generation and content hashing remain separate operations. The statements error taxonomy deliberately distinguishes missing data/references, validation failures and computation failures.

## Algorithm and controls

FinancialModelSpec::from_json and from_value deserialize with unknown-field rejection on the model and nested principal contract types, then run semantic validation. from_slice_strict delegates byte/structure/version limits to the core ContractDescriptor, maps semantic failures to the bounded contract diagnostic, and returns an otherwise empty ValidationReport after success. SchemaVersion is the core versioned wire type; constructors use its CURRENT value. Ordinary derived Serde use alone does not invoke this semantic method.

content_hash delegates to core canonical serialization of the current model. It does not first validate or normalize it. Inferred value_type fields inserted by semantic validation are part of the serialized model, so hash timing relative to validation matters.

Capital-structure wire dispatch accepts exactly Bond, ConvertibleBond, RevolvingCredit, TermLoan, InterestRateSwap, CapFloor and Swaption. Unsupported InstrumentJson variants error. Validation delegates waterfall checks, rejects selected PIK targets that are swaps/options, and rejects any bond/convertible/hedge/option in a structure with a Sweep/MandatoryPrepayment/VoluntaryPrepayment rung. The economics of those rungs remain owned by the capital-structure scope.

With json-schema enabled, the crate registers ten artifacts: check suite, forecast parameters, financial model, normalization config/result, statement result, metric registry, Monte Carlo config/results and rollup specification. A thin CLI delegates rendering/checking to the core schema generator. Typed examples validate representation rather than execute financial algorithms.

Error::kind maps NodeNotFound, MissingData and RegistryNotFound to NotFound; CircularDependency and CapitalStructure to Computation; Build, FormulaParse, Eval, CurrencyMismatch, Period, InvalidInput, Forecast, Registry and Serde to Validation. Core errors preserve their kind. Conversion back to core retains currency/cycle detail, maps lookup cases to InputError::NotFound, capital-structure failures to Internal, and other cases to Validation.

## Limitations and alternatives

Schema validity is not timeline, dependency, forecast-domain or accounting validity. A canonical hash proves a specific payload, not that it was validated, evaluated with particular market data or approved. Strict load limits apply only to the strict entry point.

The schema's financial_model_examples helper claims in its source comment to illustrate value/formula nodes, but constructs an empty node map. This is a documentation/example discrepancy, not a financial calculation. The schema registry declaration order is also not its comment's stated path-sorted order; the shared renderer owns publication ordering.

A persisted validation status or evaluation provenance envelope could make replay guarantees explicit, while rejecting unchecked raw deserialization at application boundaries would be a simpler integration rule. Neither is implied by a schema or content hash.

## Evidence

[Model loaders and instrument contract](../../../../finstack-quant/statements/src/types/model.rs), [schema registry](../../../../finstack-quant/statements/src/schema.rs), [schema generator](../../../../finstack-quant/statements/src/bin/gen_schemas.rs), [error taxonomy](../../../../finstack-quant/statements/src/error.rs), [canonical hash tests](../../../../finstack-quant/statements/tests/canonical_contract.rs) and [schema contract tests](../../../../finstack-quant/statements/tests/schema_contract.rs) provide representation, round-trip and generated-schema evidence. Inline model tests inspect unsupported instrument conversion and waterfall eligibility. These tests were not run; no schema generation was performed.

The period-close FX direction is already agreed; the capital-structure register owns reconciliation of the conflicting flow-date builder language.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-model-formula-and-check-contracts).
