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
const consumer = await mkdtemp(path.join(root, ".consumer-heatmap-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, ["finstack-chart"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/heatmap.tsx", import.meta.url)),
  );

  await buildConsumer(consumer, { title: "Publication figures" });
  ({ server, browser, page } = await openConsumer(path.join(consumer, "dist"), {
    viewport: { width: 1050, height: 1000 },
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
    () => document.querySelectorAll("svg.ts-chart").length === 2,
  );
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const small = page.getByRole("region", { name: "Small grid", exact: true });
  const chart = page.locator('[aria-label="Small grid"][tabindex]');
  await chart.focus();
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  assert.equal(await page.getByLabel("Accepted cell").textContent(), "cell-0");
  await page.keyboard.press("Escape");
  await chart.press("End");
  await page.keyboard.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Accepted cell"]').textContent ===
      "cell-8",
  );
  assert(await page.getByText(/Transient cell/).count());
  const exports = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    for (const which of ["small", "large"]) {
      const svg = await page.evaluate(
        ({ which, theme }) => window.heatmapProbe.export(which, theme),
        { which, theme },
      );
      assert(!svg.includes("Transient cell") && !svg.includes("var("));
      assert(svg.includes("Raw value") && svg.includes("Supplied x"));
      if (which === "small")
        for (const text of [
          "Original values, no calculation",
          "Caller grid",
          "Supplied note",
        ])
          assert(svg.includes(text));
      const geometry = await page.evaluate(
        async ({ svg, which }) => {
          const host = document.createElement("div");
          host.innerHTML = svg;
          document.body.append(host);
          await document.fonts.ready;
          const root = host.querySelector("svg"),
            bounds = root.getBoundingClientRect();
          const labels = [...root.querySelectorAll("text")];
          const values = labels.filter((n) =>
            n.getAttribute("data-ts-key")?.includes("heatmap-values"),
          );
          const outside = labels
            .filter((n) => {
              const r = n.getBoundingClientRect();
              return (
                r.left < bounds.left - 1 ||
                r.top < bounds.top - 1 ||
                r.right > bounds.right + 1 ||
                r.bottom > bounds.bottom + 1
              );
            })
            .map((n) => n.textContent);
          const result = { values: values.length, outside };
          host.remove();
          return result;
        },
        { svg, which },
      );
      assert.equal(geometry.values, which === "small" ? 9 : 0);
      assert.deepEqual(geometry.outside, []);
      await writeFile(`/tmp/pr027-${which}-${theme}.svg`, svg);
      exports.push({ theme, which, ...geometry });
    }
  }
  const accessibility = await page.evaluate(() => window.axe.run(document));
  assert.deepEqual(accessibility.violations, []);
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  await page.screenshot({ path: "/tmp/pr027-heatmap.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    exports,
    accessibilityViolations: 0,
    wasmRequests: 0,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr027-heatmap.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
