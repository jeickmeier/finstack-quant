# scripts

Build-time tooling for the package's TypeScript surface: one generator and two
documentation checkers, plus the contract-type generator and the web build
helpers. Nothing here is published: `scripts` is absent from the `files` list in
`package.json`.

## Where the TypeScript comes from

The Rust bindings are the one source of the published TypeScript:

- **Types and names.** `#[wasm_bindgen]` attributes in `../src/api/**` give each
  `JsValue` parameter and return its TypeScript type
  (`unchecked_param_type`, `unchecked_optional_param_type`,
  `unchecked_return_type`) and each snake_case parameter its camelCase
  `js_name`. wasm-bindgen writes them into `../pkg/finstack_quant_wasm.d.ts`.
- **Documentation.** The Rustdoc above each `#[wasm_bindgen]` item becomes its
  JSDoc in the same file. Doc comments a `macro_rules!` invocation forwards go
  through `#[finstack_quant_wasm_macros::plain_docs]` (`../macros/`), which
  turns rustc's raw-string doc literals into plain ones wasm-bindgen copies
  verbatim.
- **Host-only types.** `../src/prelude.d.ts` (the schema-generated type imports
  and shapes such as `JsonInput` and `FinstackError`) is appended to the same
  file by the `typescript_custom_section` in `../src/lib.rs`.
- **The facade.** `gen-facade-dts.mjs` writes `../index.d.ts` from `../index.js`
  and `../exports/**`: one `*Namespace` interface per namespace, each member
  `typeof wasm.X`, plus the declarations of the JavaScript the facade adds.

## Generator

| Script               | Does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | Run by                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `gen-facade-dts.mjs` | Writes `../index.d.ts` over the built `../pkg/finstack_quant_wasm.d.ts`. A facade wrapper is declared from its JSDoc ending in `@facadeType <function type>`; a method the facade adds to (or widens on) a class prototype from one JSDoc block ending in `@facadeClass Name` and `@member name(params): Return`, emitted as an augmentation of the package module; `chain(Class, [...])` methods get `this`-returning overloads with the package's parameters and docs. It fails on a wrapper or added member without documentation, and on `@omit`/`@static` (mark a replaced raw method `skip_typescript` in Rust instead). `--check` fails when the committed file differs. | `mise run gen-write` (write); `mise run wasm-test` and `wasm-test-built` (`--check`) |

## Checkers

Both run in `mise run wasm-doc` (nightly, `mise run all --full`) through
`npm run docs:check`, which also runs the tooling tests and `npm run test:dts`.

| Script                          | Enforces                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Options                                                                                                                      |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `check-typescript-docs.mjs`     | JSDoc completeness of the published declarations (`../pkg/finstack_quant_wasm.d.ts` and `../index.d.ts`), parsed with the TypeScript compiler API. Every declaration needs a summary of at least 16 characters; callables need one `@param` per parameter and a `@returns` unless they return `void`; classes (on the class or its constructor) and namespace interfaces need an `@example`. It rejects fabricated prose: the legacy catch-all throws tag, placeholder examples, generic phrases, template units, Rust intra-doc links and internal wrapper names, and a parameter documented twice. wasm-bindgen's glue (`InitOutput`, `initSync`, `free`, `[Symbol.dispose]`) is skipped. | `--declaration=<path>` (one file instead of the two), `--max-errors=<n>` (default 200), `--summary` for a per-category tally |
| `check-typescript-examples.mjs` | Every documented code sample type-checks: each fenced `ts`/`js` block in a declaration doc comment and in `../README.md`, `../../README.md` and `.agents/rules/wasm/javascript-usage-standards.md`. They compile as one `strict` TypeScript program inside this package, so `finstack-quant-wasm` resolves to `../index.d.ts` as it does for a consumer. A sample the facade copies from the package declarations is checked once. `../tests/docs/doc_examples.test.mjs` executes the same samples under `mise run wasm-test --doctest`.                                                                                                                                                    | none                                                                                                                         |

A sample must be a complete program. A block with no `import` statement
continues from the standard setup (every namespace imported and `await init()`
done), which both gates put in front of it. A fence tagged `no-run`
(` ```javascript no-run `) is skipped by both; use it only to show what not to
write. `doc-examples.mjs` collects the samples for both gates.

Both need a build (`mise run wasm-build`), since they read the generated
declarations. Exit codes: `0` clean, `1` contract violation.

`check-exports-boundary.mjs` is not a docs check: `npm run lint` runs it. It fails
on arithmetic in `exports/**` and on a function with no comment saying what
wasm-bindgen cannot express.

## Other scripts

- `generate-contract-types.mjs` writes `../types/generated/` from the Rust JSON
  Schemas (`mise run gen-write`).
- `prepare-schema.mjs` lowers draft-2020-12 tuples for that generator.
- `build-web.mjs` and `measure-web.mjs` build the web package and report its
  artifact sizes; `order.mjs` is the shared locale-independent sort.

## Tests

[`../tests/scripts/typescript_docs.test.mjs`](../tests/scripts/typescript_docs.test.mjs)
covers the doc checker's accept/reject pairs (fixtures in
`../tests/scripts/fixtures/typescript-docs/`), its class-example rule and its
glue exemptions; `exports_boundary.test.mjs` covers the boundary lint.

```bash
npm --prefix finstack-quant-wasm run test:docs-tools
```

## Related

- [`../README.md`](../README.md) — the WASM package overview
- [`../tests/README.md`](../tests/README.md) — the test layers, including the
  declaration tests
- [`../../scripts/README.md`](../../scripts/README.md) — the gate map: every
  checker and the stage that runs it
- [`../../.agents/rules/wasm/code-standards.md`](../../.agents/rules/wasm/code-standards.md)
