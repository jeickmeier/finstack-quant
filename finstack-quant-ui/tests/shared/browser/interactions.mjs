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
const consumer = await mkdtemp(path.join(root, ".consumer-interactions-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, ["figure-example"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/interactions.tsx", import.meta.url)),
  );

  const modules = await buildConsumer(consumer, {
    title: "Chart interactions",
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
  await page.waitForFunction(
    () => document.querySelectorAll("svg.ts-chart").length === 4,
  );
  const parent = page
    .getByRole("region", { name: "Linked figure example" })
    .first();
  const independent = page
    .getByRole("region", { name: "Linked figure example" })
    .nth(1);
  const chart = parent.locator(
    '[aria-label="Interactive observations"][tabindex]',
  );
  const status = parent.locator("[data-selection-status]");
  const events = parent.locator("[data-interaction-status]");
  // Ordinary controls update persistent state without stealing focus or activating charts.
  const select = parent.getByRole("button", {
    name: "Select second observation",
  });
  await select.click();
  assert.equal(
    await select.evaluate((node) => node === document.activeElement),
    true,
  );
  assert.match(await status.textContent(), /Accepted: b/);
  assert.match(
    await events.textContent(),
    /Activations: 0; proposals: 1; focus: none; group: 0/,
  );
  assert.match(
    await independent.locator("[data-selection-status]").textContent(),
    /Accepted: none/,
  );
  await parent.getByRole("button", { name: "Reverse observations" }).click();
  await parent
    .getByRole("button", { name: "Toggle second observation" })
    .click();
  assert.match(await status.textContent(), /Accepted: b/);
  await parent
    .getByRole("button", { name: "Toggle second observation" })
    .click();
  await parent.getByRole("button", { name: "Reverse observations" }).click();
  await parent.getByRole("button", { name: "Clear selection" }).click();
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  await page.waitForFunction(() =>
    document
      .querySelector("[data-selection-status]")
      .textContent.includes("Accepted: a"),
  );
  assert.match(await events.textContent(), /Activations: 1; proposals: 3/);
  const action = parent.getByRole("button", {
    name: "Open observation details",
  });
  await action.waitFor();
  // Native pinning enables keyboard action and keeps default native tooltip rows.
  assert(
    (await parent.locator(".ts-chart-tooltip").allTextContents()).some((s) =>
      s.includes("Supplied"),
    ),
  );
  await chart.press("Tab");
  assert.equal(
    await action.evaluate((node) => node === document.activeElement),
    true,
  );
  const exported = await page.evaluate(() => window.exportInteraction());
  assert(
    !exported.includes("Open observation details") &&
      !exported.includes("ts-chart-tooltip"),
  );
  assert.equal(
    await action.evaluate((node) => node === document.activeElement),
    true,
    "Export preserves the focused tooltip action",
  );
  await action.press("Enter");
  await page.waitForFunction(() =>
    document
      .querySelector("[data-selection-status]")
      .textContent.includes("detail: a"),
  );
  assert.match(await status.textContent(), /detail: a/);
  await page.waitForFunction(
    () => ![...document.querySelectorAll(".ts-chart-tooltip button")].length,
  );
  // Controlled rejection does not overwrite acceptance or stop activation notification.
  await parent
    .getByRole("checkbox", { name: "Reject selection proposals" })
    .check();
  await chart.focus();
  await chart.press("End");
  await chart.press("Enter");
  assert.match(await status.textContent(), /Accepted: a/);
  assert.match(await events.textContent(), /Activations: 2; proposals: 4/);
  await chart.press("Escape");
  assert.match(await events.textContent(), /focus: none; group: 0/);
  await parent
    .getByRole("checkbox", { name: "Reject selection proposals" })
    .uncheck();
  // Shared cursor mirrors focus in the compatible chart, not the independent parent.
  await chart.focus();
  await chart.press("ArrowRight");
  assert.equal(await parent.locator(".ts-chart-tooltip").count(), 2);
  assert.equal(await independent.locator(".ts-chart-tooltip").count(), 0);
  await chart.press("Escape");
  await chart.click({ position: { x: 1, y: 1 } });
  assert.match(await status.textContent(), /Accepted: none/);
  assert.match(await events.textContent(), /Activations: 3; proposals: 5/);
  await page.screenshot({
    path: "/tmp/pr011-interactions.png",
    fullPage: true,
  });
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  const report = {
    browser: browser.version(),
    installed,
    checks: [
      "controlled accept/reject/clear",
      "programmatic selection preserves focus",
      "reorder/removal preserves key",
      "independent parents",
      "native shared cursor",
      "single activation and selection proposal",
      "default/custom tooltip",
      "keyboard pinned action and dismissal",
      "null/empty focus clears",
      "export excludes transient tooltip",
    ],
    failures,
    tableRuntimeImports: 0,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr011-interactions.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
