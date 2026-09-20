import { createRequire } from "node:module";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import {
  installBuilt,
  buildConsumer,
  openConsumer,
  closeConsumer,
} from "./consumer.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-calibration-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "calibration-form",
    "use-calibrate",
    "use-price-instrument",
    "market-context-browser",
    "calibration-report",
    "calibration-fit-chart",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/calibration.tsx", import.meta.url)),
  );
  const source = async (name) =>
    JSON.parse(
      await readFile(
        path.join(
          root,
          "../finstack-quant/calibration/examples/market_bootstrap",
          name + ".json",
        ),
        "utf8",
      ),
    );
  const input = await source("01_usd_discount"),
    equity = await source("08_equity_vol_surface");
  input.prior_market = (await source("05_cdx_base_correlation")).prior_market;
  input.plan.settings.compute_diagnostics = false;
  const missing = structuredClone(input);
  missing.plan.steps[0].quote_set = "missing_quotes";
  const target = structuredClone(equity);
  target.plan.settings.fail_on_bad_fit = true;
  target.plan.settings.vol_surface = { validation_tolerance: 1e-4 };
  const solver = structuredClone(equity);
  solver.plan.steps[1].target_strikes = [140, 180, 220];
  solver.plan.settings.fail_on_bad_fit = true;
  solver.plan.settings.vol_surface = { validation_tolerance: 0.001 };
  const bond = JSON.parse(
    await readFile(path.join(root, "src/fixtures/results/bond.json"), "utf8"),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    JSON.stringify({ input, missing, target, solver, bond }),
  );

  const modules = await buildConsumer(consumer, { title: "Market browser" });
  assert(!modules.some((id) => /finstack-quant-wasm/.test(id)));
  ({ server, browser, page } = await openConsumer(path.join(consumer, "dist"), {
    viewport: { width: 1000, height: 1000 },
  }));
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
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  await page
    .getByRole("button", { name: "Run diagnostics", exact: true })
    .click();
  await page.waitForFunction(() => window.calibrationProbe.diagnostics);
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => window.calibrationProbe.diagnostics)),
    JSON.parse(native.dryRun(JSON.stringify(input))),
  );
  await page.getByRole("button", { name: "Calibrate", exact: true }).click();
  await page.waitForFunction(() => window.calibrationProbe.result);
  async function compare() {
    const probe = await page.evaluate(() => window.calibrationProbe);
    assert.deepEqual(probe.result, native.calibrate(probe.request));
  }
  await compare();
  for (const [field, value] of [
    ["market_data[0].rate", "0.0527"],
    ["prior_market[0].knot_points[1][1]", "0.0013"],
  ])
    await page.locator(`[data-field-path="${field}"] input`).fill(value);
  await page
    .locator(
      '[data-field-path="plan.settings.compute_diagnostics"] input[type="checkbox"]',
    )
    .check();
  const original = await page.evaluate(() => window.calibrationProbe.request);
  await page.getByRole("button", { name: "Calibrate", exact: true }).click();
  await page.waitForFunction(
    (previous) =>
      window.calibrationProbe.request !== previous &&
      window.calibrationProbe.result,
    original,
  );
  await compare();
  await page
    .getByRole("button", { name: "Price returned market", exact: true })
    .click();
  await page.waitForFunction(() => window.calibrationProbe.price);
  const probe = await page.evaluate(() => window.calibrationProbe);
  const expected = native.priceInstrument(
    bond.request.instrumentJson,
    JSON.stringify(probe.result.result.final_market),
    bond.request.asOf,
    bond.request.model,
    bond.request.metrics,
  );
  expected.meta.timestamp = probe.price.meta.timestamp;
  assert.deepEqual(probe.price, expected);
  await page
    .getByRole("button", { name: "Invalid plan diagnostics", exact: true })
    .click();
  await page.waitForFunction(
    () => JSON.parse(window.calibrationProbe.diagnostics).errors.length > 0,
  );
  assert.equal(
    await page.evaluate(() => window.calibrationProbe.diagnostics),
    native.dryRun(JSON.stringify(missing)),
  );
  for (const [name, envelope] of [
    ["Target failure", target],
    ["Solver failure", solver],
  ]) {
    let direct;
    try {
      native.calibrate(JSON.stringify(envelope));
    } catch (error) {
      direct = Object.fromEntries(
        [
          "name",
          "message",
          "kind",
          "stage",
          "step_id",
          "solver_diagnostics",
          "details",
          "cause",
        ]
          .filter((key) => error[key] !== undefined)
          .map((key) => [key, error[key]]),
      );
    }
    await page.getByRole("button", { name, exact: true }).click();
    await page.waitForFunction(
      (stage) => window.calibrationProbe.error?.stage === stage,
      direct.stage,
    );
    assert.deepEqual(
      await page.evaluate(() => window.calibrationProbe.error),
      direct,
    );
    if (name === "Target failure")
      await page
        .getByText("Solver diagnostics unavailable", { exact: true })
        .waitFor();
  }
  // Restore the reviewed accepted request for the final accessible stored browser view.
  await page.getByRole("button", { name: "Calibrate", exact: true }).click();
  await page.waitForFunction(
    () => !!window.calibrationProbe.result && !window.calibrationProbe.error,
  );
  await page
    .getByLabel("Search market fields", { exact: true })
    .fill("USD-OIS");
  const firstQuote = Object.keys(
    (await page.evaluate(() => window.calibrationProbe.result)).result
      .step_reports["USD-OIS"].residuals,
  )[0];
  const table = page.getByRole("grid", {
    name: "USD-OIS returned residuals",
    exact: true,
  });
  await table.getByText(firstQuote, { exact: true }).click();
  await page.waitForFunction(
    (key) =>
      window.calibrationProbe.selectedKey === JSON.stringify(["USD-OIS", key]),
    firstQuote,
  );
  const chart = page.locator(
    '[aria-label="USD-OIS rate_quote / swap residuals"][tabindex]',
  );
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  await page
    .getByText(`Transient residual ${firstQuote}`, { exact: true })
    .waitFor();
  assert.equal(
    (await page.evaluate(() => window.calibrationProbe.activated)).key,
    JSON.stringify(["USD-OIS", firstQuote]),
  );
  const svg = await page.evaluate(() => window.calibrationProbe.export());
  for (const text of [
    "Native residual figure",
    "Signed returned solver values",
    "Canonical calibration report",
    "Residuals, not repriced quotes",
    "Solver units (quote convention unavailable)",
    firstQuote,
  ])
    assert(svg.includes(text), text);
  assert(!svg.includes("Transient residual"));
  await writeFile("/tmp/pr033-residual.svg", svg);
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
    accessibility.push({ theme, violations: axe.violations });
    await page
      .getByRole("region", {
        name: "USD-OIS calibration residuals",
        exact: true,
      })
      .screenshot({ path: `/tmp/pr033-${theme}.png` });
  }
  assert(
    accessibility.every((a) => a.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 1);
  const report = {
    browser: browser.version(),
    installed,
    nativeCases: [
      "initial",
      "edited",
      "static-error",
      "target-error",
      "solver-error",
    ],
    accessibility,
    wasmRequests: 1,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr033-calibration.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
