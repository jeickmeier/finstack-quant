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
const consumer = await mkdtemp(path.join(root, ".consumer-curves-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, ["curve-chart"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/curves.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "src/fixtures/curves/market.json")),
  );

  const modules = await buildConsumer(consumer, { title: "Stored curves" });
  assert(!modules.some((id) => /finstack-quant-wasm/.test(id)));
  ({ server, browser, page } = await openConsumer(path.join(consumer, "dist"), {
    viewport: { width: 1200, height: 1000 },
  }));
  const failures = [],
    requests = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    requests.push(r.url());
    if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
  await page.waitForFunction(
    () => document.querySelectorAll("svg.ts-chart").length === 7,
  );
  assert(installed.includes("contract-market-context-state"));
  assert(!installed.includes("instrument-catalogue"));
  assert.equal(await page.getByRole("region", { name: / panel$/ }).count(), 9);
  const chart = page.locator('[aria-label="discount stored curves"][tabindex]');
  await chart.focus();
  await chart.press("Home");
  await chart.press("ArrowRight");
  // Locate the original high-precision OVERLAY knot through native keyboard focus.
  for (
    let i = 0;
    i < 30 &&
    !(await page.locator("[data-status]").textContent()).includes(
      "focus: OVERLAY/1;",
    );
    i++
  )
    await chart.press("ArrowRight");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /focus: OVERLAY\/1;/,
  );
  assert(
    (await page.locator(".ts-chart-tooltip").textContent()).includes(
      "0.9512345678901234",
    ),
  );
  await chart.press("Escape");
  await page.getByRole("button", { name: "Toggle custom tooltip" }).click();
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /activations: 1/,
  );
  const action = page.getByRole("button", { name: "Open curve details" });
  await action.waitFor();
  await chart.press("Tab");
  assert(await action.evaluate((node) => node === document.activeElement));
  const svg = await page.evaluate(() => window.exportCurve());
  for (const label of [
    "Stored market observations",
    "Canonical state supplied by native Market",
    "Stored coordinate",
    "Stored value",
    "Supplied knot note",
    "Publication note",
    "Retained native fixture",
    "Discount curve state",
    "OVERLAY",
    "USD-OIS",
  ])
    assert(svg.includes(label), `Export retains ${label}`);
  assert(!svg.includes("Open curve details") && !svg.includes("<image"));
  await writeFile("/tmp/pr014-curves.svg", svg);
  await action.press("Enter");
  assert.match(
    await page.locator("[data-status]").textContent(),
    /activations: 1/,
  );
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
    accessibility.push({
      theme,
      violations: axe.violations.map((v) => ({
        id: v.id,
        nodes: v.nodes.map((n) => n.failureSummary),
      })),
    });
  }
  assert(
    accessibility.every((x) => x.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
  await page.screenshot({ path: "/tmp/pr014-curves.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    accessibility,
    failures,
    panels: 9,
    plottedVariants: 7,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr014-curves.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
