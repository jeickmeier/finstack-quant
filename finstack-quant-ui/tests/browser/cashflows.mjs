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
const consumer = await mkdtemp(path.join(root, ".consumer-cashflows-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, [
    "cashflow-viewer",
    "use-price-instrument",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/cashflows.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/cashflows/cases.json")),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Cashflow exports</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
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
  assert(
    !modules.some((id) =>
      /tanstack\/(?:react-)?table|finstack-quant-wasm/.test(id),
    ),
  );
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1000, height: 1000 },
  });
  const failures = [],
    requests = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    requests.push(r.url());
    if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
  });
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"], {
    origin: server.url,
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
  const fixture = JSON.parse(
    await readFile(path.join(root, "tests/cashflows/cases.json"), "utf8"),
  );
  const viewer = page.getByRole("region", { name: "Cashflows", exact: true });
  const accessibility = [];
  const checks = [];
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  for (const entry of fixture.cases) {
    await page.getByRole("button", { name: entry.type, exact: true }).click();
    await page.waitForFunction(
      (expected) =>
        document.querySelector('[aria-label="Valuation"]')?.textContent ===
        expected,
      `${entry.pricedValue.amount} ${entry.pricedValue.currency}`,
    );
    const valuation = await page
      .getByLabel("Valuation", { exact: true })
      .textContent();
    assert.equal(
      await page.getByLabel("Selected model").textContent(),
      entry.request.model,
    );
    if (entry.cashflows !== null) {
      await viewer
        .getByRole("button", { name: "Original", exact: true })
        .click();
      await page.waitForFunction(
        (expected) =>
          document.querySelector('section[aria-label="Cashflows"] pre')
            ?.textContent === expected,
        entry.cashflows,
      );
      for (const density of ["compact", "comfortable"]) {
        if ((await viewer.getAttribute("data-density")) !== density)
          await page.getByRole("button", { name: "Toggle density" }).click();
        assert.equal(
          await viewer.locator("pre").textContent(),
          entry.cashflows,
        );
        await viewer.getByRole("button", { name: "Copy", exact: true }).click();
        await page.getByText("Copied", { exact: true }).waitFor();
        assert.equal(
          await page.evaluate(() => navigator.clipboard.readText()),
          entry.cashflows,
        );
        const audit = await page.evaluate(() => window.axe.run(document));
        accessibility.push({
          type: entry.type,
          density,
          violations: audit.violations,
        });
        assert.deepEqual(audit.violations, []);
      }
      await page.emulateMedia({ media: "print" });
      const printSource = viewer.locator("[data-json-print-source]");
      assert.equal(await printSource.isVisible(), true);
      assert.equal(await viewer.locator("pre").isVisible(), false);
      const print = await printSource.evaluate((node) => ({
        text: node.textContent,
        wrap: getComputedStyle(node).whiteSpace,
        maxHeight: getComputedStyle(node).maxHeight,
      }));
      assert.equal(
        await printSource.evaluate(
          (node) => node.scrollWidth <= node.clientWidth,
        ),
        true,
        "Printed raw JSON must wrap within the page width",
      );
      assert.deepEqual(print, {
        text: entry.cashflows,
        wrap: "pre-wrap",
        maxHeight: "none",
      });
      assert.equal(
        await viewer
          .getByRole("button", { name: "Copy", exact: true })
          .isVisible(),
        false,
      );
      if (entry.type === "fx_swap") {
        await page.pdf({
          path: "/tmp/pr022-cashflows.pdf",
          format: "A4",
          printBackground: true,
        });
        await page.screenshot({
          path: "/tmp/pr022-cashflows.png",
          fullPage: true,
        });
      }
      await page.emulateMedia({ media: "screen" });
    } else {
      await viewer.getByRole("alert").waitFor();
      assert.equal(
        await viewer.getByRole("alert").textContent(),
        `Cashflows unavailable: ${entry.error}`,
      );
      assert.equal(
        await page.getByLabel("Valuation", { exact: true }).textContent(),
        valuation,
      );
      assert.equal(
        await page.getByLabel("Selected model").textContent(),
        entry.request.model,
      );
      assert.equal(
        await viewer
          .getByRole("button", { name: "Copy", exact: true })
          .isDisabled(),
        true,
      );
      const audit = await page.evaluate(() => window.axe.run(document));
      assert.deepEqual(audit.violations, []);
      accessibility.push({ type: entry.type, violations: audit.violations });
    }
    checks.push({
      type: entry.type,
      exactNativeText: entry.cashflows !== null,
      nativeError: entry.error,
      valuationRetained: true,
      selectedModel: entry.request.model,
    });
  }
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 1);
  const report = {
    browser: browser.version(),
    installed,
    checks,
    accessibility,
    failures,
    print: "Original text wraps without clipping; toolbar hidden",
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr022-cashflows.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
