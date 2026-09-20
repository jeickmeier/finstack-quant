import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import tailwind from "@tailwindcss/postcss";
import { chromium } from "playwright";
import { serveExport } from "./static-server.mjs";
import { installBuilt } from "./consumer.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-schema-form-"));
let browser, server;
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
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Chart schema-form</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
  );
  const modules = [];
  await build({
    root: consumer,
    configFile: false,
    logLevel: "error",
    resolve: { alias: { "@": consumer } },
    worker: { format: "es" },
    plugins: [
      {
        name: "inspect-dependencies",
        generateBundle(_, bundle) {
          for (const chunk of Object.values(bundle))
            if (chunk.type === "chunk")
              modules.push(...Object.keys(chunk.modules));
        },
      },
    ],
    css: { postcss: { plugins: [tailwind()] } },
    build: { outDir: path.join(consumer, "dist"), emptyOutDir: true },
  });
  assert(
    !modules.some((id) =>
      /tanstack\/(?:react-)?table|finstack-quant-wasm/.test(id),
    ),
  );
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1000, height: 1000 },
  });
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
    0,
  );
  await page
    .getByRole("button", { name: "More instrument terms fields", exact: true })
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
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
