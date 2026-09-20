import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { cp, mkdir, mkdtemp, rm, writeFile, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

if (
  process.env.REGISTRY_MEASURE_FOOTPRINT === "1" &&
  !process.env.REGISTRY_WASM_PACKAGE
)
  throw new Error(
    "Set REGISTRY_WASM_PACKAGE to the optimized web package before footprint measurements",
  );
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
  for (const name of ["page.jsx", "probe.worker.js", "footprint.js"])
    await cp(
      resolve(import.meta.dirname, "fixture", name),
      resolve(fixture, "app/registry-probe", name),
    );
  await cp(
    resolve(import.meta.dirname, "fixture/layout.jsx"),
    resolve(fixture, "app/layout.jsx"),
  );
  if (process.env.REGISTRY_WASM_PACKAGE) {
    // Use matching wasm-bindgen glue with the optimized binary. Preserve the
    // repository package and all public facade sources unchanged.
    const wasmPackage = resolve(fixture, "node_modules/finstack-quant-wasm");
    await mkdir(wasmPackage, { recursive: true });
    for (const name of [
      "package.json",
      "index.js",
      "index.d.ts",
      "exports",
      "types",
    ])
      await cp(
        resolve(repo, "finstack-quant-wasm", name),
        resolve(wasmPackage, name),
        { recursive: true },
      );
    await cp(
      resolve(process.env.REGISTRY_WASM_PACKAGE),
      resolve(wasmPackage, "pkg"),
      { recursive: true },
    );
  }
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
      REGISTRY_EXPECTED_WASM_SHA256: createHash("sha256")
        .update(
          await readFile(
            resolve(
              process.env.REGISTRY_WASM_PACKAGE ??
                resolve(repo, "finstack-quant-wasm/pkg"),
              "finstack_quant_wasm_bg.wasm",
            ),
          ),
        )
        .digest("hex"),
    });
  }
  if (process.env.REGISTRY_MEASURE_FOOTPRINT === "1") {
    await run(
      process.execPath,
      [resolve(import.meta.dirname, "footprint.mjs")],
      {
        NEXT_PUBLIC_BASE_PATH: "/finstack-quant",
        REGISTRY_EXPORT_DIR: resolve(fixture, "out"),
        REGISTRY_FOOTPRINT_REPORT: resolve(evidence, "footprint.json"),
      },
    );
  }
  console.log(`Production browser evidence: ${evidence}`);
} finally {
  await rm(fixture, { recursive: true, force: true });
}
