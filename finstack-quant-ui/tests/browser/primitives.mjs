import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import tailwind from "@tailwindcss/postcss";
import { chromium } from "playwright";
import { expect } from "playwright/test";
import { serveExport } from "./static-server.mjs";
import { installBuilt } from "./consumer.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-primitives-"));
let browser, server;
try {
  const names = JSON.parse(
    await readFile(
      path.join(root, "registry/primitives/registry.json"),
      "utf8",
    ),
  ).items.map((item) => item.name);
  const installed = await installBuilt(root, consumer, names);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/primitives.tsx", import.meta.url)),
  );
  const bond = JSON.parse(
    await readFile(path.join(root, "src/generated/examples/bond.json"), "utf8"),
  );
  const text = ' {"seed":18446744073709551615,"amount":"1.2300"}\n';
  await writeFile(
    path.join(consumer, "data.json"),
    JSON.stringify({ notional: bond.instrument.spec.notional, text }),
  );
  await writeFile(
    path.join(consumer, "tsconfig.json"),
    JSON.stringify({
      compilerOptions: {
        jsx: "react-jsx",
        module: "ESNext",
        moduleResolution: "Bundler",
        target: "ES2022",
        lib: ["ES2024", "DOM", "DOM.Iterable"],
        strict: true,
        noEmit: true,
        resolveJsonModule: true,
        skipLibCheck: true,
        baseUrl: ".",
        paths: { "@/*": ["./*"] },
      },
    }),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n@source "./lib/finstack/format";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Registry primitives</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
  );
  await promisify(execFile)(
    process.execPath,
    [
      path.join(root, "node_modules/typescript/bin/tsc"),
      "--project",
      path.join(consumer, "tsconfig.json"),
    ],
    { cwd: consumer },
  );
  await build({
    root: consumer,
    configFile: false,
    logLevel: "error",
    resolve: { alias: { "@": consumer } },
    css: { postcss: { plugins: [tailwind()] } },
    build: { outDir: path.join(consumer, "dist"), emptyOutDir: true },
  });
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1200, height: 1100 },
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const page = await context.newPage();
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    if (r.status() >= 400) failures.push(r.url());
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  const decimal = page.getByRole("textbox", { name: "Exact decimal" });
  await decimal.focus();
  assert.equal(await decimal.inputValue(), "12345678901234567890.1234");
  await decimal.fill("12345678901234567890.1235");
  await decimal.blur();
  assert.equal(await decimal.inputValue(), "12,345,678,901,234,567,890.1235");
  await page.getByRole("button", { name: "Exact decimal help" }).click();
  assert(
    await page
      .getByText("Canonical decimal strings retain their supplied precision.")
      .isVisible(),
  );
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("button", { name: "Exact decimal help" }),
  ).toBeFocused();
  const id = page.getByRole("combobox", { name: "Identifier" });
  await id.click();
  await page.getByRole("option").first().waitFor();
  const visibleOptions = await page.getByRole("option").count();
  assert(
    visibleOptions > 0 && visibleOptions < 100,
    "ID suggestions must be virtualized",
  );
  await id.press("ArrowUp");
  await page.getByRole("option", { name: "ID-09999", exact: true }).waitFor();
  await id.press("Enter");
  await page.waitForFunction(
    () =>
      document.querySelector('input[role="combobox"]')?.value === "ID-09999",
  );
  await id.fill("CUSTOM-ID");
  assert.equal(await page.getByLabel("Accepted ID").textContent(), "CUSTOM-ID");
  await id.press("Escape");
  const model = page.getByRole("combobox", { name: "Supplied model" });
  await model.click();
  await page.getByRole("option", { name: "Beta" }).waitFor();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  assert((await model.textContent()).includes("Beta"));
  await page.getByRole("button", { name: "As of calendar" }).click();
  await page.getByRole("button", { name: /February 28/ }).click();
  await expect(
    page.getByRole("button", { name: /February 28/ }),
  ).not.toBeVisible();
  assert.equal(
    await page
      .getByRole("textbox", { name: "As of", exact: true })
      .inputValue(),
    "2024-02-28",
  );
  await page.getByRole("button", { name: "Original", exact: true }).click();
  assert.equal(await page.locator("pre").textContent(), text);
  await page.getByRole("button", { name: "Copy", exact: true }).click();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), text);
  const wideTable = page.getByRole("table", { name: "Wide supplied values" });
  await page.locator("#wide-table-start").focus();
  await page.keyboard.press("Tab");
  await expect(wideTable).toBeFocused();
  const tableScroll = wideTable.locator("..");
  assert.equal(await tableScroll.getAttribute("data-slot"), "table-container");
  assert(
    await tableScroll.evaluate((node) => node.scrollWidth > node.clientWidth),
  );
  await page.keyboard.press("ArrowRight");
  await expect
    .poll(() => tableScroll.evaluate((node) => node.scrollLeft))
    .toBeGreaterThan(0);
  const keyboardTableScroll = await tableScroll.evaluate((node) => ({
    scrollLeft: node.scrollLeft,
    scrollWidth: node.scrollWidth,
    clientWidth: node.clientWidth,
  }));
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = [];
  for (const theme of ["light", "dark"])
    for (const density of ["compact", "comfortable"]) {
      await page.evaluate(
        ({ theme, density }) => {
          document.documentElement.dataset.theme = theme;
          document.documentElement.dataset.density = density;
        },
        { theme, density },
      );
      await page.evaluate(async () => {
        document.body.getBoundingClientRect();
        await Promise.all(
          document
            .getAnimations()
            .filter(
              (animation) =>
                animation.effect?.getTiming().iterations !== Infinity,
            )
            .map((animation) => animation.finished.catch(() => {})),
        );
      });
      const result = await page.evaluate(() => window.axe.run(document));
      accessibility.push({
        theme,
        density,
        violations: result.violations.map((v) => ({
          id: v.id,
          nodes: v.nodes.map((n) => ({
            target: n.target,
            summary: n.failureSummary,
          })),
        })),
      });
    }
  assert(
    accessibility.every((mode) => mode.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert(
    !server.requests.some((url) => url.endsWith(".wasm")),
    "Primitives must not initialize WASM",
  );
  const output =
    process.env.REGISTRY_PRIMITIVES_REPORT ?? "/tmp/pr008-primitives.json";
  await page.screenshot({
    path: output.replace(/\.json$/, ".png"),
    fullPage: true,
  });
  const report = {
    browser: browser.version(),
    installed,
    visibleOptions,
    keyboardTableScroll,
    accessibility,
    failures,
    verdict: "pass",
  };
  await writeFile(output, JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
