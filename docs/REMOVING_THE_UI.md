# Removing the UI

`finstack-quant-ui` is the shadcn component registry. It renders
`finstack-quant-wasm` results and holds no financial logic. The Cargo
workspace has no dependency on it. Rust, Python, and WASM keep building
after the package is gone, once the seams below are cut.

Do the steps in order. `gen-check` and two valuations tests read files
inside the UI tree, so deleting the directory first fails those gates.

The analyst curriculum in `docs-site/` stays. Its checks under
`docs-site/gen/` and `docs-site/tests/` do not call the registry. The gallery, the registry guide pages, and the
`ui-sync` and `ui-docs-install` steps inside `docs-site-install` do.

## Move the pricing catalogue

Two integration tests load
`finstack-quant-ui/tests/valuations/instruments/pricing-cases.json`:

- `finstack-quant/valuations/tests/instruments/loan_facility_wire_keys.rs`
  (`pricing_cases`, used by
  `migrated_inputs_reprice_to_recorded_reference`)
- `finstack-quant/valuations/tests/instruments/coupon_strike_inflation_wire_keys.rs`
  (`pricing_cases`, used by
  `migrated_examples_reprice_to_recorded_reference` and
  `rescaled_inputs_reprice_to_recorded_reference`)

Copy that JSON to
`finstack-quant/valuations/tests/instruments/pricing-cases.json` and point
both `workspace_root().join(...)` calls at the new path. Leave the UI copy
in place until the package directory is deleted.

## Unhook generation

In `mise.toml`:

- Remove `npm --prefix finstack-quant-ui run generate` from `[tasks.gen-write]`.
- Remove `npm --prefix finstack-quant-ui run generate -- --check` from
  `[tasks.gen-check]`.

In `scripts/generation_digest.py`, remove every `finstack-quant-ui/...` path
and `docs-site/content/docs/registry/instruments.mdx` from `DEFAULT_PATHS`
and `DEFAULT_MANIFEST_PATHS`. That MDX file is written by
`finstack-quant-ui/scripts/catalogue-docs.mjs`.

Delete
`test_component_registry_generated_contracts_participate_in_drift_checks`
in `scripts/tests/test_generation_digest.py`. It asserts those UI paths
stay in the digest.

Rewrite the inventory after the path lists change and before the UI tree
is deleted:

```bash
uv run --no-sync python scripts/generation_digest.py --write-manifest
```

`scripts/generated-artifacts.txt` then drops the registry files. The
manifest paragraph in `scripts/README.md` describes the instrument JSON
examples and the WASM TypeScript declarations, so it stays accurate.

In `.github/workflows/build.yml`, delete the lint job step
`Install component registry dependencies` (`mise run ui-sync`).

## Keep the analyst reader, drop the gallery

In `docs-site/package.json`, remove the `finstack-quant-ui` file
dependency, then refresh the lockfile without lifecycle scripts:

```bash
npm --prefix docs-site install --ignore-scripts --package-lock-only
```

In `mise.toml` `[tasks.docs-site-install]`, remove `"ui-sync"` from the
parallel group and the `{ task = "ui-docs-install" }` step.

Delete:

- `docs-site/content/docs/registry/`
- `docs-site/src/app/(gallery)/`
- `docs-site/src/components/registry-gallery/`
- `docs-site/src/components/registry-demo.tsx`

Then edit:

- `docs-site/src/lib/navigation.tsx`: remove the Component registry folder.
- `docs-site/src/app/(reader)/layout.tsx`: remove the `REGISTRY_ONLY`
  branch that points the nav at `/docs/registry/workbench`.
- `docs-site/src/app/(reader)/docs/[[...slug]]/page.tsx`: remove the
  `registry/workbench` full-width case.
- `docs-site/README.md`: remove the `mise run ui-sync` prerequisite and
  the "Component registry development" section.

## Remove tasks and workflows

Delete the `[tasks.ui-*]` block in `mise.toml`, from `ui-sync` through
`ui-publish-check`, including the section header that introduces it.

Delete:

- `.github/workflows/ui-gallery.yml`
- `.github/workflows/registry-publish.yml`

In `.github/workflows/docs.yml`, remove the `registry_publish` and
`registry_only` workflow inputs, the `registry-gallery` and
`registry-publish` jobs, and the `if:` guards on `rust-docs` and
`host-docs` that skip those jobs when a registry input is set.

Update the workflow table in `.github/workflows/README.md`.

## Delete the package

```bash
rm -rf finstack-quant-ui
```

Update the remaining mentions:

- `README.md`: the tree entry for `finstack-quant-ui/` and the
  "UI components and docs site" section. Keep the `docs-site/` description
  for the analyst reader.
- `AGENTS.md`: the two bullets that tell agents to regenerate UI
  valuation-detail and cashflow fixtures after WASM source changes.

Search the tree for `finstack-quant-ui` and `ui-docs-install`. The OSV
scan in `mise.toml` lists `Cargo.lock`, `uv.lock`, and
`finstack-quant-wasm/package-lock.json`. Pre-commit does not call the UI
tasks.

## Check

```bash
mise run rust-test-filter -- finstack-quant-valuations migrated_inputs_reprice_to_recorded_reference
mise run rust-test-filter -- finstack-quant-valuations migrated_examples_reprice_to_recorded_reference
mise run rust-test-filter -- finstack-quant-valuations rescaled_inputs_reprice_to_recorded_reference
uv run --no-sync pytest scripts/tests/test_generation_digest.py -q
mise run gen-check
```

`gen-check` is the lint-job gate that used to install and regenerate the
registry. `docs-site/tests` covers the analyst reader.
