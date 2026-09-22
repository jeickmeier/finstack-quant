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
} from "../../browser/consumer.mjs";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-pricing-forms-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "pricing-forms-example",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/pricing-forms.tsx", import.meta.url)),
  );

  const modules = await buildConsumer(consumer, {
    title: "Standalone pricing forms",
  });
  assert(
    !modules.some((id) =>
      /tanstack\/(?:react-)?table|finstack-quant-wasm/.test(id),
    ),
  );
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
  await page.goto(server.url, { waitUntil: "networkidle" });
  const price = page.getByRole("button", { name: "Price supplied request" });
  await price.waitFor();
  await page.waitForFunction(() =>
    [...document.querySelectorAll("button")].some(
      (b) => b.textContent === "Price supplied request" && !b.disabled,
    ),
  );
  const amount = page.getByRole("textbox", { name: "Amount", exact: true });
  await amount.fill("abc");
  const issue = page.getByRole("button", {
    name: "instrument.spec.notional.amount",
  });
  await issue.waitFor();
  await issue.click();
  assert(await amount.evaluate((node) => node === document.activeElement));
  assert(await price.isDisabled());
  await amount.fill("1000000.123456789");
  await page.getByRole("button", { name: "Add theta period" }).click();
  await page
    .getByRole("textbox", { name: "Theta period", exact: true })
    .fill("1W");
  const history = JSON.stringify({
    base_date: "2025-01-01",
    window_days: 2,
    scenarios: [
      {
        date: "2024-12-30",
        shifts: [
          {
            factor: {
              type: "discount_rate",
              curve_id: "USD-OIS",
              tenor_years: 1,
            },
            shift: 0.001,
          },
        ],
      },
      {
        date: "2024-12-31",
        shifts: [
          {
            factor: {
              type: "discount_rate",
              curve_id: "USD-OIS",
              tenor_years: 1,
            },
            shift: -0.0005,
          },
        ],
      },
    ],
  });
  await page
    .getByRole("textbox", { name: "Market history", exact: true })
    .fill(history);
  const metrics = page.getByRole("button", { name: "Metrics" });
  await metrics.click();
  await page.getByRole("checkbox", { name: "hvar", exact: true }).check();
  await metrics.click();
  await page.waitForTimeout(400);
  await price.click();
  const output = page
    .getByRole("region", { name: "Pricing result" })
    .locator("pre");
  await output.waitFor();
  const submitted = JSON.parse(
    await page
      .getByRole("region", { name: "Submitted request" })
      .locator("pre")
      .textContent(),
  );
  assert.equal(submitted.pricingOptions, '{"theta_period":"1W"}');
  assert.equal(submitted.marketHistory, history);
  assert.equal(
    JSON.parse(submitted.instrumentJson).instrument.spec.notional.amount,
    "1000000.123456789",
  );
  assert.deepEqual(submitted.metrics, ["hvar"]);
  const native = createRequire(import.meta.url)(
    "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const direct = native.priceInstrument(
    submitted.instrumentJson,
    submitted.marketJson,
    submitted.asOf,
    submitted.model,
    submitted.metrics,
    submitted.pricingOptions,
    submitted.marketHistory,
  );
  const actual = JSON.parse(await output.textContent());
  direct.meta.timestamp = actual.meta.timestamp;
  assert.deepEqual(actual, direct);
  const theta = page.getByRole("textbox", {
    name: "Theta period",
    exact: true,
  });
  await theta.fill("nope");
  await page.waitForTimeout(400);
  await price.click();
  await page
    .getByRole("region", { name: "Pricing result" })
    .getByRole("alert")
    .waitFor();
  assert.equal(
    JSON.parse(
      await page
        .getByRole("region", { name: "Submitted request" })
        .locator("pre")
        .textContent(),
    ).pricingOptions,
    '{"theta_period":"nope"}',
  );
  assert(
    (
      await page
        .getByRole("region", { name: "Pricing result" })
        .getByRole("alert")
        .textContent()
    ).includes("Invalid input data"),
  );
  await page.getByRole("button", { name: "Omit pricing overrides" }).click();
  await page.getByRole("button", { name: "Load example" }).click();
  assert.notEqual(await amount.inputValue(), "1000000.123456789");
  await page.getByRole("combobox", { name: "Instrument type" }).click();
  await page.getByRole("option", { name: "equity", exact: true }).click();
  await page.getByRole("textbox", { name: "Ticker", exact: true }).waitFor();
  assert(await price.isDisabled());
  await page.getByRole("combobox", { name: "Instrument type" }).click();
  await page.getByRole("option", { name: "bond", exact: true }).click();
  await amount.waitFor();
  await page.locator('[data-slot="combobox-list"]').waitFor({
    state: "hidden",
  });
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
  assert(requests.some((url) => /equity-[^/]+\.js$/.test(url)));
  await page.screenshot({ path: "/tmp/pr015-forms.png" });
  const report = {
    browser: browser.version(),
    installed,
    violations: [],
    failures,
    checks: [
      "lazy bond and full catalogue",
      "canonical theta override and exact history",
      "full request matches native hvar",
      "invalid theta period is rejected",
      "invalid path focus",
      "example replacement",
    ],
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr015-forms.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
