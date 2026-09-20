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
const consumer = await mkdtemp(path.join(root, ".consumer-figures-"));
let browser, server;
try {
  const installed = await installBuilt(root, consumer, ["figure-example"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/figures.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n@source "./index.html";\n@source "./main.tsx";\n@source "./components";\n',
  );
  await writeFile(
    path.join(consumer, "index.html"),
    '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Publication figures</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
  );
  await build({
    root: consumer,
    configFile: false,
    logLevel: "error",
    css: { postcss: { plugins: [tailwind()] } },
    build: { outDir: path.join(consumer, "dist"), emptyOutDir: true },
  });
  server = await serveExport(path.join(consumer, "dist"));
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1050, height: 1000 },
  });
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
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const exports = [];
  for (const name of ["numeric", "date", "category"])
    for (const width of [340, 900]) {
      const options = {
        width,
        height: name === "numeric" ? 640 : 480,
        scale: 2,
        theme: "light",
      };
      const svg = await page.evaluate(
        ({ name, options }) => window.figureProbe.export(name, "svg", options),
        { name, options },
      );
      assert(
        svg.includes("<text") &&
          (svg.includes("<path") || svg.includes("<circle")),
      );
      assert(
        !svg.includes("foreignObject") &&
          !svg.includes("<image") &&
          !svg.includes("var("),
      );
      assert(svg.includes("data:font/") || svg.includes("data:application/"));
      assert(
        svg.includes(`width="${width}"`) &&
          svg.includes(`height="${options.height}"`),
      );
      await writeFile(`/tmp/pr010-${name}-${width}.svg`, svg);
      // Inspect all exported text geometry in the same loaded browser font environment.
      const geometry = await page.evaluate(
        async ({ svg, width, height }) => {
          const host = document.createElement("div");
          host.innerHTML = svg;
          document.body.append(host);
          await document.fonts.ready;
          const root = host.querySelector("svg");
          const bounds = root.getBoundingClientRect();
          const outside = [...root.querySelectorAll("text")]
            .filter((text) => text.textContent.trim())
            .flatMap((text) => {
              const r = text.getBoundingClientRect();
              return r.left < bounds.left - 1 ||
                r.right > bounds.right + 1 ||
                r.top < bounds.top - 1 ||
                r.bottom > bounds.bottom + 1
                ? [
                    {
                      text: text.textContent,
                      box: {
                        left: r.left - bounds.left,
                        top: r.top - bounds.top,
                        right: r.right - bounds.left,
                        bottom: r.bottom - bounds.top,
                      },
                    },
                  ]
                : [];
            });
          const prose = [
            ...root.querySelectorAll('[data-ts-key^="figure-"]'),
          ].filter((node) => node.tagName === "text");
          const chartText = [...root.querySelectorAll("text")].filter(
            (node) => !node.getAttribute("data-ts-key")?.startsWith("figure-"),
          );
          const overlaps = prose.flatMap((note) =>
            chartText.flatMap((label) => {
              const a = note.getBoundingClientRect(),
                b = label.getBoundingClientRect();
              return a.left < b.right &&
                a.right > b.left &&
                a.top < b.bottom &&
                a.bottom > b.top
                ? [{ text: note.textContent, overlap: label.textContent }]
                : [];
            }),
          );
          host.remove();
          return [...outside, ...overlaps];
        },
        { svg, width, height: options.height },
      );
      assert.deepEqual(
        geometry,
        [],
        `${name}/${width}: text must fit the exported figure`,
      );
      const png = Buffer.from(
        await page.evaluate(
          ({ name, options }) =>
            window.figureProbe.export(name, "png", options),
          { name, options },
        ),
      );
      assert.equal(png.readUInt32BE(16), width * 2);
      assert.equal(png.readUInt32BE(20), options.height * 2);
      await writeFile(`/tmp/pr010-${name}-${width}.png`, png);
      exports.push({
        name,
        width,
        height: options.height,
        pngWidth: width * 2,
        pngHeight: options.height * 2,
        svgBytes: Buffer.byteLength(svg),
        pngBytes: png.length,
        clippedText: geometry.length,
      });
    }
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
      await page.waitForTimeout(100);
      const result = await page.evaluate(() => window.axe.run(document));
      accessibility.push({
        theme,
        density,
        violations: result.violations.map((v) => ({
          id: v.id,
          nodes: v.nodes.map((n) => n.failureSummary),
        })),
      });
    }
  assert(
    accessibility.every((mode) => mode.violations.length === 0),
    JSON.stringify(accessibility),
  );
  const lightFromDark = await page.evaluate(() =>
    window.figureProbe.export("numeric", "svg", {
      width: 900,
      height: 640,
      theme: "light",
    }),
  );
  const tokens = JSON.parse(
    await readFile(
      path.join(root, "registry/theme/finstack-theme/tokens.json"),
      "utf8",
    ),
  );
  assert(
    lightFromDark.includes(`fill="${tokens.light.background}"`),
    "A light export must be independent of the dark application theme",
  );
  assert.deepEqual(failures, []);
  assert(!requests.some((url) => url.endsWith(".wasm")));
  await page.screenshot({ path: "/tmp/pr010-figures.png", fullPage: true });
  const report = {
    browser: browser.version(),
    installed,
    exports,
    accessibility,
    failures,
    wasmRequests: 0,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr010-figures.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser?.close();
  await server?.close();
  await rm(consumer, { recursive: true, force: true });
}
