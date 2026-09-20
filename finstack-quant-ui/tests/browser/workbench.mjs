import { fxQuotes } from "../surfaces/fixtures.ts";
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
async function original(viewer) {
  await viewer.locator("pre").waitFor();
  await viewer.getByRole("button", { name: "Original", exact: true }).click();
}

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
  page.on("pageerror", (error) => {
    failures.push(error.message);
    console.error(error);
  });
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
  await results.getByRole("tab", { name: "Request", exact: true }).click();
  await original(
    results.getByRole("region", { name: "Last priced request JSON" }),
  );
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
  await results
    .getByRole("tab", { name: "4 Diagnostics", exact: true })
    .click();
  await results.getByText("Complete result JSON", { exact: true }).click();
  await original(
    results.getByRole("region", { name: "Complete valuation result JSON" }),
  );
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
  await page.getByLabel("Search market fields").fill("/curves/0");
  await page
    .getByRole("button", { name: "Inspect /curves/0", exact: true })
    .click();
  await page.locator("svg.ts-chart").first().waitFor();
  await page.getByRole("tab", { name: "Snapshot JSON", exact: true }).click();
  assert(
    await page.getByRole("region", { name: "Market snapshot" }).isVisible(),
  );
  await page.getByRole("tab", { name: "View market", exact: true }).click();
  await page.getByRole("tab", { name: "1 Instrument" }).click();
  assert.equal(await amount.inputValue(), "1000000.123456789");
  const calibrationChecks = [];
  if (process.env.REGISTRY_PHASE3 === "1") {
    const cases = JSON.parse(
      await readFile(path.join(root, "tests/calibration/cases.json"), "utf8"),
    );
    await page.getByRole("tab", { name: "Calibrate" }).click();
    const panel = page.getByRole("region", { name: "Calibration workflow" });
    await panel
      .getByText("Import calibration envelope", { exact: true })
      .click();
    for (const c of cases) {
      await panel
        .getByLabel("Calibration envelope JSON", { exact: true })
        .fill(JSON.stringify(c.input));
      await panel
        .getByRole("button", { name: "Load envelope", exact: true })
        .click();
      await panel
        .getByRole("region", { name: "Static diagnostics", exact: true })
        .waitFor();
      await page.waitForFunction(
        () =>
          !document.querySelector(
            '[aria-label="Calibration workflow"] button[type="submit"]',
          )?.disabled,
      );
      await panel
        .getByRole("button", { name: "Calibrate", exact: true })
        .click();
      const output = panel.getByRole("region", {
        name: "Native calibration result",
        exact: true,
      });
      await output.waitFor({ timeout: 60000 });
      const value = JSON.parse(await output.locator("pre").textContent());
      assert.deepEqual(
        value,
        native.calibrate(JSON.stringify(c.input)),
        c.source,
      );
      const step = Object.keys(value.result.step_reports)[0];
      if (step) {
        await panel
          .getByLabel("Calibration report", { exact: true })
          .selectOption(`step/${step}`);
        await panel
          .getByRole("region", {
            name: `${step} calibration report`,
            exact: true,
          })
          .waitFor();
      }
      calibrationChecks.push(c.source);
      console.log(`Verified calibration ${c.source}`);
      if (c.source.includes("01_usd_discount")) {
        await panel
          .getByRole("button", { name: "Use calibrated market", exact: true })
          .click();
        const marketHandle = new native.Market(
          JSON.stringify(value.result.final_market),
        );
        const expectedMarket = marketHandle.toJson();
        marketHandle.free();
        await results
          .getByRole("tab", { name: "Request", exact: true })
          .click();
        await original(
          results.getByRole("region", { name: "Last priced request JSON" }),
        );
        await page.waitForFunction(
          (expected) =>
            JSON.parse(
              document.querySelector(
                '[aria-label="Last priced request JSON"] pre',
              ).textContent,
            ).marketJson === expected,
          expectedMarket,
        );
        const priced = JSON.parse(
          await results
            .getByRole("region", { name: "Last priced request JSON" })
            .locator("pre")
            .textContent(),
        );
        const directPrice = native.priceInstrument(
          priced.instrumentJson,
          priced.marketJson,
          priced.asOf,
          priced.model,
          priced.metrics,
          priced.pricingOptions,
          priced.marketHistory,
        );
        await results
          .getByRole("tab", { name: "4 Diagnostics", exact: true })
          .click();
        await results
          .getByText("Complete result JSON", { exact: true })
          .click();
        await original(
          results.getByRole("region", {
            name: "Complete valuation result JSON",
          }),
        );
        const actualPrice = JSON.parse(
          await results
            .getByRole("region", { name: "Complete valuation result JSON" })
            .locator("pre")
            .textContent(),
        );
        directPrice.meta.timestamp = actualPrice.meta.timestamp;
        assert.deepEqual(actualPrice, directPrice);
        const rate = panel.locator(
          '[data-field-path="market_data[0].rate"] input',
        );
        await rate.fill("0.0527");
        assert.equal(
          await panel
            .getByRole("button", { name: "Use calibrated market", exact: true })
            .isDisabled(),
          true,
        );
        await page.waitForTimeout(500);
        assert.equal(
          await panel
            .getByRole("button", { name: "Use calibrated market", exact: true })
            .isDisabled(),
          true,
        );
      }
    }
    // Evaluate supplied native market objects through the block's public view options.
    for (const source of ["07_swaption", "08_equity"]) {
      const c = cases.find((item) => item.source.includes(source));
      if (!c) throw new Error(`Missing calibration source ${source}`);
      await panel
        .getByLabel("Calibration envelope JSON", { exact: true })
        .fill(JSON.stringify(c.input));
      await panel
        .getByRole("button", { name: "Load envelope", exact: true })
        .click();
      await panel
        .getByRole("region", { name: "Static diagnostics", exact: true })
        .waitFor();
      await page.waitForFunction(
        () =>
          !document.querySelector(
            '[aria-label="Calibration workflow"] button[type="submit"]',
          )?.disabled,
      );
      await panel
        .getByRole("button", { name: "Calibrate", exact: true })
        .click();
      await panel
        .getByRole("region", { name: "Native calibration result", exact: true })
        .waitFor();
      await panel
        .getByRole("button", { name: "Use calibrated market", exact: true })
        .click();
      await page.getByRole("tab", { name: "2 Market" }).click();
      const field = source.includes("swaption")
        ? "vol_cubes"
        : source.includes("fx")
          ? "fx_delta_vol_surfaces"
          : "surfaces";
      await page.getByLabel("Search market fields").fill(`/${field}/0`);
      await page
        .getByRole("button", { name: `Inspect /${field}/0`, exact: true })
        .click();
      await page
        .getByRole("region", { name: "Selected market field" })
        .locator("svg.ts-chart")
        .first()
        .waitFor();
      assert.equal(
        await page
          .getByRole("region", { name: "Selected market field" })
          .getByRole("alert")
          .count(),
        0,
      );
      await page.getByRole("tab", { name: "Calibrate" }).click();
    }
    const supplemental = JSON.parse(
      await readFile(
        path.join(root, "tests/market-browser/cases.json"),
        "utf8",
      ),
    ).supplemental;
    supplemental.fx_delta_vol_surfaces = [fxQuotes];
    await page.getByRole("tab", { name: "2 Market" }).click();
    await page.getByRole("tab", { name: "Edit market", exact: true }).click();
    const editor = page.getByRole("region", { name: "Market context form" });
    await editor.getByText("Import market", { exact: true }).click();
    await editor
      .getByLabel("Market or calibration result JSON", { exact: true })
      .fill(JSON.stringify(supplemental));
    await editor
      .getByRole("button", { name: "Import JSON", exact: true })
      .click();
    await page.getByRole("tab", { name: "View market", exact: true }).click();
    await page
      .getByLabel("Search market fields")
      .fill("/fx_delta_vol_surfaces/0");
    await page
      .getByRole("button", {
        name: "Inspect /fx_delta_vol_surfaces/0",
        exact: true,
      })
      .click();
    await page
      .getByRole("region", { name: "Selected market field" })
      .locator("svg.ts-chart")
      .first()
      .waitFor();
    assert.equal(
      await page
        .getByRole("region", { name: "Selected market field" })
        .getByRole("alert")
        .count(),
      0,
    );
    await page.getByRole("tab", { name: "Calibrate" }).click();
    const missing = structuredClone(cases[0].input);
    missing.plan.steps[0].quote_set = "missing_quotes";
    await panel
      .getByLabel("Calibration envelope JSON", { exact: true })
      .fill(JSON.stringify(missing));
    await panel
      .getByRole("button", { name: "Load envelope", exact: true })
      .click();
    const diagnostics = panel.getByRole("region", {
      name: "Static diagnostics",
      exact: true,
    });
    await diagnostics.waitFor();
    assert(
      JSON.parse(await diagnostics.locator("pre").textContent()).errors.length >
        0,
    );
    await panel.getByRole("button", { name: "Calibrate", exact: true }).click();
    assert.equal(
      await panel
        .getByRole("region", { name: "Native calibration result", exact: true })
        .count(),
      0,
    );
    const solver = structuredClone(
      cases.find((c) => c.source.includes("08_equity")).input,
    );
    solver.plan.steps[1].target_strikes = [140, 180, 220];
    solver.plan.settings.fail_on_bad_fit = true;
    solver.plan.settings.vol_surface = { validation_tolerance: 0.001 };
    await panel
      .getByLabel("Calibration envelope JSON", { exact: true })
      .fill(JSON.stringify(solver));
    await panel
      .getByRole("button", { name: "Load envelope", exact: true })
      .click();
    await panel
      .getByRole("region", { name: "Static diagnostics", exact: true })
      .waitFor();
    await page.waitForFunction(
      () =>
        !document.querySelector(
          '[aria-label="Calibration workflow"] button[type="submit"]',
        )?.disabled,
    );
    await panel.getByRole("button", { name: "Calibrate", exact: true }).click();
    const failure = panel.getByRole("region", {
      name: "Failed step structured failure",
      exact: true,
    });
    await failure.waitFor();
    const expectedFailure = JSON.parse(
      await readFile(path.join(root, "tests/calibration/failure.json"), "utf8"),
    );
    assert.deepEqual(
      JSON.parse(await failure.locator("pre").textContent()),
      expectedFailure,
    );
    await page.getByRole("tab", { name: "1 Instrument" }).click();
  }
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
    calibrationChecks,
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
