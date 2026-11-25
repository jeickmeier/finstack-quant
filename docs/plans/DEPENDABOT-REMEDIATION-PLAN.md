# Dependabot remediation plan

GitHub reported **95 open alerts across 63 advisories** on November 24, 2026. The docs lockfile accounts for 54 alerts and the UI lockfile accounts for 41. The severity breakdown is 3 critical, 29 high, 52 medium, and 11 low. All 95 are npm alerts. The reviewed default branch was `master` at `7006c3f65145b75b4b51a05cf977d1010482b151`. The only open Dependabot PR in the snapshot was an unrelated GitHub Actions update. [Dependabot alerts](https://github.com/jeickmeier/finstack-quant/security/dependabot) · [Reviewed commit](https://github.com/jeickmeier/finstack-quant/commit/7006c3f65145b75b4b51a05cf977d1010482b151) · [PR #157](https://github.com/jeickmeier/finstack-quant/pull/157)

Two temporary lockfiles resolved all 95 advertised vulnerable ranges. **Thirty alerts are covered by removing the affected dependencies and 65 by upgrading them.** Both scratch projects installed with Node 24.14.0 and `npm ci --ignore-scripts`, and both npm audits reported zero findings. The candidate shadcn CLI built 364 registry JSON files and the candidate docs Markdown pipeline rendered inline and display equations. An OSV scan across all five tracked lockfiles reported no issues after the existing Chrono filter and flagged the WASM Picomatch exception as unused. Full UI and docs gates and GitHub alert closure remain to be verified after implementation.

## 1. Patch Next.js first

- Change the exact `next` pin in [the docs manifest](../../docs-site/package.json) from `16.3.4` to the verified `16.4.6`. The highest minimum fixed version in the snapshot is `16.4.5`. Next `16.4.6` also requires optional `sharp ^0.35.5`, which resolves the Sharp alert. Confirm both changes in the docs lockfile and let npm select the matching platform packages. [Next.js package metadata](https://registry.npmjs.org/next/16.4.6)
- Prioritize this slice because it covers all three critical alerts, [#1](https://github.com/jeickmeier/finstack-quant/security/dependabot/1), [#18](https://github.com/jeickmeier/finstack-quant/security/dependabot/18), and [#21](https://github.com/jeickmeier/finstack-quant/security/dependabot/21). The checked-in [Next.js configuration](../../docs-site/next.config.mjs) produces a static export with image optimization disabled. Verify any separately hosted Next.js server against the rollout inventory.
- Run the docs typecheck after setup and verify a static export. Smoke-test a lesson, its static assets, the search index, and the registry gallery. If the docs are published, rebuild and roll out the patched static artifact through the existing publication process.

## 2. Upgrade the shared registry and docs rendering dependencies

- Pin `shadcn 4.23.0` and `zod 4.6.6` in both npm manifests. Shadcn 4.23.0 uses `@modelcontextprotocol/server` in place of the v1 SDK. npm removed the old SDK, `hono`, `@hono/node-server`, `express`, `body-parser`, and `qs` from both candidate graphs. Check that those packages remain absent or resolve outside the advisory ranges if another dependency adds them. [Shadcn package metadata](https://registry.npmjs.org/shadcn/4.23.0)
- Use the scoped npm override `"@ts-morph/common": { "minimatch": "11.0.9" }` in both manifests. Shadcn 4.23.0 still resolves through `ts-morph 26.0.0` and `@ts-morph/common 0.27.0`, which declares `minimatch ^10.0.1`. All 10.x releases in that range are affected. Exercise a fresh registry install as well as the build because the override crosses the parent's declared major version. npm root overrides apply to these two projects. Independently installed copies of the shadcn CLI will need an upstream parent update to inherit the Minimatch fix. [Minimatch advisory](https://github.com/advisories/GHSA-g3hv-75wg-m249) · [Parent package metadata](https://registry.npmjs.org/@ts-morph%2fcommon/0.27.0) · [npm override scope](https://docs.npmjs.com/cli/v11/configuring-npm/package-json#overrides)
- Add `"katex": "$katex"` to the docs manifest's npm overrides. This reuses the existing direct pin `0.18.5` for both nested branches, `rehype-katex` and `remark-math → micromark-extension-math`. Run a lockfile-only install and check that both `0.16.47` copies disappear. [KaTeX advisory](https://github.com/advisories/GHSA-238p-pmpm-9mq7)
- Refresh the remaining transitive packages through npm within the ranges already declared by their parents. The scratch resolutions were `fast-uri 3.1.9`, `ip-address 10.7.6`, `source-map-js 1.2.4`, `systeminformation 5.33.26`, and `undici 7.30.2`. PostCSS and Tailwind pull in source-map-js, AJV pulls in fast-uri, and the shadcn tooling pulls in systeminformation and multiple copies of Undici.
- Propagate the Zod pins through [the core registry](../../finstack-quant-ui/registry/core/registry.json) and [registry content generator](../../finstack-quant-ui/scripts/registry-content.mjs). Propagate the shadcn pin through [the theme generator](../../finstack-quant-ui/scripts/gen-theme.mjs), [public registry checks](../../finstack-quant-ui/scripts/verify-public.mjs), and the docs commands for [distribution](../../docs-site/content/docs/registry/distribution.mdx) and [upgrades](../../docs-site/content/docs/registry/upgrading.mdx). Run `mise run gen-write` for generated registry output. Verify the docs install and a fresh registry consumer pick up the new direct pins.

## 3. Patch the UI build and test dependencies

- Pin `vite 8.3.8` and `vitest 5.0.3` in [the UI manifest](../../finstack-quant-ui/package.json). Vitest 5.0.3 pins `@vitest/mocker 5.0.3`, so the parent upgrade fixes both mocker alerts. [Vite metadata](https://registry.npmjs.org/vite/8.3.8) · [Vitest metadata](https://registry.npmjs.org/vitest/5.0.3)
- Update Undici after the shadcn change and inspect all copies. The UI uses a separate Undici 8 through `@apidevtools/json-schema-ref-parser 16.0.2`. The tested graph selected `8.13.1` for that parent and `7.30.2` for shadcn, dotenvx, and jsdom paths. The highest fixed floors in the snapshot are `7.30.1` and `8.12.2`. A combined npm update initially retained the old 8.x copy. A second targeted `npm update undici --package-lock-only --ignore-scripts` refreshed it.
- Resolve the UI manifest before the docs lockfile because docs links to `file:../finstack-quant-ui`. Use `npm install --package-lock-only --ignore-scripts` from each package directory to write manifest changes, then targeted `npm update <packages> --package-lock-only --ignore-scripts` for in-range transitive changes. Check the resulting trees with `npm ls --package-lock-only --all <packages>`, supplementing it with an inspection of all matching `packages` entries in the lockfiles.

## Alert inventory

The candidate versions below were observed in both temporary lockfiles where applicable. Alert numbers refer to [the GitHub Dependabot inventory](https://github.com/jeickmeier/finstack-quant/security/dependabot).

| Package | Docs alerts | UI alerts | Highest fixed floor | Verified lockfile resolution |
| --- | --- | --- | --- | --- |
| `next` | 1, 6–11, 17–19, 21, 109, 111, 115–118 | | 16.4.5 | 16.4.6 |
| `sharp` | 4 | | 0.35.5 | 0.35.5 |
| `@modelcontextprotocol/sdk` | 5, 13–16, 32 | 44, 46, 47, 49 | 1.32.0 | Removed through shadcn |
| `hono` | 12, 94, 96, 98, 100, 102 | 45, 93, 95, 97, 99, 101 | 4.13.16 | Removed through shadcn |
| `@hono/node-server` | 39, 90, 92 | 84, 89 | 2.1.6 | Removed through shadcn |
| `body-parser` | 23 | | 2.4.0 | Removed through shadcn |
| `qs` | 31 | 71 | 6.16.1 | Removed through shadcn |
| `zod` | 22 | 59 | 4.6.6 | 4.6.6 |
| `minimatch` | 42 | 88 | 11.0.1 | 11.0.9 |
| `katex` | 3 | | 0.18.2 | 0.18.5 |
| `fast-uri` | 24–27 | 61–64 | 3.1.9 | 3.1.9 |
| `ip-address` | 20 | | 10.7.4 | 10.7.6 |
| `source-map-js` | 2 | 43 | 1.2.2 | 1.2.4 |
| `systeminformation` | 40, 41 | 85–87 | 5.33.16 | 5.33.26 |
| `undici` | 28–30, 33–37 | 69, 70, 75–82 | 7.30.1 and 8.12.2 | 7.30.2 in docs and UI, 8.13.1 in UI |
| `vite` | | 48, 50, 51, 53–55 | 8.3.3 | 8.3.8 |
| `@vitest/mocker` | | 56, 57 | 5.0.3 | 5.0.3 |
| **Total** | **54** | **41** | | **95 covered** |

## Verification and ongoing coverage

1. Before implementation, refresh the open alert inventory with the command below. Compare each alert's vulnerable range against every installed copy in its reported manifest, including optional and development dependencies.

   ```sh
   gh api --paginate 'repos/jeickmeier/finstack-quant/dependabot/alerts?state=open&per_page=100'
   ```

2. Run clean Node 24 installs and a full npm audit in both projects. Verify `mise run ui-check` with the development web and Node WASM packages. Build release WASM packages with `mise run wasm-pkg` for `mise run ui-install`, which builds the registry and checks fresh consumers. Run `mise run ui-docs-build` before `mise run ui-e2e` so Playwright can serve the gallery export. Exercise the documented `shadcn view` and MCP client configuration commands against the updated CLI.
3. Run `mise run docs-site-install`, `npm --prefix docs-site run typecheck`, and `mise run docs-site-build`. Check a lesson with inline and display math, the docs search, and the registry gallery in the static export.
4. Extend [`mise run all-audit`](../../mise.toml) to scan the docs and UI lockfiles alongside Cargo, uv, and WASM. The existing GitHub supply-chain job invokes this task. Remove the obsolete WASM OSV exception for the withdrawn Picomatch advisory after confirming the extended scan. Keep Arrow's separate Chrono maintenance advisory under its existing review process. Add weekly npm version updates for the two projects to [Dependabot configuration](../../.github/dependabot.yml), which currently schedules GitHub Actions updates only. [Withdrawn Picomatch advisory](https://github.com/advisories/GHSA-fc5q-39xc-vjpf) · [Chrono maintenance advisory](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)
5. Run `mise run slice-gate` before any slice commit and `mise run all` once the batch is complete. After merge, refresh Dependabot on `master` and confirm all 95 baseline alerts are marked fixed. Check any alerts opened since the inventory was taken. If an alert remains open, verify the affected path in the default-branch dependency graph and trace any remaining vulnerable copy. [GitHub alert behavior](https://docs.github.com/en/code-security/concepts/supply-chain-security/dependabot-alerts)
