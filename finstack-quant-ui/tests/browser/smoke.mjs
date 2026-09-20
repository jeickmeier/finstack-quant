import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { chromium } from "playwright";
import init, { valuations } from "../../../finstack-quant-wasm/index.js";
import { serializeHost } from "../../src/codec.mjs";
import { pricingCases } from "./cases.mjs";
import { serveExport } from "./static-server.mjs";

const repo = resolve(import.meta.dirname, "../../..");
const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
const requests = await pricingCases();
await init({
  module_or_path: await readFile(
    resolve(repo, "finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm"),
  ),
});
function baseline(request) {
  try {
    return {
      ok: true,
      value: valuations.instruments.priceInstrument(
        request.instrumentJson,
        request.marketJson,
        request.asOf,
        request.model,
        request.metrics,
        request.pricingOptions,
        request.marketHistory,
      ),
    };
  } catch (error) {
    const value = {
      name: error.name,
      message: error.message,
      kind: error.kind,
    };
    for (const key of [
      "report",
      "cause",
      "stage",
      "step_id",
      "solver_diagnostics",
      "details",
    ])
      if (error[key] !== undefined) value[key] = error[key];
    return { ok: false, error: value };
  }
}
const expected = Object.fromEntries(
  Object.entries(requests).map(([key, request]) => [key, baseline(request)]),
);
assert(
  expected.stochastic.value.details.data.seed > BigInt(Number.MAX_SAFE_INTEGER),
);
assert.equal(expected.badInstrument.ok, false);
assert.equal(expected.missingMarket.ok, false);
const server = await serveExport(
  process.env.REGISTRY_EXPORT_DIR ?? resolve(repo, "docs-site/out"),
  basePath,
);
let browser;
try {
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  await context.addInitScript(() => {
    for (const name of ["instantiate", "instantiateStreaming"])
      WebAssembly[name] = () => {
        throw new Error("Main-thread WASM is forbidden in the registry probe");
      };
  });
  const assets = [];
  context.on("response", (response) => {
    if (/\.(wasm|js)(\?|$)/.test(response.url()))
      assets.push({
        url: response.url(),
        status: response.status(),
        mime: response.headers()["content-type"],
      });
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`${server.url}/registry-probe/`);
  await page.waitForFunction(
    () => ["ready", "failed"].includes(window.registryProbe?.state),
    undefined,
    { timeout: 120_000 },
  );
  assert.deepEqual(
    await page.evaluate(() => window.registryProbe.initialization),
    { ok: true, value: { state: "ready", worker: true } },
  );
  for (const [name, request] of Object.entries(requests)) {
    const actual = await page.evaluate(
      (request) => window.registryProbe.proxy.price(request),
      request,
    );
    const reference = structuredClone(expected[name]);
    if (actual.ok) {
      // Each real pricing call supplies its own wall-clock stamp. Compare the
      // remaining result exactly, then round-trip this call's timestamp below.
      assert.equal(typeof actual.value.meta.timestamp, "string");
      reference.value.meta.timestamp = actual.value.meta.timestamp;
      const exported = await page.evaluate(
        (value) => window.registryProbe.proxy.exportResult(value),
        actual.value,
      );
      assert.deepEqual(exported, {
        ok: true,
        value: valuations.validateValuationResultJson(
          serializeHost(actual.value),
        ),
      });
    }
    assert.deepEqual(actual, reference, name);
    if (name === "stochastic") {
      const exported = await page.evaluate(async (request) => {
        const result = await window.registryProbe.proxy.price(request);
        return window.registryProbe.proxy.exportResult(result.value);
      }, request);
      assert.equal(exported.ok, true);
      assert(
        exported.value.includes(`"seed":${actual.value.details.data.seed}`),
      );
    }
  }
  const originalText = valuations.instruments.instrumentCashflowsJson(
    requests.bond.instrumentJson,
    requests.bond.marketJson,
    requests.bond.asOf,
    requests.bond.model,
  );
  assert.deepEqual(
    await page.evaluate(
      (request) => window.registryProbe.proxy.cashflows(request),
      requests.bond,
    ),
    { ok: true, value: originalText },
  );
  assert(
    assets.some(
      (asset) =>
        asset.url.endsWith(".wasm") &&
        asset.status === 200 &&
        asset.mime === "application/wasm",
    ),
  );
  assert(
    assets
      .filter((asset) => asset.url.endsWith(".js"))
      .every((asset) => asset.status === 200 && /javascript/.test(asset.mime)),
  );
  assert(
    assets.every((asset) =>
      new URL(asset.url).pathname.startsWith(`${basePath}/`),
    ),
  );
  assert.equal(page.workers().length, 1);
  const workerUrl = page.workers()[0].url().split("#")[0];
  assert(
    assets.some(
      (asset) =>
        asset.url === workerUrl &&
        asset.status === 200 &&
        /javascript/.test(asset.mime),
    ),
  );
  assert.deepEqual(errors, []);
  const failed = await context.newPage();
  await failed.goto(
    `${server.url}/registry-probe/?wasm=${encodeURIComponent(`${server.url}/missing.wasm`)}`,
  );
  await failed.waitForFunction(
    () => window.registryProbe?.state === "failed",
    undefined,
    { timeout: 120_000 },
  );
  const initializationError = await failed.evaluate(
    () => window.registryProbe.initialization,
  );
  assert.equal(initializationError.ok, false);
  assert.equal(typeof initializationError.error.message, "string");
  assert(initializationError.error.message.length > 0);
  const report = {
    browser: await browser.version(),
    platform: process.platform,
    arch: process.arch,
    basePath,
    verdict: "pass",
    seed: expected.stochastic.value.details.data.seed.toString(),
    cases: Object.keys(requests),
    initializationError,
    workerUrl,
    errors: Object.fromEntries(
      Object.entries(expected).filter(([, result]) => !result.ok),
    ),
    assets,
  };
  const output = process.env.REGISTRY_SMOKE_REPORT;
  if (output) await writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server.close();
}
