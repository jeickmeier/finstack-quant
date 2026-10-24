# Language / ecosystem lenses

Only use sections relevant to the code at hand. These are **prompts to investigate**, not rules to enforce or excuses to replace idiomatic patterns with a personal preference. Read the project's established conventions first.

## Rust

- Is crate/module structure navigable (`lib.rs`, `mod.rs`, exports, feature flags)? Can a maintainer locate public behavior and internal implementations?
- Do trait abstractions, generics, lifetime constraints, macros, and type aliases communicate useful domain ideas, or require following many impls just to understand ordinary paths?
- Are public error types, conversions, ownership boundaries, invariants and any `unsafe` assumptions visible where they matter?
- Is it clear which modules define domain models, algorithms, adapters, and bindings (such as PyO3 and WASM)? Are core semantics separated from language-specific plumbing without excessive interfaces?
- Are tests placed where behavior and important invariants can be discovered?

## Python

- Does the module structure distinguish domain logic, I/O, orchestration and interface adapters?
- Are mutation and side effects apparent despite decorators, context managers, dynamic dispatch or global configuration?
- Do useful type hints and error contracts disambiguate behavior without creating boilerplate or faux static type systems?
- Are data models, validation decisions, and transformations owned consistently?

## TypeScript / JavaScript / React

- Can readers trace state ownership and changes across hooks, components and context providers?
- Are component boundaries cohesive, avoiding giant god-components *and* many one-use micro-components?
- Do types/interfaces make contracts clearer, or hide key semantics in complex generic utilities?
- Are API/client/server boundaries, asynchronous side effects and error propagation discoverable?

## SQL / database / migrations

- Are canonical tables and business definitions discoverable, with stable naming and clear migration intent?
- Are joins, transformations, exception handling, transaction boundaries, and database-side rules documented when non-obvious?
- Can a developer identify whether a rule belongs in the database, domain layer, or reporting/query layer?

## Workflow orchestration (e.g., Airflow)

- Can a reader distinguish schedule/dependency orchestration from reusable business logic?
- Are retries, idempotency assumptions, side effects, and inter-task contracts visible?
- Are DAG/task boundaries and naming comprehensible to someone tracing a failed run?

## Cross-language systems

- Identify the authoritative owner of shared domain semantics and schemas.
- Check that boundaries (FFI, HTTP, events, DB, generated clients) have readable contracts, error semantics, and consistent naming.
- Avoid automatically proposing a shared abstraction if simpler explicit adapters make the system easier to understand.
