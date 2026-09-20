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
const consumer = await mkdtemp(path.join(root, ".consumer-results-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, ["measures-grid"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/results.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "src/fixtures/results/bond.json")),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Valuation results</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
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
  await page.getByText("SUPPLIED-EUR-CONTEXT", { exact: true }).waitFor();
  const firstPanel = await page
    .getByRole("region", { name: "Valuation", exact: true })
    .boundingBox();
  const secondPanel = await page
    .getByRole("region", { name: "Comparison valuation" })
    .boundingBox();
  assert(
    firstPanel.x < secondPanel.x && firstPanel.y === secondPanel.y,
    "Comparison panels are side by side at desktop width",
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = [];
  const heights = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    for (const density of ["compact", "comfortable"]) {
      const current = await page
        .locator("[data-density]")
        .first()
        .getAttribute("data-density");
      if (current !== density)
        await page.getByRole("button", { name: "Toggle density" }).click();
      const axe = await page.evaluate(() => window.axe.run(document));
      accessibility.push({
        theme,
        density,
        violations: axe.violations.map((v) => ({
          id: v.id,
          nodes: v.nodes.map((n) => n.failureSummary),
        })),
      });
      const cells = await page
        .locator("tbody td:not(:first-child)")
        .evaluateAll((nodes) =>
          nodes.map((node) => ({
            align: getComputedStyle(node).textAlign,
            numeric: getComputedStyle(node).fontVariantNumeric,
          })),
        );
      assert(
        cells.every(
          (cell) =>
            cell.align === "right" && cell.numeric.includes("tabular-nums"),
        ),
      );
      heights.push({
        theme,
        density,
        height: await page
          .locator("tbody tr")
          .first()
          .evaluate((node) => node.getBoundingClientRect().height),
      });
    }
  }
  assert(
    accessibility.every((mode) => mode.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert(
    heights
      .filter((row) => row.density === "comfortable")
      .every(
        (row) =>
          row.height >
          heights.find(
            (other) => other.theme === row.theme && other.density === "compact",
          ).height,
      ),
  );
  assert(
    (
      await page
        .getByRole("region", { name: "Valuation", exact: true })
        .textContent()
    ).includes("USD 1,042,500"),
  );
  assert(
    (
      await page
        .getByRole("region", { name: "Comparison valuation" })
        .textContent()
    ).includes("EUR 987,654.32"),
  );
  assert.equal(
    await page
      .getByText("bucketed_dv01::USD-OIS::30y", { exact: true })
      .count(),
    1,
  );
  assert.equal(
    await page.getByText("Unit unavailable", { exact: true }).count(),
    30,
  );
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  assert(
    !modules.some((id) =>
      /finstack-quant-wasm|@tanstack\/charts|@tanstack\/react-form/.test(id),
    ),
  );
  await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
  await page.screenshot({ path: "/tmp/pr013-results.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    accessibility,
    heights,
    failures,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr013-results.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
