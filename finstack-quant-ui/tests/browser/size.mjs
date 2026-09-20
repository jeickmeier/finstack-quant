import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { gzipSync, brotliCompressSync, constants } from "node:zlib";

const [rawPath, optimizedPath, output] = process.argv.slice(2);
if (!rawPath || !optimizedPath || !output)
  throw new Error(
    "Usage: node size.mjs <raw.wasm> <optimized.wasm> <report.json>",
  );
const raw = await readFile(rawPath);
const optimized = await readFile(optimizedPath);
const hash = (value) => createHash("sha256").update(value).digest("hex");
const report = {
  measuredAt: new Date().toISOString(),
  rawBytes: raw.length,
  optimizedBytes: optimized.length,
  gzipBytes: gzipSync(optimized, { level: 9 }).length,
  brotliBytes: brotliCompressSync(optimized, {
    params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
  }).length,
  rawSha256: hash(raw),
  optimizedSha256: hash(optimized),
  limitBytes: 10_000_000,
  verdict: optimized.length <= 10_000_000 ? "pass" : "fail",
  method:
    "Raw is wasm-bindgen output from release-size with --no-opt. Optimized uses wasm-opt -Oz with the existing release feature allowlist. Compression is gzip level 9 and Brotli quality 11; neither changes the raw-byte gate.",
};
await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));

if (report.verdict === "fail") process.exitCode = 1;
