# scripts

The JSDoc / TypeScript documentation tooling for this package. Two of these are
checkers; two are generators that write documentation into
[`../index.d.ts`](../index.d.ts); one is the shared JSDoc block parser they build
on. Three can fail `mise run wasm-doc` — both checkers, plus
`complete-facade-jsdoc.mjs` in its `--check` mode. The fourth,
`sync-facade-jsdoc.mjs --check`, fails `mise run wasm-test` (and
`wasm-test-built`), where the `pkg/` build it reads exists.

These exist because `index.d.ts` is hand-maintained. `wasm-bindgen` emits
declarations under `pkg/`, but they describe a flat module rather than the
namespaced facade the package publishes, so the IntelliSense surface every
TypeScript consumer sees is a file no compiler regenerates. That file is large
enough that hand-review does not scale — hence a generator that produces the
mechanical parts and a checker that refuses the boilerplate the generator used to
emit.

The repository-root [`../../scripts/README.md`](../../scripts/README.md) covers
the Python-language gates, including `check_wasm_api_input_docs.py`, which
polices the _Rust_ doc comments under `../src/api/` — the text wasm-bindgen copies
into `pkg/`. That checker and these five are the two halves of `mise run wasm-doc`.

Nothing here is published: `scripts` is absent from the `files` list in
`package.json`.

## Running them

All five are plain Node ESM, run from the package root and dependent only on
`typescript` (a devDependency) and `node:*`:

```bash
npm --prefix finstack-quant-wasm run docs:check   # the gate: all checkers + tooling tests + tsc
npm --prefix finstack-quant-wasm run docs:sync    # the generators, in --write mode
npm --prefix finstack-quant-wasm run docs:check-sync  # sync --check; needs a pkg/ build
mise run wasm-doc                                 # docs:check plus the Rust-side @param checker
```

Prefer the npm script or the `mise` task over invoking a file directly — they pass
the argument set CI uses. `mise run wasm-doc` is a dependency of both
`mise run all-lint` and `mise run all-doc`.

Exit codes are uniform: `0` clean, `1` contract violation, `2` usage error
(passing `--write` and `--check` together).

## Checkers

Run by `docs:check`, in this order.

| Script                      | Enforces                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Options                                                                                                                                           |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check-dts-docs.mjs`        | The WASM-ownership contract as _text_. `index.d.ts` must declare `export interface WasmOwned`, expose `free(): void;`, carry the package-level ownership note and the conditional-`Symbol.dispose` sentence, and must **not** declare `[Symbol.dispose](): void;` (that would force ES2020 consumers onto `esnext.disposable`). Eight named classes must each merge the contract as `export interface <Name> extends WasmOwned {}`. `../README.md` must carry the matching `## WASM Object Disposal` section.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | none — paths are fixed                                                                                                                            |
| `check-typescript-docs.mjs` | Per-declaration JSDoc completeness, parsed with the TypeScript compiler API. Every exported interface, class, type alias, function and variable statement needs a summary of at least 16 characters. Callables need a `@param` per parameter and a `@returns` unless the return type is `void` or the node is a constructor. Exported functions and `*Namespace` / `*Constructor` interfaces additionally need an `@example`. It then rejects fabricated prose: the legacy catch-all `@throws`, non-executable placeholder `@example` blocks, 33 exact generic phrases, two summary regexes (`Perform … for this \``,`Compute … for this \``), the placeholder `@param`/`@returns` shapes the completer used to derive from a name or type (`<Name> used by this call.`, `<Name> as a finite number.`, …), the template Greek units (`per 1.0 absolute move in`, `per year of calendar time`), an internal Rust wrapper name (``[`Js…`]``), and an `@param`name documented twice. Top-level`declare class` handles are checked like exported classes. Members marked`private` are skipped. | `--declaration=<path>` (default `../index.d.ts`), `--max-errors=<n>` (default 200), `--summary` for a count-per-category tally before the listing |

The eight classes hard-coded in `check-dts-docs.mjs` are `Performance`,
`CreditFactorModel`, `CreditCalibrator`, `LevelsAtDate`, `PeriodDecomposition`,
`FactorCovarianceForecast`, `Market`, and `Portfolio`. A new wasm-bindgen class
that owns heap memory is not covered until it is added to that list.

The banned-phrase list in `check-typescript-docs.mjs` mirrors text
`complete-facade-jsdoc.mjs` once emitted. The two files are coupled: changing a
default string in the generator without updating the blocklist (or vice versa)
leaves a phrase either unenforced or unproducible.

## Generators

Run by `docs:sync` with `--write`. Both accept `--check`, which exits 1 instead of
writing when the file would change; `--write` and `--check` together exit 2.

| Script                      | Does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Options                                                                                                                       |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `sync-facade-jsdoc.mjs`     | Pulls contract tags from the wasm-bindgen output into the facade. Reads `pkg/finstack_quant_wasm.d.ts` as authoritative for `@param` / `@returns` / `@throws`, converting rustdoc `# Arguments` / `# Returns` / `# Errors` sections into JSDoc tags and mapping snake_case Rust argument names onto the facade's camelCase parameter names. Facade prose and `@example` blocks are preserved; only contract tags are replaced. The legacy catch-all `@throws` is dropped.                                                                                                                                                                                           | `--facade=<path>` (default `../index.d.ts`), `--raw=<path>` (default `../pkg/finstack_quant_wasm.d.ts`), `--write`, `--check` |
| `complete-facade-jsdoc.mjs` | Fills what the synchronizer cannot: strips legacy boilerplate, inserts a default summary wherever the existing one is missing or under 16 characters, and appends any missing `@param` / `@returns`. It never derives a unit, convention or parameter meaning from a name: a number- or string-returning member with no `@returns` repeats its own summary, and an undocumented parameter is left without an `@param` so the checker reports it. Units and parameter semantics are written in the Rustdoc and arrive through the synchronizer. Type-derived text remains for handles, arrays and structured results, plus per-kind interface/type/member summaries. | `--declaration=<path>` (default `../index.d.ts`), `--write`, `--check`                                                        |

