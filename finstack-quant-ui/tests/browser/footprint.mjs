import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { cpus, release, totalmem } from "node:os";
import { resolve } from "node:path";
import { chromium } from "playwright";
import { serveExport } from "./static-server.mjs";

const output = process.env.REGISTRY_FOOTPRINT_REPORT;
if (!output || !process.env.REGISTRY_EXPORT_DIR)
  throw new Error("Set REGISTRY_FOOTPRINT_REPORT and REGISTRY_EXPORT_DIR");
const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
const server = await serveExport(
  resolve(process.env.REGISTRY_EXPORT_DIR),
  basePath,
);
const report = {
  method:
    "One fresh Chromium process; default cache with no prior navigation. Sequential ESM imports retained, explicit CDP garbage collection after each type. Memory is main-realm V8 heap, not total renderer or WASM-worker RSS. No timing threshold or cross-device performance claim.",
  measuredAt: new Date().toISOString(),
  platform: process.platform,
  os: release(),
  cpu: cpus()[0].model,
  memoryBytes: totalmem(),
  samples: [],
  verdict: "incomplete",
};
const save = () => writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
let browser;
try {
  browser = await chromium.launch({ headless: true });
  report.browser = await browser.version();
  const page = await browser.newPage();
  await page.addInitScript(() =>
    performance.setResourceTimingBufferSize(10000),
  );
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("HeapProfiler.enable");
  await page.goto(`${server.url}/registry-probe/`);
  await page.waitForFunction(
    () => ["ready", "failed"].includes(window.registryProbe?.state),
    undefined,
    { timeout: 120_000 },
  );
  assert.equal(await page.evaluate(() => window.registryProbe.state), "ready");
  report.coldInitializationMs = await page.evaluate(
    () => window.registryProbe.initializationMs,
  );
  const fixtures = JSON.parse(
    await readFile(
      new URL("../../src/generated/fixtures.json", import.meta.url),
      "utf8",
    ),
  ).filter((fixture) => fixture.kind === "instrument");
  const repo = resolve(import.meta.dirname, "../../..");
  const expected = (
    await Promise.all(
      fixtures.map(
        async (fixture) =>
          JSON.parse(await readFile(resolve(repo, fixture.source), "utf8"))
            .instrument.type,
      ),
    )
  ).sort();
  const types = await page.evaluate(() =>
    window.registryProbe.instrumentTypes(),
  );
  // The source catalogue is checked against the generated fixture manifest.
  assert.deepEqual([...types].sort(), expected);
  await cdp.send("HeapProfiler.collectGarbage");
  report.beforeInstruments = await cdp.send("Runtime.getHeapUsage");
  await save();
  for (const type of types) {
    console.log(
      `Measuring ${report.samples.length + 1}/${types.length}: ${type}`,
    );
    const sample = await page.evaluate(
      (type) => window.registryProbe.measureInstrument(type),
      type,
    );
    await cdp.send("HeapProfiler.collectGarbage");
    sample.heapAfterCollection = await cdp.send("Runtime.getHeapUsage");
    report.samples.push(sample);
    await save();
  }
  report.afterOne = report.samples[0].heapAfterCollection;
  report.afterAll = report.samples.at(-1).heapAfterCollection;
  report.discoveredCount = types.length;
  report.completedCount = report.samples.length;
  report.verdict = "complete-corpus";
  await save();
  console.log(`Completed ${types.length} validators; evidence: ${output}`);
} catch (error) {
  report.verdict = "failed";
  report.error = { name: error.name, message: error.message };
  await save();
  throw error;
} finally {
  await browser?.close();
  await server.close();
}
