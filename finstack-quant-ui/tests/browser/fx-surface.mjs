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
const consumer = await mkdtemp(path.join(root, ".consumer-fx-surface-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "market-context-form",
    "use-market-validator",
    "fx-surface-chart",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/fx-surface.tsx", import.meta.url)),
  );
  const bond = JSON.parse(
    await readFile(path.join(root, "src/fixtures/results/bond.json"), "utf8"),
  );
  const quotes = {
    id: "EURUSD",
    expiries: [0.5, 1],
    atm_vols: [0.08, 0.09],
    rr_25d: [0.01, 0.012],
    bf_25d: [0.005, 0.006],
    rr_10d: [0.02, 0.025],
    bf_10d: [0.008, 0.009],
  };
  await writeFile(
    path.join(consumer, "fixture.json"),
    JSON.stringify({
      market: {
        ...JSON.parse(bond.request.marketJson),
        fx_delta_vol_surfaces: [quotes],
      },
    }),
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
  await page.addInitScript(() => {
    window.fxCalls = 0;
    const post = Worker.prototype.postMessage;
    Worker.prototype.postMessage = function (message, ...args) {
      if (message?.path?.[0] === "sampleFxDelta") window.fxCalls++;
      return post.call(this, message, ...args);
    };
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
  await page.getByText(/Supply an explicit forward/).waitFor();
  const calls = () => page.evaluate(() => window.fxCalls);
  assert.equal(await calls(), 0);
  await page.getByLabel("Explicit forward", { exact: true }).fill("1.1");
  const view = page.getByRole("region", {
    name: "EURUSD evaluated surface",
    exact: true,
  });
  await view.waitFor();
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  async function compare() {
    const { market, coordinates } = await page.evaluate(() => ({
      market: window.fxSurfaceProbe.market,
      coordinates: window.fxSurfaceProbe.coordinates,
    }));
    const q = market.fx_delta_vol_surfaces[0],
      surface = new native.FxDeltaVolSurface(
        q.id,
        q.expiries,
        q.atm_vols,
        q.rr_25d,
        q.bf_25d,
        q.rr_10d ?? undefined,
        q.bf_10d ?? undefined,
      );
    let values;
    try {
      values = coordinates.map((c) =>
        native.getFxDeltaVol(surface, c.expiry, c.strike, c.forward),
      );
    } finally {
      surface.free();
    }
    await page.waitForFunction((values) => {
      const rows = [
        ...document.querySelectorAll(
          '[aria-label="EURUSD evaluated surface"] tbody tr',
        ),
      ];
      return (
        rows.length === values.length &&
        rows.every(
          (row, i) =>
            row.querySelectorAll("td")[2].textContent === String(values[i]),
        )
      );
    }, values);
    return values;
  }
  const before = await compare();
  await view
    .getByRole("gridcell", { name: String(before[2]), exact: true })
    .click();
  assert.equal(
    await page.getByLabel("Accepted sample").textContent(),
    JSON.stringify(["EURUSD", 0.5, 1.2]),
  );
  const chart = page.locator('[aria-label="EURUSD heatmap"][tabindex]');
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Accepted sample"]').textContent ===
      JSON.stringify(["EURUSD", 0.5, 1]),
  );
  await page.getByText(/Explicit forward 1.1/).waitFor();
  const svg = await page.evaluate(() => window.fxSurfaceProbe.export());
  assert(
    svg.includes("Native evaluated samples") &&
      svg.includes("Explicit caller forwards") &&
      !svg.includes("Explicit forward 1.1"),
  );
  await writeFile("/tmp/pr029-evaluated.svg", svg);
  await chart.press("Escape");
  await page.getByLabel("Explicit forward", { exact: true }).fill("1.12");
  const changedForward = await compare();
  assert.notDeepEqual(changedForward, before);
  await page.getByRole("button", { name: /Edit .*EURUSD/ }).click();
  await page
    .locator('[data-field-path="fx_delta_vol_surfaces[0].atm_vols[1]"] input')
    .fill("0.095");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.waitForFunction(
    () =>
      window.fxSurfaceProbe.market.fx_delta_vol_surfaces[0].atm_vols[1] ===
      0.095,
  );
  const changedQuote = await compare();
  assert.notDeepEqual(changedQuote, changedForward);
  await page.getByLabel("Explicit forward", { exact: true }).fill("-1");
  await page
    .getByRole("alert")
    .filter({ hasText: "Values must be positive" })
    .waitFor();
  assert.equal(await view.count(), 0);
  const beforeMissing = await calls();
  await page.getByLabel("Explicit forward", { exact: true }).fill("");
  await page.getByText(/Supply an explicit forward/).waitFor();
  assert.equal(await calls(), beforeMissing);
  await page.getByLabel("Explicit forward", { exact: true }).fill("1.12");
  await compare();
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
      .getByRole("region", { name: "EURUSD FX surface", exact: true })
      .screenshot({ path: `/tmp/pr029-${theme}.png` });
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
    before,
    changedForward,
    changedQuote,
    sampleCalls: await calls(),
    accessibility,
    wasmRequests: 1,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr029-fx-surface.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
