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
const consumer = await mkdtemp(path.join(root, ".consumer-curves-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, ["curve-chart"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/curves.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "src/fixtures/curves/market.json")),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Stored curves</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
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
  assert(!modules.some((id) => /finstack-quant-wasm/.test(id)));
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
  await page.waitForFunction(
    () => document.querySelectorAll("svg.ts-chart").length === 7,
  );
  assert(installed.includes("contract-market-context-state"));
  assert(!installed.includes("instrument-catalogue"));
  assert.equal(await page.getByRole("region", { name: / panel$/ }).count(), 9);
  const chart = page.locator('[aria-label="discount stored curves"][tabindex]');
  await chart.focus();
  await chart.press("Home");
  await chart.press("ArrowRight");
  // Locate the original high-precision OVERLAY knot through native keyboard focus.
  for (
    let i = 0;
    i < 30 &&
    !(await page.locator("[data-status]").textContent()).includes(
      "focus: OVERLAY/1;",
    );
    i++
  )
    await chart.press("ArrowRight");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /focus: OVERLAY\/1;/,
  );
  assert(
    (await page.locator(".ts-chart-tooltip").textContent()).includes(
      "0.9512345678901234",
    ),
  );
  await chart.press("Escape");
  await page.getByRole("button", { name: "Toggle custom tooltip" }).click();
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /activations: 1/,
  );
  const action = page.getByRole("button", { name: "Open curve details" });
  await action.waitFor();
  await chart.press("Tab");
  assert(await action.evaluate((node) => node === document.activeElement));
  const svg = await page.evaluate(() => window.exportCurve());
  for (const label of [
    "Stored market observations",
    "Canonical state supplied by native Market",
    "Stored coordinate",
    "Stored value",
    "Supplied knot note",
    "Publication note",
    "Retained native fixture",
    "Discount curve state",
    "OVERLAY",
    "USD-OIS",
  ])
    assert(svg.includes(label), `Export retains ${label}`);
  assert(!svg.includes("Open curve details") && !svg.includes("<image"));
  await writeFile("/tmp/pr014-curves.svg", svg);
  await action.press("Enter");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /activations: 1/,
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    const axe = await page.evaluate(() => window.axe.run(document));
    accessibility.push({
      theme,
      violations: axe.violations.map((v) => ({
        id: v.id,
        nodes: v.nodes.map((n) => n.failureSummary),
      })),
    });
  }
  assert(
    accessibility.every((x) => x.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
  await page.screenshot({ path: "/tmp/pr014-curves.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    accessibility,
    failures,
    panels: 9,
    plottedVariants: 7,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr014-curves.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
