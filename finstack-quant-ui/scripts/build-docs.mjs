import { cp, mkdir, rm, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { installProbe } from "../tests/browser/install-probe.mjs";

// Export the real registry docs route/layout without unrelated notebook routes.
const repo = fileURLToPath(new URL("../../", import.meta.url));
const site = path.join(repo, "docs-site");
const stage = path.join(site, ".registry-publish");
await rm(stage, { recursive: true, force: true });
await mkdir(stage, { recursive: true });
for (const name of [
  "package.json",
  "tsconfig.json",
  "postcss.config.mjs",
  "source.config.ts",
  "src",
  "content/docs",
  "public/r",
]) {
  await cp(path.join(site, name), path.join(stage, name), {
    recursive: true,
    filter: (source) => source !== path.join(site, "src/app"),
  });
}
for (const name of [
  "layout.tsx",
  "globals.css",
  "(reader)/layout.tsx",
  "(reader)/docs",
  "(gallery)",
])
  await cp(
    path.join(site, "src/app", name),
    path.join(stage, "src/app", name),
    { recursive: true },
  );
await mkdir(path.join(stage, "content/learn"), { recursive: true });
await mkdir(path.join(stage, "content/labs"), { recursive: true });
await writeFile(
  path.join(stage, "next.config.mjs"),
  "import config from '../next.config.mjs';\nexport default config;\n",
);
if (process.env.REGISTRY_TEST_PROBE === "1")
  await installProbe(path.join(stage, "src"));
// This is only a navigation projection, not curriculum or execution certification.
const projection = `from pathlib import Path
import sys
sys.path.insert(0, ${JSON.stringify(path.join(site, "gen"))})
from common import SITE, curriculum, frontmatter, write_json
write_json(Path(${JSON.stringify(path.join(stage, ".build/curriculum.json"))}), {"lessons": [{**record, **frontmatter(SITE / "content/learn" / record["path"])[0]} for record in curriculum()]})
`;
const run = (command, args) =>
  new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: stage,
      stdio: "inherit",
      env: process.env,
    });
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`${command} failed: ${code}`)),
    );
  });
await run("uv", [
  "run",
  "--no-sync",
  "--with",
  "pyyaml",
  "python",
  "-c",
  projection,
]);
await run(process.execPath, [
  path.join(site, "node_modules/next/dist/bin/next"),
  "build",
]);
console.log(`Registry docs export: ${path.join(stage, "out")}`);
