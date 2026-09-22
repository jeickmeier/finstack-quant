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
const consumer = await mkdtemp(path.join(root, ".consumer-details-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "valuation-details",
    "contract-valuation-result",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/details.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/valuations/details/cases.json")),
  );

  await writeFile(
    path.join(consumer, "restore.ts"),
    (
      await readFile(
        path.join(root, "tests/valuations/details/restore.ts"),
        "utf8",
      )
    ).replaceAll("../../../src/", "./lib/finstack/"),
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
    await readFile(
      path.join(root, "tests/valuations/details/cases.json"),
      "utf8",
    ),
  );
  const checks = [];
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  for (const entry of fixture.cases) {
    await page.getByRole("button", { name: entry.type, exact: true }).click();
    if (entry.detailType === "monte_carlo")
      await page
        .getByRole("region", { name: "Monte Carlo diagnostics" })
        .waitFor();
    else
      await page
        .getByRole("region", {
          name: {
            composite: "Composite details JSON",
            credit_derivative: "Credit derivative details JSON",
            fx: "FX details JSON",
            structured_credit_stochastic: "Structured credit details JSON",
          }[entry.detailType],
        })
        .waitFor();
    const summary = page.getByText("Complete result JSON", { exact: true });
    if (!(await summary.evaluate((node) => node.parentElement.open)))
      await summary.click();
    const raw = page.getByRole("region", {
      name: "Complete valuation result JSON",
    });
    await raw.getByRole("button", { name: "Original", exact: true }).click();
    const text = await raw.locator("pre").textContent();
    assert(text.includes(`"type":"${entry.detailType}"`));
    if (entry.type === "bond") {
      const seed = entry.resultJson.match(/"seed":(\d+)/)[1];
      assert(text.includes(`"seed":${seed}`));
      assert(BigInt(seed) > BigInt(Number.MAX_SAFE_INTEGER));
      assert(await page.getByText(seed, { exact: true }).isVisible());
    }
    await raw.getByRole("button", { name: "Copy", exact: true }).click();
    await raw.getByText("Copied", { exact: true }).waitFor();
    assert.equal(
      await page.evaluate(() => navigator.clipboard.readText()),
      text,
    );
    const audit = await page.evaluate(() => window.axe.run(document));
    assert.deepEqual(audit.violations, []);
    checks.push({
      type: entry.type,
      detailType: entry.detailType,
      exactCopy: true,
      violations: audit.violations,
    });
  }
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 0);
  const report = {
    browser: browser.version(),
    installed,
    checks,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr023-details.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
