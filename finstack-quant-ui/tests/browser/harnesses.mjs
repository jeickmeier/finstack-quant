// Standalone browser harnesses. Each one installs its items from a fresh
// `shadcn build` (tests/browser/consumer.mjs installBuilt) into a Vite consumer
// and checks behaviour only a real browser shows. None of them needs a WASM
// build or `ui-build`. `mise run ui-harness` runs this file.
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

const tests = path.resolve(import.meta.dirname, "..");
const harnesses = [
  "shared/browser/figures.mjs",
  "shared/browser/primitives.mjs",
  "shared/browser/interactions.mjs",
  "valuations/browser/heatmap.mjs",
  "valuations/browser/results.mjs",
];
const evidence =
  process.env.REGISTRY_SMOKE_DIR ??
  (await mkdtemp(path.join(tmpdir(), "registry-harnesses-")));
await mkdir(evidence, { recursive: true });
for (const harness of harnesses) {
  console.log(`Running tests/${harness}`);
  const { status } = spawnSync(process.execPath, [path.join(tests, harness)], {
    stdio: "inherit",
    env: {
      ...process.env,
      REGISTRY_SMOKE_DIR: path.join(evidence, path.basename(harness, ".mjs")),
    },
  });
  if (status !== 0) {
    console.error(`tests/${harness} failed; evidence: ${evidence}`);
    process.exit(status ?? 1);
  }
}
console.log(`Browser harnesses passed: ${evidence}`);
