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
const consumer = await mkdtemp(path.join(root, ".consumer-market-browser-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "market-context-form",
    "use-market-validator",
    "market-context-browser",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/market-browser.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/core/market-browser/cases.json")),
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
  async function originalSelected() {
    const viewer = page.locator('[aria-label="Selected stored value"]');
    if (!(await viewer.isVisible()))
      await page
        .getByText("Selected stored value · JSON", { exact: true })
        .click();
    await viewer.getByRole("button", { name: "Original", exact: true }).click();
  }
  const leaves = await page.evaluate(() => window.marketProbe.leaves);
  for (const leaf of leaves) {
    await page
      .getByLabel("Search market fields", { exact: true })
      .fill(leaf.pointer);
    await page
      .getByRole("button", { name: `Inspect ${leaf.pointer}`, exact: true })
      .click();
    await originalSelected();
    assert.equal(
      await page
        .getByRole("region", { name: "Selected stored value", exact: true })
        .locator("pre")
        .textContent(),
      leaf.json,
    );
  }
  await page
    .getByLabel("Search market fields", { exact: true })
    .fill("USD~1CSA");
  await page
    .getByRole("button", { name: "Inspect /collateral/USD~1CSA", exact: true })
    .click();
  await originalSelected();
  assert.equal(
    await page
      .getByRole("region", { name: "Selected stored value", exact: true })
      .locator("pre")
      .textContent(),
    '"NEG-RATES"',
  );
  const pointer = await page.evaluate(() => window.marketProbe.negativePointer);
  await page.getByLabel("Search market fields", { exact: true }).fill(pointer);
  await page
    .getByRole("button", { name: `Inspect ${pointer}`, exact: true })
    .click();
  await originalSelected();
  const selected = page
    .getByRole("region", { name: "Selected stored value", exact: true })
    .locator("pre");
  assert.equal(await selected.textContent(), "1.01");
  const knotIndex = await page.evaluate(() => window.marketProbe.knotIndex);
  const input = page
    .getByLabel("Stored value 1", { exact: true })
    .nth(knotIndex);
  await input.fill("1.02");
  assert.equal(await selected.textContent(), "1.01");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Selected stored value"] pre')
        .textContent === "1.02",
  );
  await input.fill("-1");
  await page.getByRole("alert").first().waitFor();
  assert.equal(await selected.textContent(), "1.02");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.getByRole("alert").first().waitFor();
  assert.equal(await selected.textContent(), "1.02");
  await input.fill("1.02");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page
    .getByLabel("Search market fields", { exact: true })
    .fill("fx/config");
  await page
    .getByRole("button", { name: "Inspect /fx/config", exact: true })
    .click();
  const fx = page.getByRole("region", {
    name: "Complete stored FX state",
    exact: true,
  });
  await fx.getByRole("button", { name: "Copy", exact: true }).click();
  assert.deepEqual(
    JSON.parse(await page.evaluate(() => navigator.clipboard.readText())),
    await page.evaluate(() => window.marketProbe.market.fx),
  );
  await page.getByLabel("Search market fields", { exact: true }).fill("");
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
      .getByRole("region", { name: "Market context browser", exact: true })
      .screenshot({ path: `/tmp/pr031-${theme}.png` });
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
    leavesNavigated: leaves.length,
    accessibility,
    wasmRequests: 1,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr031-market-browser.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
