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
  const installed = await installBuilt(root, consumer, ["pricing-workbench"]);
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
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1200, height: 1000 },
  });
  const failures = [],
    requests = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    requests.push(r.url());
    if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
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
  await results.getByText("Result JSON", { exact: true }).click();
  const actual = JSON.parse(
    await results
      .getByRole("region", { name: "Valuation result JSON" })
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
  const axe = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(
    axe.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.failureSummary),
    })),
    [],
  );
  assert.deepEqual(failures, []);
  assert(requests.some((url) => url.endsWith(".wasm")));
  await page.screenshot({ path: "/tmp/pr016-workbench.png" });
  const report = {
    browser: browser.version(),
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
  await writeFile(
    "/tmp/pr016-workbench.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
