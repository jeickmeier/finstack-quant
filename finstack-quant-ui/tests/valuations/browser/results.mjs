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
  assert.equal(
    await page
      .getByRole("button", { name: "Compact" })
      .getAttribute("aria-pressed"),
    "true",
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
        await page
          .getByRole("button", {
            name: density === "compact" ? "Compact" : "Comfortable",
          })
          .click();
      assert.equal(
        await page
          .getByRole("button", {
            name: density === "compact" ? "Compact" : "Comfortable",
          })
          .getAttribute("aria-pressed"),
        "true",
      );
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
  assert(
    (await page.getByRole("note").textContent()).includes(
      "Different currencies and valuation dates",
    ),
  );
  assert.equal(
    await page.getByText("Yield to maturity", { exact: true }).count(),
    1,
  );
  assert.equal(
    await page.getByText("Modified duration", { exact: true }).count(),
    1,
  );
  assert.equal(
    await page
      .getByText("bucketed_dv01::USD-OIS::30y", { exact: true })
      .count(),
    0,
  );
  assert.equal(
    await page.getByText("Unit unavailable", { exact: true }).count(),
    0,
  );
  const dv01Toggle = page.getByRole("button", {
    name: /^Bucketed DV01 · USD-OIS/,
  });
  assert.equal(await dv01Toggle.getAttribute("aria-expanded"), "false");
  await dv01Toggle.click();
  assert.equal(await dv01Toggle.getAttribute("aria-expanded"), "true");
  const dv01Table = page.getByRole("table", {
    name: "Sensitivity: Bucketed DV01 · USD-OIS",
  });
  const labels = () =>
    dv01Table.locator("tbody tr td:first-child").allTextContents();
  assert.deepEqual(await labels(), [
    "3m",
    "6m",
    "1y",
    "2y",
    "3y",
    "5y",
    "7y",
    "10y",
    "15y",
    "20y",
    "30y",
  ]);
  assert.equal(await page.locator("[data-bucket-bar]").count(), 0);

  await page.getByRole("button", { name: "Nonzero only" }).click();
  assert.equal(
    await page
      .getByRole("button", { name: "Nonzero only" })
      .getAttribute("aria-pressed"),
    "true",
  );
  const filteredTenors = await labels();
  assert(
    !filteredTenors.includes("15y"),
    "Both supplied zero values are hidden",
  );
  assert(
    filteredTenors.includes("3m"),
    "Zero with a missing comparison remains visible",
  );
  assert(
    filteredTenors.includes("10y"),
    "Zero with a nonzero comparison remains visible",
  );
  const creditTable = page.getByRole("table", {
    name: "Credit: Bucketed CS01 · SYNTHETIC-CREDIT",
  });
  const creditRows = await creditTable
    .locator("tbody tr")
    .evaluateAll((rows) =>
      rows.map((row) => [...row.cells].map((cell) => cell.textContent.trim())),
    );
  const creditFiveYear = creditRows.find((cells) => cells[0] === "5y");
  assert(
    creditFiveYear,
    "Missing valuation with a zero comparison remains visible",
  );
  assert.equal(creditFiveYear[1], "—");
  assert(creditFiveYear[2].startsWith("0"));
  await page.getByRole("button", { name: "Show all" }).click();
  assert((await labels()).includes("15y"));

  await page.getByRole("button", { name: "Exact keys" }).click();
  assert.equal(
    await page
      .getByRole("button", { name: "Exact keys" })
      .getAttribute("aria-pressed"),
    "true",
  );
  assert.equal(
    await page
      .getByText("bucketed_dv01::USD-OIS::30y", { exact: true })
      .count(),
    1,
  );
  await page.getByRole("button", { name: "Exact values" }).click();
  assert.equal(
    await page
      .getByRole("button", { name: "Exact values" })
      .getAttribute("aria-pressed"),
    "true",
  );
  const thirtyYearCells = await dv01Table
    .locator("tbody tr")
    .filter({
      has: page.getByText("bucketed_dv01::USD-OIS::30y", { exact: true }),
    })
    .locator("td")
    .allTextContents();
  assert(thirtyYearCells[1].includes("-103.45856181590352"));
  assert(thirtyYearCells[2].startsWith("2"));
  await page.getByRole("button", { name: "Exact keys" }).click();
  await page.getByRole("button", { name: "Exact values" }).click();
  await dv01Toggle.click();
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
  const printSectionId = await dv01Toggle.getAttribute("aria-controls");
  await page.emulateMedia({ media: "print" });
  const printSection = page.locator(`#${printSectionId}`);
  const printSectionStyle = await printSection.evaluate((node) => ({
    hidden: node.hidden,
    display: getComputedStyle(node).display,
  }));
  assert(
    await printSection.isVisible(),
    `Collapsed buckets remain visible in print: ${JSON.stringify(printSectionStyle)}`,
  );
  const printedThirtyYear = printSection.locator("tbody tr").filter({
    has: page.locator('[title="bucketed_dv01::USD-OIS::30y"]'),
  });
  const printValue = printedThirtyYear.locator(
    'td:nth-child(2) span[aria-hidden="true"]',
  );
  assert.equal(await printValue.textContent(), "-103.45856181590352");
  assert(await printValue.isVisible(), "Exact raw value is visible in print");
  assert.equal(
    await printedThirtyYear
      .locator('td:nth-child(2) span[class~="print:hidden"]')
      .isVisible(),
    false,
    "Rounded screen value is hidden in print",
  );
  await page.emulateMedia({ media: "screen" });
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
    readableBucketOrder: [
      "3m",
      "6m",
      "1y",
      "2y",
      "3y",
      "5y",
      "7y",
      "10y",
      "15y",
      "20y",
      "30y",
    ],
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
