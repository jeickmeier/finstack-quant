import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import tailwind from "@tailwindcss/postcss";
import { chromium } from "playwright";
import { serveExport } from "./static-server.mjs";
import { installBuilt } from "./consumer.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-workbench-"));
let browser, server;
try {
  let installed = ["pricing-workbench"];
  if (!process.env.REGISTRY_EXPORT_DIR) {
    installed = await installBuilt(root, consumer, ["pricing-workbench"]);
    await writeFile(
      path.join(consumer, "main.tsx"),
      await readFile(new URL("./fixture/workbench.tsx", import.meta.url)),
    );
    await writeFile(
      path.join(consumer, "app.css"),
      '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
    );
    await writeFile(
      path.join(consumer, "index.html"),
      '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Embedded bond pricing</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
    );
    const modules = [];
    await build({
      root: consumer,
      configFile: false,
      logLevel: "error",
      resolve: { alias: { "@": consumer } },
      worker: { format: "es" },
      plugins: [
        {
          name: "inspect-dependencies",
          generateBundle(_, bundle) {
            for (const chunk of Object.values(bundle))
              if (chunk.type === "chunk")
                modules.push(...Object.keys(chunk.modules));
          },
        },
      ],
      css: { postcss: { plugins: [tailwind()] } },
      build: { outDir: path.join(consumer, "dist"), emptyOutDir: true },
    });
  }
  const exportDirectory =
    process.env.REGISTRY_EXPORT_DIR ?? path.join(consumer, "dist");
  const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
  server = await serveExport(exportDirectory, basePath);
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1200, height: 1000 },
  });
  const failures = [],
    requests = [],
    excludedPrefetchRequests = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    requests.push(r.url());
    const pathname = new URL(r.url()).pathname.slice(basePath.length);
    // Next prefetches unrelated navigation targets omitted by the scoped docs export.
    const outsideScope =
      process.env.REGISTRY_SCOPED_DOCS === "1" &&
      ["/", "/labs/"].includes(pathname);
    if (outsideScope && r.status() >= 400)
      excludedPrefetchRequests.push(r.url());
    if (r.status() >= 400 && !outsideScope)
      failures.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(
    `${server.url}${process.env.REGISTRY_WORKBENCH_PATH ?? "/"}`,
    { waitUntil: "networkidle" },
  );
  await page.getByText("USD 1,042,500", { exact: true }).waitFor();
  const results = page.getByRole("region", { name: "Results", exact: true });
  await results.getByText("Last priced request", { exact: true }).click();
  const old = JSON.parse(
    await results
      .getByRole("region", { name: "Last priced request JSON" })
      .locator("pre")
      .textContent(),
  );
  const amount = page.getByRole("textbox", { name: "Amount", exact: true });
  await amount.fill("1000000.123456789");
  await page.waitForFunction(() =>
    document
      .querySelector('[aria-label="Last priced request JSON"] pre')
      .textContent.includes("1000000.123456789"),
  );
  const request = JSON.parse(
    await results
      .getByRole("region", { name: "Last priced request JSON" })
      .locator("pre")
      .textContent(),
  );
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const direct = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model,
    request.metrics,
    request.pricingOptions,
    request.marketHistory,
  );
  await results.getByText("Complete result JSON", { exact: true }).click();
  const actual = JSON.parse(
    await results
      .getByRole("region", { name: "Complete valuation result JSON" })
      .locator("pre")
      .textContent(),
  );
  direct.meta.timestamp = actual.meta.timestamp;
  assert.deepEqual(actual, direct);
  assert.equal(old.marketJson, request.marketJson);
  await page.getByRole("tab", { name: "2 Market" }).click();
  await page.waitForFunction(
    () => document.querySelectorAll("svg.ts-chart").length === 2,
  );
  assert(
    await page.getByRole("region", { name: "Market snapshot" }).isVisible(),
  );
  await page.getByRole("tab", { name: "1 Instrument" }).click();
  assert.equal(await amount.inputValue(), "1000000.123456789");
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const axe = await page.evaluate(() =>
    window.axe.run(document.querySelector(".finstack-surface")),
  );
  assert.deepEqual(
    axe.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.failureSummary),
    })),
    [],
  );
  assert.deepEqual(failures, []);
  assert(requests.some((url) => url.endsWith(".wasm")));
  const output =
    process.env.REGISTRY_WORKBENCH_REPORT ?? "/tmp/pr016-workbench.json";
  let wasm;
  if (process.env.REGISTRY_EXPORT_DIR) {
    if (process.env.REGISTRY_SCOPED_DOCS === "1") {
      const response = await fetch(`${server.url}/r/pricing-workbench.json`);
      assert.equal(response.status, 200);
      assert.equal((await response.json()).name, "pricing-workbench");
    }
    assert(
      requests.some((url) => /\.woff2?(\?|$)/.test(url)),
      "local font requests",
    );
    const url = new URL(requests.find((url) => url.endsWith(".wasm")));
    assert(url.pathname.startsWith(`${basePath}/`));
    const bytes = await readFile(
      path.join(
        exportDirectory,
        decodeURIComponent(url.pathname.slice(basePath.length)),
      ),
    );
    wasm = {
      bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    };
    assert(wasm.bytes <= 25_000_000);
    assert.equal(wasm.sha256, process.env.REGISTRY_EXPECTED_WASM_SHA256);
    assert.equal(page.workers().length, 1);
  }
  await page.screenshot({ path: output.replace(/\.json$/, ".png") });
  const report = {
    browser: browser.version(),
    basePath,
    wasm,
    fonts: requests.filter((url) => /\.woff2?(\?|$)/.test(url)),
    excludedPrefetchRequests,
    installed,
    violations: [],
    failures,
    checks: [
      "edited block matches native facade",
      "persistent result context",
      "tabs retain controls",
      "stored market curves",
    ],
    verdict: "pass",
  };
  await writeFile(output, JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