`--check` on `complete-facade-jsdoc.mjs` is part of `docs:check`, which makes the
committed `index.d.ts` a **fixed point** of the generator: if running the
completer would change a single block, the gate fails. Hand-written documentation
survives that check only when its summary is at least 16 characters and its tags
are complete — the generator overwrites nothing it considers adequate.

Name resolution in `sync-facade-jsdoc.mjs` maps facade shapes onto raw classes:
interface `FooConstructor` takes its members from the statics of raw class `Foo`
plus that class's instance constructor; interface `FooNamespace` takes them from
raw free functions; a plain interface `Foo` takes them from `Foo`'s instance
members, and `FooInstrument` (the FX instrument handles) from raw class `Foo`
when no raw class is named `FooInstrument`. Class and top-level function
declarations map directly.

Parameters are matched by name after snake_case to camelCase conversion, then by
the name without its `Json` suffix (Rust `instrument_json` documents the facade's
`instrument`). A facade parameter the Rustdoc does not name keeps its
hand-written `@param`; every parameter the Rustdoc does name is replaced.

`sync-facade-jsdoc.mjs` needs `pkg/finstack_quant_wasm.d.ts` on disk, so it
requires a `wasm-pack --target web` build. That is why it is not in `docs:check`
— `mise run wasm-doc` must run without a build. Its `--check` mode is the
`docs:check-sync` npm script, which `mise run wasm-test` and
`mise run wasm-test-built` run after the facade tests: a Rustdoc edit above a
`#[wasm_bindgen]` item that is not followed by `npm run docs:sync` fails there.

## Shared module

`typescript-docs-shared.mjs` is the only importable module here; the other four
are executables. It is `pub(crate)` in spirit — imported by
`check-typescript-docs.mjs`, `sync-facade-jsdoc.mjs`, and
`complete-facade-jsdoc.mjs`, and by nothing outside this directory.

| Export                              | Purpose                                                                                                                                                                    |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LEGACY_CATCH_ALL_THROW`            | The exact banned `@throws` sentence. Single source of truth for the checker and both generators.                                                                           |
| `documentationBlocks(text)`         | Splits a JSDoc comment into tag-anchored blocks (`prose`, `param`, `returns`, `throws`, `example`). This block model is why tags can be replaced without disturbing prose. |
| `documentationFromBlocks(blocks)`   | The inverse: reassembles a comment, or `null` when empty.                                                                                                                  |
| `isLegacyPlaceholderExample(block)` | Recognizes the three exact shapes of non-executable `@example` the old generator produced.                                                                                 |
| `stripLegacyBoilerplate(text)`      | Removes the catch-all `@throws` and every placeholder example from a comment.                                                                                              |

Whatever pattern this module defines, the checker enforces and the generators
produce — keep the three in step when changing any of them.

## Tests

The tooling is tested like production code, because it edits the published
contract. [`../tests/scripts/typescript_docs.test.mjs`](../tests/scripts/typescript_docs.test.mjs)
covers the synchronizer in `--write` and `--check` (including the `Json`-suffix
parameter mapping, facade-only parameters and `<Name>Instrument` interfaces), the
completer's legacy-removal and no-invented-unit behaviour, the checker's
accept/reject pairs, and the `--write`/`--check` exit-2 contract.

Fixtures live in `../tests/scripts/fixtures/typescript-docs/`: `raw.d.ts` (a stand-in
for wasm-bindgen output), `facade.stale.d.ts` and `facade.expected.d.ts` (the
synchronizer's before/after), `checker.valid.d.ts` and `checker.legacy.d.ts`.
Every mutating case copies its fixture to a temp directory first, so the tests
never touch the repository.

```bash
npm --prefix finstack-quant-wasm run test:docs-tools   # this file only
npm --prefix finstack-quant-wasm run docs:check        # includes it
```

## Adding a checker

- Add it to the `docs:check` chain in `package.json`; it then reaches
  `mise run wasm-doc`, `all-lint`, and `all-doc` with no further wiring.
- Honour the exit-code contract: 0 / 1 / 2 as above. Print failures to stderr,
  one per line, prefixed with `path:line:` where a position exists.
- Support `--declaration=<path>` (or the equivalent) so the tooling tests can
  point it at a fixture instead of the real `index.d.ts`.
- Add a case and a fixture to `../tests/scripts/typescript_docs.test.mjs`, and copy
  the fixture to a temp directory if the script mutates it.
- eslint waives `no-console` for `scripts/**`; prettier covers `.mjs` under
  `mise run wasm-lint`. Fixture `.d.ts` files are excluded from prettier by
  `../.prettierignore`.

## Related

- [`../README.md`](../README.md) — the WASM package overview
- [`../index.d.ts`](../index.d.ts) — the published contract these scripts write
  and police
- [`../tests/README.md`](../tests/README.md) — the four test layers, layer 4 being
  this tooling plus the `tsc` declaration checks
- [`../../scripts/README.md`](../../scripts/README.md) — the Python-language
  gates, including the Rust-side `check_wasm_api_input_docs.py`
- [`../../.agents/rules/wasm/code-standards.md`](../../.agents/rules/wasm/code-standards.md)
