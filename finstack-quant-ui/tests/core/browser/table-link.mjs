import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { chromium } from "playwright";
import { serveExport } from "../../browser/static-server.mjs";
import {
  installBuilt,
  buildConsumer,
  closeConsumer,
} from "../../browser/consumer.mjs";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const mode = process.argv[2];
if (!mode) {
  // Tailwind's process-wide imported CSS cache must not carry a Vite rebasing
  // result between independent consumer builds.
  for (const childMode of ["table", "linked"])
    await new Promise((resolve, reject) => {
      const child = spawn(
        process.execPath,
        [fileURLToPath(import.meta.url), childMode],
        { stdio: "inherit", env: process.env },
      );
      child.once("error", reject);
      child.once("exit", (code) =>
        code === 0
          ? resolve()
          : reject(new Error(`Consumer failed: ${childMode}`)),
      );
    });
  const reports = await Promise.all(
    ["table", "linked"].map((name) =>
      readFile(`/tmp/pr018-${name}.json`, "utf8").then(JSON.parse),
    ),
  );
  await writeFile(
    "/tmp/pr018-table-link.json",
    JSON.stringify({ reports, verdict: "pass" }, null, 2) + "\n",
  );
  process.exit(0);
}

const consumers = [];
let browser, server;
try {
  browser = await chromium.launch({ headless: true });
  const reports = [];
  for (const linked of [mode === "linked"]) {
    const consumer = await mkdtemp(path.join(root, ".consumer-table-link-"));
    consumers.push(consumer);
    const name = linked ? "curve-link-example" : "finstack-table";
    const installed = await installBuilt(root, consumer, [name]);
    const entry = linked
      ? 'import { CurveLinkExample } from "./components/finstack/core/components/curve-link-example/curve-link-example"; createRoot(document.getElementById("root")!).render(<main><h1>Table and curve linking</h1><CurveLinkExample/><CurveLinkExample label="Independent stored curves"/></main>);'
      : 'import { FinstackTable } from "./components/finstack/shared/table/finstack-table/finstack-table"; createRoot(document.getElementById("root")!).render(<main><h1>Independent table</h1><FinstackTable caption="Exact supplied values" data={[{id:"a",value:"1000000.123456789"}]} columns={[{id:"value",accessorKey:"value",header:"Value"}]} getRowId={row=>row.id}/></main>);';
    await writeFile(
      path.join(consumer, "main.tsx"),
      'import { createRoot } from "react-dom/client";\n' + entry,
    );

    const modules = await buildConsumer(consumer, {
      title: "Stored curve selection",
    });
    assert(!modules.some((id) => id.includes("finstack-quant-wasm")));
    if (!linked) assert(!modules.some((id) => id.includes("@tanstack/charts")));
    server = await serveExport(path.join(consumer, "dist"));
    const page = await browser.newPage({
      viewport: { width: 1150, height: 1050 },
    });
    const failures = [],
      requests = [];
    page.on("pageerror", (error) => failures.push(error.message));
    page.on("response", (r) => {
      requests.push(r.url());
      if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
    });
    await page.goto(server.url, { waitUntil: "networkidle" });
    if (linked) {
      await page.waitForFunction(
        () => document.querySelectorAll("svg.ts-chart").length === 2,
      );
      const first = page
        .getByRole("region", { name: "Linked stored curves" })
        .first();
      const second = page.getByRole("region", {
        name: "Independent stored curves",
      });
      const status = first.locator("[data-link-status]");
      await first
        .getByRole("gridcell", { name: "OVERLAY", exact: true })
        .click();
      assert.match(
        await status.textContent(),
        /Accepted: \["series","OVERLAY"\]; proposals: 1; activations: 1/,
      );
      const value = first.getByRole("gridcell", {
        name: "0.9512345678901234",
        exact: true,
      });
      await value.click();
      assert.equal(await value.getAttribute("aria-selected"), "true");
      assert.match(await status.textContent(), /proposals: 2; activations: 2/);
      assert.match(
        await second.locator("[data-link-status]").textContent(),
        /Accepted: none; proposals: 0/,
      );
      await first
        .getByRole("button", { name: "Inspect OVERLAY", exact: true })
        .click();
      assert.match(
        await status.textContent(),
        /proposals: 2; activations: 2; detail: Inspect OVERLAY/,
      );
      await first.getByRole("button", { name: "Clear", exact: true }).click();
      const chart = first.locator(
        '[aria-label="Linked stored curves stored curves"][tabindex]',
      );
      await chart.focus();
      await chart.press("End");
      await chart.press("Enter");
      assert.match(
        await status.textContent(),
        /Accepted: \["point","USD-OIS",40\]; proposals: 4; activations: 3/,
      );
      assert.equal(await first.locator('td[aria-selected="true"]').count(), 1);
      await chart.press("Escape");
      await first.getByRole("checkbox", { name: "Reject proposals" }).check();
      await value.click();
      assert.equal(await value.getAttribute("aria-selected"), "false");
      assert.match(
        await status.textContent(),
        /Accepted: \["point","USD-OIS",40\]; proposals: 5; activations: 4/,
      );
      await first.getByRole("checkbox", { name: "Reject proposals" }).uncheck();
      const select = first.getByRole("button", {
        name: "Select overlay point",
      });
      await select.click();
      assert.equal(
        await select.evaluate((n) => n === document.activeElement),
        true,
      );
      assert.match(await status.textContent(), /proposals: 6; activations: 4/);
      await first.getByRole("button", { name: "Reverse rows" }).click();
      assert.equal(await value.getAttribute("aria-selected"), "true");
      await first.getByRole("button", { name: "Toggle overlay" }).click();
      assert.equal(await first.locator('td[aria-selected="true"]').count(), 0);
      assert.match(await status.textContent(), /proposals: 6; activations: 4/);
      await page.screenshot({ path: "/tmp/pr018-table-link.png" });
    } else
      await page
        .getByRole("cell", { name: "1000000.123456789", exact: true })
        .waitFor();
    await page.addScriptTag({
      path: path.join(root, "node_modules/axe-core/axe.min.js"),
    });
    const axe = await page.evaluate(() => window.axe.run(document));
    assert.deepEqual(
      axe.violations.map((v) => ({
        id: v.id,
        nodes: v.nodes.map((n) => n.failureSummary),
      })),
      [],
    );
    assert.deepEqual(failures, []);
    assert(!requests.some((url) => url.endsWith(".wasm")));
    reports.push({
      name,
      installed,
      violations: [],
      failures,
      chartRuntime: modules.some((id) => id.includes("@tanstack/charts")),
      wasmRuntime: false,
      verdict: "pass",
    });
    await page.close();
    await server.close();
    server = undefined;
  }
  const report = { browser: browser.version(), reports, verdict: "pass" };
  await writeFile(
    `/tmp/pr018-${mode}.json`,
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, ...consumers);
}
