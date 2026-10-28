import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Development task that builds each package; `mise run wasm-pkg` builds both for release.
const builders = { pkg: "wasm-build", "pkg-node": "wasm-build-node" };

/** Exit before any work when a WASM package the caller needs is not built. */
export function requireWasm(names) {
  for (const name of names)
    if (!(name in builders)) throw new Error(`Unknown WASM package ${name}`);
  const missing = names.filter(
    (name) =>
      !existsSync(
        new URL(
          `../../finstack-quant-wasm/${name}/finstack_quant_wasm.js`,
          import.meta.url,
        ),
      ),
  );
  for (const name of missing)
    console.error(
      `finstack-quant-wasm/${name} is not built; run \`mise run ${builders[name]}\` (dev) or \`mise run wasm-pkg\` (release) first.`,
    );
  if (missing.length) process.exit(1);
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  requireWasm(process.argv.slice(2));
