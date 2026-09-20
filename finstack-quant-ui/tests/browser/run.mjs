import { spawn } from "node:child_process";
import { cp, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

const repo = resolve(import.meta.dirname, "../../..");
const fixture = await mkdtemp(resolve(repo, "docs-site/.registry-probe-"));
const evidence =
  process.env.REGISTRY_SMOKE_DIR ??
  (await mkdtemp(resolve(tmpdir(), "registry-smoke-")));
await mkdir(evidence, { recursive: true });
function run(command, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repo,
      env: { ...process.env, ...env },
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) =>
      code === 0
        ? resolve()
        : reject(new Error(`${command} failed: ${code ?? signal}`)),
    );
  });
}
try {
  await mkdir(resolve(fixture, "app/registry-probe"), { recursive: true });
  for (const name of ["page.jsx", "probe.worker.js"])
    await cp(
      resolve(import.meta.dirname, "fixture", name),
      resolve(fixture, "app/registry-probe", name),
    );
  await cp(
    resolve(import.meta.dirname, "fixture/layout.jsx"),
    resolve(fixture, "app/layout.jsx"),
  );
  await writeFile(
    resolve(fixture, "next.config.mjs"),
    "import { config } from '../next.config.mjs';\nexport default config;\n",
  );
  for (const [name, basePath] of [
    ["root", ""],
    ["base-path", "/finstack-quant"],
  ]) {
    console.log(`Building isolated production export: ${name}`);
    await run(
      process.execPath,
      [
        resolve(repo, "docs-site/node_modules/next/dist/bin/next"),
        "build",
        fixture,
      ],
      { NEXT_PUBLIC_BASE_PATH: basePath },
    );
    await run(process.execPath, [resolve(import.meta.dirname, "smoke.mjs")], {
      NEXT_PUBLIC_BASE_PATH: basePath,
      REGISTRY_EXPORT_DIR: resolve(fixture, "out"),
      REGISTRY_SMOKE_REPORT: resolve(evidence, `${name}.json`),
    });
  }
  console.log(`Production browser evidence: ${evidence}`);
} finally {
  await rm(fixture, { recursive: true, force: true });
}
