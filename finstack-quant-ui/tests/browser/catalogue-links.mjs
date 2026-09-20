import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { chromium } from "playwright";
import { serveExport } from "./static-server.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const inventory = JSON.parse(
  await readFile(path.join(root, "src/generated/catalogue.json"), "utf8"),
);
const server = await serveExport(
  path.resolve(root, "../docs-site/.registry-publish/out"),
);
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`${server.url}/docs/registry/instruments/`, {
    waitUntil: "networkidle",
  });
  const links = await page
    .locator('a[href*="?instrument="]')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute("href")));
  assert.equal(links.length, inventory.length);
  const checks = [];
  for (const entry of inventory) {
    assert(
      links.some(
        (link) =>
          new URL(link, server.url).searchParams.get("instrument") ===
          entry.type,
      ),
    );
    await page.goto(
      `${server.url}/docs/registry/workbench/?instrument=${entry.type}`,
      { waitUntil: "domcontentloaded" },
    );
    await page.waitForFunction(
      (id) =>
        document.querySelector('[data-field-path="instrument.spec.id"] input')
          ?.value === id,
      entry.exampleId,
      { timeout: 15000 },
    );
    assert.equal(
      await page
        .getByPlaceholder("Search instruments…", { exact: true })
        .inputValue(),
      entry.title,
    );
    checks.push({ type: entry.type, exampleId: entry.exampleId });
  }
  assert.deepEqual(errors, []);
  const report = {
    browser: browser.version(),
    source: "served registry docs export",
    links: links.length,
    checks,
    errors,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr026-links.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser.close();
  await server.close();
}
