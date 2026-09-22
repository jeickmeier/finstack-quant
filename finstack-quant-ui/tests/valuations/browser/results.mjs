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
const consumer = await mkdtemp(path.join(root, ".consumer-results-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, ["measures-grid"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/results.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "src/fixtures/results/bond.json")),
  );

  const modules = await buildConsumer(consumer, { title: "Valuation results" });
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
  await page.getByText("SUPPLIED-EUR-CONTEXT", { exact: true }).waitFor();
  const firstPanel = await page
    .getByRole("region", { name: "Valuation", exact: true })
    .boundingBox();
  const secondPanel = await page
    .getByRole("region", { name: "Comparison valuation" })
    .boundingBox();
  assert(
    firstPanel.x < secondPanel.x && firstPanel.y === secondPanel.y,
    "Comparison panels are side by side at desktop width",
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = [];
  const heights = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    await page.waitForFunction(
      () =>
        getComputedStyle(document.querySelector("tbody td")).color ===
        getComputedStyle(document.body).color,
    );
    for (const density of ["compact", "comfortable"]) {
      const current = await page
        .locator("[data-density]")
        .first()
        .getAttribute("data-density");
      if (current !== density)
        await page.getByRole("button", { name: "Toggle density" }).click();
      const axe = await page.evaluate(() => window.axe.run(document));
      accessibility.push({
        theme,
        density,
        rowToken: await page
          .locator(".finstack-measures")
          .evaluate((node) =>
            getComputedStyle(node).getPropertyValue("--row-height").trim(),
          ),
        violations: axe.violations.map((v) => ({
          id: v.id,
          nodes: v.nodes.map((n) => n.failureSummary),
        })),
      });
      const cells = await page
        .locator("tbody td:not(:first-child)")
        .evaluateAll((nodes) =>
          nodes.map((node) => ({
            align: getComputedStyle(node).textAlign,
            numeric: getComputedStyle(node).fontVariantNumeric,
          })),
        );
      assert(
        cells.every(
          (cell) =>
            cell.align === "right" && cell.numeric.includes("tabular-nums"),
        ),
      );
      heights.push({
        theme,
        density,
        height: await page
          .locator("tbody tr")
          .first()
          .evaluate((node) => node.getBoundingClientRect().height),
      });
    }
  }
  assert(
    accessibility.every((mode) => mode.violations.length === 0),
    JSON.stringify(accessibility),
  );
  // Stock shadcn tables retain their own cell spacing in both densities.
  assert(
    accessibility.every(
      (mode) =>
        mode.rowToken === (mode.density === "comfortable" ? "36px" : "28px"),
    ),
  );
  assert(
    (
      await page
        .getByRole("region", { name: "Valuation", exact: true })
        .textContent()
    ).includes("USD 1,042,500"),
  );
  assert(
    (
      await page
        .getByRole("region", { name: "Comparison valuation" })
        .textContent()
    ).includes("EUR 987,654.32"),
  );
  assert.equal(
    await page
      .getByText("bucketed_dv01::USD-OIS::30y", { exact: true })
      .count(),
    1,
  );
  assert.equal(
    await page.getByText("Unit unavailable", { exact: true }).count(),
    0,
  );
  assert.equal(
    await page
      .getByText("Raw measure values · units unavailable", { exact: true })
      .count(),
    0,
  );
  const bars = await page.locator("[data-bucket-bar]").evaluateAll((nodes) =>
    nodes.map((track) => {
      const cell = track.parentElement;
      const rect = track.getBoundingClientRect();
      const fill = track
        .querySelector("[data-bucket-bar-fill]")
        ?.getBoundingClientRect();
      return {
        key: cell.parentElement.cells[0].textContent,
        column: cell.cellIndex,
        raw: cell.querySelector(":scope > span[title]").getAttribute("title"),
        hidden: track.getAttribute("aria-hidden"),
        width: rect.width,
        fill: fill
          ? {
              left: (fill.left - rect.left) / rect.width,
              width: fill.width / rect.width,
            }
          : null,
      };
    }),
  );
  const bar = (key, column = 2) =>
    bars.find((item) => item.key === key && item.column === column);
  const near = (actual, expected) =>
    assert(
      Math.abs(actual - expected) < 0.002,
      `${actual} differs from ${expected}`,
    );
  for (const [key, left, width, raw] of [
    ["bucketed_cs01::SYNTHETIC-CREDIT::1y", 0, 0.5, "-100"],
    ["bucketed_cs01::SYNTHETIC-CREDIT::3y", 0.5, 0.25, "50"],
    ["bucketed_cs01::SYNTHETIC-OTHER::1y", 0.5, 0.5, "10"],
    ["bucketed_dv01::USD-OIS::30y", 0.5, 0.5, "2"],
    ["bucketed_dv01::USD-OIS::10y", 0.25, 0.25, "-1"],
  ]) {
    const item = bar(key);
    near(item.fill.left, left);
    near(item.fill.width, width);
    assert.equal(item.raw, raw);
  }
  assert.equal(bar("bucketed_cs01::SYNTHETIC-CREDIT::5y").fill, null);
  near(bar("bucketed_dv01::USD-OIS::30y", 1).fill.width, 0.5);
  assert(bars.every((item) => item.hidden === "true" && item.width > 0));
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  assert(
    !modules.some((id) =>
      /finstack-quant-wasm|@tanstack\/charts|@tanstack\/react-form/.test(id),
    ),
  );
  await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
  await page.waitForFunction(
    () =>
      getComputedStyle(document.querySelector("tbody td")).color ===
      getComputedStyle(document.body).color,
  );
  await page.screenshot({ path: "/tmp/pr013-results.png", fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  const mobileLayout = await page.evaluate(() => ({
    viewport: innerWidth,
    document: document.documentElement.scrollWidth,
    tables: [...document.querySelectorAll('[data-slot="table-container"]')].map(
      (node) => ({ width: node.clientWidth, scroll: node.scrollWidth }),
    ),
  }));
  const mobileAxe = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(mobileAxe.violations, []);
  await page.screenshot({
    path: "/tmp/pr013-results-mobile.png",
    fullPage: true,
  });
  assert(
    mobileLayout.document <= mobileLayout.viewport,
    JSON.stringify(mobileLayout),
  );
  const report = {
    browser: browser.version(),
    installed,
    accessibility,
    heights,
    bars,
    mobileAccessibilityViolations: mobileAxe.violations.length,
    mobileLayout,
    failures,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr013-results.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
