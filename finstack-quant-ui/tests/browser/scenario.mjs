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
const consumer = await mkdtemp(path.join(root, ".consumer-scenario-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, ["scenario-heatmap"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/scenario.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/scenarios/cases.json")),
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
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"], {
    origin: server.url,
  });
  const fixture = JSON.parse(
    await readFile(path.join(root, "tests/scenarios/cases.json"), "utf8"),
  );
  const checked = [];
  for (let index = 0; index < fixture.cases.length; index++) {
    if (index)
      await page.getByText("Switch balance fixture", { exact: true }).click();
    for (const severity of fixture.grid.severities) {
      await page
        .getByRole("radio", { name: String(severity), exact: true })
        .check();
      const expected = fixture.cases[index].table.cells.filter(
        (c) => c.severity === severity,
      );
      const table = page.getByRole("grid", {
        name: `${fixture.trancheId} selected scenario prices`,
        exact: true,
      });
      assert.deepEqual(
        await table
          .locator("tbody tr")
          .evaluateAll((rows) =>
            rows.map((row) =>
              [...row.querySelectorAll("td")].map((cell) => cell.textContent),
            ),
          ),
        expected.map((c) => [c.cpr, c.cdr, c.severity, c.price].map(String)),
      );
      const chart = page.locator(
        `[aria-label="${fixture.trancheId} scenario heatmap"][tabindex]`,
      );
      await chart.focus();
      await chart.press("Home");
      await chart.press("Enter");
      const key = JSON.stringify([
        fixture.trancheId,
        expected[0].cpr,
        expected[0].cdr,
        severity,
      ]);
      await page.waitForFunction(
        (key) => window.scenarioProbe.selectedKey === key,
        key,
      );
      assert.equal(
        await page.evaluate(() => window.scenarioProbe.activated),
        key,
      );
      await page
        .getByText(`Transient price ${expected[0].price}`, { exact: true })
        .waitFor();
      checked.push({
        fixture: fixture.cases[index].name,
        severity,
        prices: expected.map((c) => c.price),
      });
    }
  }
  await page
    .getByRole("region", {
      name: `${fixture.trancheId} complete native scenario table`,
      exact: true,
    })
    .getByRole("button", { name: "Copy", exact: true })
    .click();
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => navigator.clipboard.readText())),
    fixture.cases[1].table,
  );
  const exports = [],
    accessibility = [];
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    const svg = await page.evaluate(
      (theme) => window.scenarioProbe.export(theme),
      theme,
    );
    for (const label of [
      "CPR (annual decimal)",
      "CDR (annual decimal)",
      "Clean settlement price (% of current tranche balance)",
      "Par reference 100",
      "Original native prices",
      "Canonical Rust ScenarioCell.price",
      "Native returned cells",
    ])
      assert(svg.includes(label), label);
    assert(!svg.includes("Transient price"));
    const values = await page.evaluate((svg) => {
      const host = document.createElement("div");
      host.innerHTML = svg;
      const result = [...host.querySelectorAll("text")]
        .filter((n) =>
          n.getAttribute("data-ts-key")?.includes("heatmap-values"),
        )
        .map((n) => n.textContent);
      return result;
    }, svg);
    assert.deepEqual(
      values,
      fixture.cases[1].table.cells
        .filter((c) => c.severity === 0.6)
        .map((c) => String(c.price)),
    );
    await writeFile(`/tmp/pr034-${theme}.svg`, svg);
    exports.push({ theme, values });
    const axe = await page.evaluate(() => window.axe.run(document));
    accessibility.push({ theme, violations: axe.violations });
    await page
      .getByRole("region", {
        name: `${fixture.trancheId} scenario prices`,
        exact: true,
      })
      .screenshot({ path: `/tmp/pr034-${theme}.png` });
  }
  await page.getByText("Toggle standalone example", { exact: true }).click();
  await page.getByRole("radio", { name: "0.2", exact: true }).check();
  assert(
    await page
      .getByRole("region", {
        name: `${fixture.trancheId} scenario prices`,
        exact: true,
      })
      .isVisible(),
  );
  assert(
    accessibility.every((a) => a.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  const report = {
    browser: browser.version(),
    installed,
    checked,
    exports,
    accessibility,
    wasmRequests: 0,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr034-scenario.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
