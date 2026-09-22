import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { expect } from "playwright/test";
import {
  installBuilt,
  buildConsumer,
  openConsumer,
  closeConsumer,
} from "../../browser/consumer.mjs";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-schema-form-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "schema-form",
    "contract-bond",
    "use-instrument-validator",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/bond-form.tsx", import.meta.url)),
  );

  const modules = await buildConsumer(consumer, { title: "Chart schema-form" });
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
  await page.getByText("Worker: ready", { exact: true }).waitFor();
  const amount = page
    .getByRole("group", { name: "Notional", exact: true })
    .getByRole("textbox", { name: "Amount", exact: true });
  assert(
    (await amount.boundingBox()).width >= 200,
    "Nested objects must not halve field widths",
  );
  await amount.fill("abc");
  assert.equal(await amount.getAttribute("aria-invalid"), "true");
  assert.match(
    (await page.getByRole("alert").allTextContents()).join(" "),
    /pattern|decimal/i,
  );
  await page
    .getByRole("button", { name: "Apply instrument", exact: true })
    .click();
  assert.equal(
    await amount.evaluate((node) => node === document.activeElement),
    true,
  );
  assert.equal(
    await page.getByRole("textbox", { name: "Canonical output" }).inputValue(),
    "",
  );
  await amount.fill("1234567.1234567890");
  await page
    .getByRole("button", { name: "Apply instrument", exact: true })
    .click();
  const output = page.getByRole("textbox", { name: "Canonical output" });
  await page.waitForFunction(
    () => document.querySelector("textarea").value.length > 0,
  );
  const canonical = await output.inputValue();
  assert.equal(
    JSON.parse(canonical).instrument.spec.notional.amount,
    "1234567.1234567890",
  );
  assert.equal(
    await page
      .getByRole("textbox", { name: "Settlement days", exact: true })
      .count(),
    1,
  );
  await page
    .getByRole("button", { name: /^More instrument terms fields/ })
    .click();
  const settlement = page.getByRole("textbox", {
    name: "Settlement days",
    exact: true,
  });
  await settlement.fill("001");
  await page
    .getByRole("button", { name: "Apply instrument", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      JSON.parse(document.querySelector("textarea").value).instrument.spec
        .settlement_days === 1,
  );
  assert.equal(await settlement.inputValue(), "001");
  const maturity = page.getByRole("textbox", { name: "Maturity", exact: true });
  await maturity.fill("2020-01-01");
  const summary = page.getByRole("alert", { name: "Validation errors" });
  await summary.waitFor();
  await page
    .getByRole("button", { name: "Apply instrument", exact: true })
    .click();
  await expect(summary).toBeFocused();
  assert.equal(await output.inputValue(), canonical);
  await maturity.fill("2034-01-15");
  await summary.waitFor({ state: "detached" });
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(
    accessibility.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => n.failureSummary),
    })),
    [],
  );
  assert.deepEqual(failures, []);
  assert(requests.some((url) => url.endsWith(".wasm")));
  assert(
    !modules.some((id) =>
      /tanstack\/(?:react-)?table|@tanstack\/charts/.test(id),
    ),
  );
  await page.screenshot({ path: "/tmp/pr012-schema-form.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    checks: [
      "lazy generated bond module",
      "actual shared WASM worker validation",
      "inline decimal-pattern errors",
      "invalid submit focuses the field",
      "native domain error focuses summary and preserves prior output",
      "canonical native output",
      "working text retained after canonicalization",
      "basic/full fields",
      "accessible generated controls",
    ],
    failures,
    accessibility: accessibility.violations,
    wasmRequests: requests.filter((url) => url.endsWith(".wasm")).length,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr012-schema-form.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
