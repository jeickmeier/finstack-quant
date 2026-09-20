import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const pkg = resolve(
  process.env.REGISTRY_WASM_PACKAGE ??
    fileURLToPath(new URL("../../../finstack-quant-wasm/pkg", import.meta.url)),
);
const measured = JSON.parse(
  await readFile(resolve(pkg, "size-report.json"), "utf8"),
);
const binary = await readFile(resolve(pkg, "finstack_quant_wasm_bg.wasm"));
assert.equal(
  binary.length,
  measured.optimizedBytes,
  "Size report must match the selected WASM artifact",
);
assert.equal(
  createHash("sha256").update(binary).digest("hex"),
  measured.optimizedSha256,
  "Size report must identify the selected WASM artifact",
);
const limitBytes = 25_000_000;
const report = {
  ...measured,
  limitBytes,
  verdict: binary.length <= limitBytes ? "pass" : "fail",
};
console.log(JSON.stringify(report, null, 2));
if (report.verdict === "fail") process.exitCode = 1;
