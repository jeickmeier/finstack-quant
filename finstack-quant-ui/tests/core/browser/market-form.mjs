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
const consumer = await mkdtemp(path.join(root, ".consumer-market-form-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "market-context-form",
    "use-market-validator",
    "curve-chart",
    "use-price-instrument",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/market-form.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "src/fixtures/results/bond.json")),
  );

  const modules = await buildConsumer(consumer, { title: "Cashflow exports" });
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
  const fixture = JSON.parse(
    await readFile(path.join(root, "src/fixtures/results/bond.json"), "utf8"),
  );
  const native = createRequire(import.meta.url)(
    "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const marketView = page.getByRole("region", { name: "Applied market JSON" });
  const resultView = page.getByRole("region", {
    name: "Native valuation JSON",
  });
  await resultView.waitFor();
  const before = JSON.parse(await resultView.locator("pre").textContent());
  await page.getByLabel("Stored value 1", { exact: true }).first().fill("0.96");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.waitForFunction(() => {
    const text = document.querySelector(
      '[aria-label="Applied market JSON"] pre',
    )?.textContent;
    return (
      text &&
      JSON.parse(text).curves.find((c) => c.id === "USD-OIS")
        .knot_points[1][1] === 0.96
    );
  });
  await marketView
    .getByRole("button", { name: "Original", exact: true })
    .click();
  const marketJson = await marketView.locator("pre").textContent();
  const handle = new native.Market(marketJson);
  try {
    assert.equal(handle.toJson(), marketJson);
  } finally {
    handle.free();
  }
  const r = fixture.request;
  const direct = native.priceInstrument(
    r.instrumentJson,
    marketJson,
    r.asOf,
    r.model,
    r.metrics,
  );
  await page.waitForFunction((amount) => {
    const text = document.querySelector(
      '[aria-label="Native valuation JSON"] pre',
    )?.textContent;
    return text && JSON.parse(text).value.amount === amount;
  }, direct.value.amount);
  const result = JSON.parse(await resultView.locator("pre").textContent());
  direct.meta.timestamp = result.meta.timestamp;
  assert.deepEqual(result, direct);
  assert.notEqual(result.value.amount, before.value.amount);
  await marketView.getByRole("button", { name: "Copy", exact: true }).click();
  await marketView.getByText("Copied", { exact: true }).waitFor();
  assert.equal(
    await page.evaluate(() => navigator.clipboard.readText()),
    marketJson,
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const audit = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(audit.violations, []);
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 1);
  const report = {
    browser: browser.version(),
    installed,
    fullMarketCanonical: true,
    editedKnot: [40, 0.96],
    directPricingMatch: true,
    exactCopy: true,
    violations: audit.violations,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr024-market-form.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
