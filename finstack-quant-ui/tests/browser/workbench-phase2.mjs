import { parse } from "lossless-json";
import { serializeHost } from "../../src/codec.mjs";
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
  const detail = JSON.parse(
    await readFile(path.join(root, "tests/details/cases.json"), "utf8"),
  );
  const cashflows = JSON.parse(
    await readFile(path.join(root, "tests/cashflows/cases.json"), "utf8"),
  );
  const cases = [
    ...detail.cases.map((entry) => ({ ...entry, id: `details-${entry.type}` })),
    ...cashflows.cases
      .filter((entry) => entry.type !== "bond")
      .map((entry) => ({ ...entry, id: `cashflows-${entry.type}` })),
  ];
  await writeFile(path.join(consumer, "cases.json"), JSON.stringify(cases));
  let installed = ["pricing-workbench"];
  if (!process.env.REGISTRY_EXPORT_DIR) {
    installed = await installBuilt(root, consumer, ["pricing-workbench"]);
    await writeFile(
      path.join(consumer, "main.tsx"),
      await readFile(
        new URL("./fixture/workbench-phase2.tsx", import.meta.url),
      ),
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
  await page.goto(server.url, { waitUntil: "networkidle" });
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const checks = [];
  function withoutClock(value) {
    if (Array.isArray(value)) return value.map(withoutClock);
    if (value && typeof value === "object" && !("isLosslessNumber" in value))
      return Object.fromEntries(
        Object.entries(value).map(([key, child]) => [
          key,
          key === "meta" && child
            ? withoutClock(
                Object.fromEntries(
                  Object.entries(child).filter(
                    ([name]) => name !== "timestamp",
                  ),
                ),
              )
            : withoutClock(child),
        ]),
      );
    return value;
  }
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  for (const entry of cases) {
    await page.getByRole("button", { name: entry.id, exact: true }).click();
    const results = page.getByRole("region", { name: "Results", exact: true });
    await results.getByText("Complete result JSON", { exact: true }).click();
    const resultText = await results
      .getByRole("region", { name: "Complete valuation result JSON" })
      .locator("pre")
      .textContent();
    await results.getByText("Last priced request", { exact: true }).click();
    const request = JSON.parse(
      await results
        .getByRole("region", { name: "Last priced request JSON" })
        .locator("pre")
        .textContent(),
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
    assert.deepEqual(
      withoutClock(parse(native.validateValuationResultJson(resultText))),
      withoutClock(
        parse(native.validateValuationResultJson(serializeHost(direct))),
      ),
    );
    if (entry.detailType) {
      assert.equal(direct.details.type, entry.detailType);
      if (entry.detailType === "monte_carlo") {
        await results
          .getByRole("region", { name: "Monte Carlo diagnostics" })
          .waitFor();
        assert(resultText.includes(`"seed":${direct.details.data.seed}`));
      } else
        await results
          .getByRole("region", {
            name: {
              composite: "Composite details JSON",
              credit_derivative: "Credit derivative details JSON",
              fx: "FX details JSON",
              structured_credit_stochastic: "Structured credit details JSON",
            }[entry.detailType],
          })
          .waitFor();
    }
    if (entry.id.startsWith("cashflows-")) {
      await results.getByText("Cashflows", { exact: true }).click();
      const viewer = results.getByRole("region", {
        name: "Cashflows",
        exact: true,
      });
      if (entry.error) {
        await viewer.getByRole("alert").waitFor();
        assert.equal(
          await viewer.getByRole("alert").textContent(),
          `Cashflows unavailable: ${entry.error}`,
        );
      } else {
        await page.waitForFunction(
          (text) =>
            document.querySelector('section[aria-label="Cashflows"] pre')
              ?.textContent === text,
          entry.cashflows,
        );
        assert.equal(
          await viewer.locator("pre").textContent(),
          entry.cashflows,
        );
      }
      assert.equal(
        await results
          .getByRole("region", { name: "Complete valuation result JSON" })
          .locator("pre")
          .textContent(),
        resultText,
      );
    }
    const audit = await page.evaluate(() => window.axe.run(document));
    assert.deepEqual(audit.violations, []);
    checks.push({
      id: entry.id,
      details: direct.details?.type,
      fullNativeResultMatch: true,
      cashflow: entry.id.startsWith("cashflows-")
        ? entry.error
          ? "native error; price retained"
          : "exact native mixed-currency text"
        : null,
      violations: audit.violations,
    });
  }
  await page.getByRole("button", { name: "details-bond", exact: true }).click();
  await page.getByRole("region", { name: "Monte Carlo diagnostics" }).waitFor();
  await page.getByRole("tab", { name: "2 Market", exact: true }).click();
  await page.getByRole("tab", { name: "Edit", exact: true }).click();
  await page.getByLabel("Stored value 1", { exact: true }).first().fill("0.96");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.getByRole("tab", { name: "View", exact: true }).click();
  const cell = page.getByRole("gridcell", { name: "0.96", exact: true });
  await cell.click();
  assert.equal(await cell.getAttribute("aria-selected"), "true");
  await page.locator('svg.ts-chart circle[r="9"]').waitFor();
  const selectedY = await page
    .locator('svg.ts-chart circle[r="9"]')
    .getAttribute("cy");
  const chart = page.locator('[aria-label="discount stored curves"][tabindex]');
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  assert.equal(
    await page.locator('table[role="grid"] td[aria-selected="true"]').count(),
    1,
  );
  assert.equal(
    await page
      .locator('table[role="grid"] td[aria-selected="true"]')
      .textContent(),
    "1",
  );
  assert.notEqual(
    await page.locator('svg.ts-chart circle[r="9"]').getAttribute("cy"),
    selectedY,
  );
  const results = page.getByRole("region", { name: "Results", exact: true });
  await results.getByText("Last priced request", { exact: true }).click();
  await page.waitForFunction(() =>
    document
      .querySelector('[aria-label="Last priced request JSON"] pre')
      ?.textContent.includes("0.96"),
  );
  const edited = JSON.parse(
    await results
      .getByRole("region", { name: "Last priced request JSON" })
      .locator("pre")
      .textContent(),
  );
  assert.equal(
    JSON.parse(edited.marketJson).curves.find((curve) => curve.id === "USD-OIS")
      .knot_points[1][1],
    0.96,
  );
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 1);
  const report = {
    browser: browser.version(),
    installed,
    checks,
    marketEdit: true,
    linkedSelection: true,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr026-phase2.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
