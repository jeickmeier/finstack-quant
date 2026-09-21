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
const consumer = await mkdtemp(path.join(root, ".consumer-cashflows-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "cashflow-viewer",
    "use-price-instrument",
    "use-cashflows",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/cashflows.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/cashflows/cases.json")),
  );

  const modules = await buildConsumer(consumer, { title: "Cashflow exports" });
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
      await viewer.getByRole("table", { name: "Cashflow schedule" }).waitFor();
      assert.equal(
        await viewer.locator("tbody tr").count(),
        JSON.parse(entry.cashflows).flows.length,
      );
      if ((await viewer.locator("details").getAttribute("open")) === null)
        await viewer.getByText("Original JSON", { exact: true }).click();
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
      assert.equal(
        await viewer
          .getByRole("table", { name: "Cashflow schedule" })
          .isVisible(),
        true,
      );
      assert.equal(
        await viewer.locator("[data-json-print-source]").isVisible(),
        false,
      );
      assert.equal(await viewer.locator("pre").isVisible(), false);
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
      await page.setViewportSize({ width: 390, height: 844 });
      const table = viewer.getByRole("table", { name: "Cashflow schedule" });
      const scroller = viewer.locator('[data-slot="table-container"]');
      assert.equal(
        await scroller.evaluate((node) => node.scrollWidth > node.clientWidth),
        true,
      );
      await table.focus();
      assert.equal(
        await table.evaluate((node) => document.activeElement === node),
        true,
      );
      const scrollBefore = await scroller.evaluate((node) => node.scrollLeft);
      await page.keyboard.press("ArrowRight");
      await page.waitForFunction(
        (before) =>
          document.querySelector(
            '[aria-label="Cashflows"] [data-slot="table-container"]',
          ).scrollLeft > before,
        scrollBefore,
      );
      for (const viewport of [
        { width: 390, height: 844 },
        { width: 1440, height: 1700 },
      ]) {
        await page.setViewportSize(viewport);
        const audit = await page.evaluate(() => window.axe.run(document));
        assert.deepEqual(audit.violations, []);
        accessibility.push({
          type: entry.type,
          viewport,
          keyboardScroll: true,
          violations: audit.violations,
        });
      }
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
      if ((await viewer.locator("details").getAttribute("open")) === null)
        await viewer.getByText("Original JSON", { exact: true }).click();
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
    print: "Native cashflow table visible; raw source and toolbar hidden",
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr022-cashflows.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
