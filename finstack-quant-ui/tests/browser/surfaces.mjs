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
const consumer = await mkdtemp(path.join(root, ".consumer-surfaces-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, [
    "vol-surface-chart",
    "fx-delta-quotes",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/surfaces.tsx", import.meta.url)),
  );
  const stored = {
    id: "Supplied surface",
    expiries: [0.5, 1, 2],
    strikes: [80, 100, 120],
    secondary_axis: "strike",
    quote_type: "black_lognormal",
    interpolation_mode: "total_variance",
    vols_row_major: [0.21, 0.2, 0.23, 0.22, 0.24, 0.26, 0.25, 0.27, 0.28],
  };
  const normal = {
    ...stored,
    id: "Normal tenor",
    strikes: [1, 5, 10],
    secondary_axis: "tenor",
    quote_type: "normal",
    vols_row_major: [
      0.005, 0.006, 0.007, 0.008, 0.009, 0.01, 0.011, 0.012, 0.013,
    ],
  };
  const large = {
    ...stored,
    id: "Large",
    expiries: Array.from({ length: 13 }, (_, i) => i + 1),
    strikes: Array.from({ length: 13 }, (_, i) => 80 + i * 5),
    vols_row_major: Array.from({ length: 169 }, (_, i) => 0.1 + i / 1000),
  };
  const fx = {
    id: "EURUSD",
    expiries: [0.5, 1],
    atm_vols: [0.08, 0.09],
    rr_25d: [0.01, 0.012],
    bf_25d: [0.005, 0.006],
    rr_10d: null,
    bf_10d: null,
  };
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const bond = JSON.parse(
    await readFile(path.join(root, "src/fixtures/results/bond.json"), "utf8"),
  );
  const market = new native.Market(
    JSON.stringify({
      ...JSON.parse(bond.request.marketJson),
      surfaces: [stored, normal, large],
      fx_delta_vol_surfaces: [fx],
    }),
  );
  try {
    const canonical = JSON.parse(market.toJson());
    for (const surface of [stored, normal, large])
      assert.deepEqual(
        canonical.surfaces.find((s) => s.id === surface.id),
        surface,
      );
    assert.deepEqual(canonical.fx_delta_vol_surfaces, [fx]);
  } finally {
    market.free();
  }
  await writeFile(
    path.join(consumer, "fixture.json"),
    JSON.stringify({ stored, normal, large, fx }),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Publication figures</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
  );
  await build({
    root: consumer,
    configFile: false,
    logLevel: "error",
    resolve: { alias: { "@": consumer } },
    css: { postcss: { plugins: [tailwind()] } },
    build: { outDir: path.join(consumer, "dist"), emptyOutDir: true },
  });
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1050, height: 1000 },
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
    () => document.querySelectorAll("svg.ts-chart").length === 3,
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const surface = page.getByRole("region", {
    name: "Supplied surface stored surface",
    exact: true,
  });
  const output = page.getByLabel("Accepted node");
  await surface.getByRole("gridcell", { name: "0.24", exact: true }).click();
  assert.equal(
    await output.textContent(),
    JSON.stringify(["Supplied surface", 1, 100]),
  );
  await page
    .locator('[aria-label="Supplied surface row slice"][tabindex]')
    .waitFor();
  const heatmap = page.locator(
    '[aria-label="Supplied surface heatmap"][tabindex]',
  );
  await heatmap.focus();
  await heatmap.press("Home");
  await heatmap.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Accepted node"]').textContent ===
      JSON.stringify(["Supplied surface", 0.5, 80]),
  );
  assert.equal(
    await surface
      .getByRole("gridcell", { name: "0.21", exact: true })
      .getAttribute("aria-selected"),
    "true",
  );
  await heatmap.press("Escape");
  await surface
    .getByLabel("Stored coordinate")
    .selectOption(JSON.stringify(["Supplied surface", 2, 120]));
  assert.equal(
    await output.textContent(),
    JSON.stringify(["Supplied surface", 2, 120]),
  );
  await page.getByText("Select middle externally", { exact: true }).click();
  assert.equal(
    await surface.getByLabel("Stored coordinate").inputValue(),
    JSON.stringify(["Supplied surface", 1, 100]),
  );
  const exports = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    for (const kind of ["heatmap", "row", "column"]) {
      const svg = await page.evaluate(
        ({ kind, theme }) => window.surfaceProbe.export(kind, theme),
        { kind, theme },
      );
      for (const text of [
        "Stored coordinates only",
        "Native validated fixture",
        "Supplied annotation",
      ])
        assert(svg.includes(text));
      assert(!svg.includes("Caller node") && !svg.includes("var("));
      await writeFile(`/tmp/pr028-${kind}-${theme}.svg`, svg);
      exports.push({ kind, theme, bytes: Buffer.byteLength(svg) });
    }
    await surface.screenshot({
      path: `/tmp/pr028-${theme}.png`,
      fullPage: true,
    });
  }
  await surface.getByText("Clear selected node", { exact: true }).click();
  assert.equal(await output.textContent(), "none");
  assert.equal(
    await page
      .locator('[aria-label="Supplied surface row slice"][tabindex]')
      .count(),
    0,
  );
  await page.getByText("Select middle externally", { exact: true }).click();
  await page.getByText("Remove selected expiry", { exact: true }).click();
  assert.equal(await surface.getByLabel("Stored coordinate").inputValue(), "");
  assert.equal(
    await page
      .locator('[aria-label="Supplied surface row slice"][tabindex]')
      .count(),
    0,
  );
  assert.equal(
    await surface.locator('[data-ts-key*="heatmap-accepted"]').count(),
    0,
  );
  const largeView = page.getByRole("region", {
    name: "Large stored surface",
    exact: true,
  });
  assert.equal(
    await largeView.locator('[data-ts-key*="heatmap-values"]').count(),
    0,
  );
  assert(await page.getByText("Tenor (years)", { exact: true }).count());
  const axe = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(axe.violations, []);
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  const report = {
    browser: browser.version(),
    installed,
    exports,
    accessibilityViolations: 0,
    wasmRequests: 0,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr028-surfaces.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
