import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { gzipSync, brotliCompressSync, constants } from 'node:zlib';

/** Measure build artifacts without applying a consumer-specific size policy. */
export async function measureWasm(rawPath, optimizedPath) {
  const raw = await readFile(rawPath);
  const optimized = await readFile(optimizedPath);
  const hash = (value) => createHash('sha256').update(value).digest('hex');
  return {
    measuredAt: new Date().toISOString(),
    rawBytes: raw.length,
    optimizedBytes: optimized.length,
    gzipBytes: gzipSync(optimized, { level: 9 }).length,
    brotliBytes: brotliCompressSync(optimized, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
    rawSha256: hash(raw),
    optimizedSha256: hash(optimized),
    method:
      'Raw is wasm-bindgen output from release-size with --no-opt. Optimized uses wasm-opt -Oz with the existing release feature allowlist. Compression is gzip level 9 and Brotli quality 11.',
  };
}
