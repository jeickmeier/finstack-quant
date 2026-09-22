import { buildConsumer, openConsumer, closeConsumer } from "./consumer.mjs";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { copyShadcn } from "../../scripts/shadcn.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-theme-"));
let browser, server, page;
try {
  await promisify(execFile)(
    process.execPath,
    [
      path.join(root, "node_modules/shadcn/dist/index.js"),
      "build",
      "--output",
      path.join(consumer, "r"),
    ],
    { cwd: root },
  );
  const item = JSON.parse(
    await readFile(path.join(consumer, "r/finstack-theme.json"), "utf8"),
  );
  for (const file of item.files) {
    const target = path.join(consumer, file.target);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, file.content);
  }
  await copyShadcn(root, consumer, ["input"]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    'import React from "react"; import {createRoot} from "react-dom/client"; import {Input} from "./components/ui/input"; createRoot(document.getElementById("field-host")!).render(<Input id="field" defaultValue="BOND_A" />);',
  );
  await mkdir(path.join(consumer, "public"));
  await writeFile(
    path.join(consumer, "public/tenant.css"),
    await readFile(path.join(root, "tests/shared/fixtures/tenant.css")),
  );

  await buildConsumer(consumer, {
    html: `<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Registry theme</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface p-8"><main class="max-w-3xl mx-auto"><h1 class="text-xl font-medium">Component registry theme</h1><p class="text-muted-foreground my-4">IBM Plex · shared controls, tables and charts</p><section class="bg-card border border-border rounded-md p-4"><label class="block text-sm" for="field">Instrument identifier</label><div id="field-host"></div><div id="row" class="finstack-row finstack-numeric flex items-center justify-between border-b border-border"><span>Present value · USD</span><span>1,024,567.80</span></div><p class="text-primary my-4">Shared tenant primary</p><div class="flex gap-2">${Array.from({ length: 6 }, (_, i) => `<span class="rounded-sm p-2 text-xs" style="border: var(--border-width) solid var(--chart-${i + 1})">Series ${i + 1}</span>`).join("")}</div></section></main><script type="module" src="/main.tsx"></script></body></html>`,
  });
  ({ server, browser, page } = await openConsumer(path.join(consumer, "dist"), {
    viewport: { width: 1000, height: 650 },
  }));
  const fonts = [];
  const failures = [];
  page.on("response", (r) => {
    if (/\.woff2?(?:\?|$)/.test(r.url()))
      fonts.push({ url: new URL(r.url()).pathname, status: r.status() });
    if (r.status() >= 400) failures.push(r.url());
  });
  page.on("pageerror", (error) => failures.push(error.message));
  await page.goto(server.url, { waitUntil: "networkidle" });
  const faces = await page.evaluate(async () => {
    const specs = [
      '400 14px "IBM Plex Sans"',
      'italic 400 14px "IBM Plex Sans"',
      '500 14px "IBM Plex Sans"',
      '600 14px "IBM Plex Sans"',
      '400 14px "IBM Plex Mono"',
      '500 14px "IBM Plex Mono"',
    ];
    return Promise.all(
      specs.map(async (spec) => ({
        spec,
        count: (await document.fonts.load(spec)).length,
        loaded: document.fonts.check(spec),
      })),
    );
  });
  assert(faces.every((face) => face.loaded && face.count > 0));
  assert(fonts.length >= 6 && fonts.every((font) => font.status === 200));
  const modes = [];
  for (const theme of ["light", "dark"])
    for (const density of ["compact", "comfortable"]) {
      const measured = await page.evaluate(
        ({ theme, density }) => {
          document.documentElement.dataset.theme = theme;
          document.documentElement.dataset.density = density;
          const style = getComputedStyle(document.body);
          return {
            theme,
            density,
            background: style.backgroundColor,
            font: style.fontFamily,
            row: document.querySelector("#row").getBoundingClientRect().height,
            field: document.querySelector("#field").getBoundingClientRect()
              .height,
            primary: style.getPropertyValue("--primary").trim(),
          };
        },
        { theme, density },
      );
      assert.equal(measured.row, density === "compact" ? 28 : 36);
      assert.equal(
        measured.field,
        36,
        "App density must not override the stock Input height",
      );
      assert(measured.font.includes("IBM Plex Sans"));
      modes.push(measured);
    }
  await page.evaluate(
    () =>
      new Promise((resolve, reject) => {
        const link = document.createElement("link");
        link.rel = "stylesheet";
        link.href = "/tenant.css";
        link.onload = resolve;
        link.onerror = reject;
        document.head.append(link);
      }),
  );
  const tenant = [];
  for (const theme of ["light", "dark"]) {
    const primary = await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
      return getComputedStyle(document.body)
        .getPropertyValue("--primary")
        .trim();
    }, theme);
    assert.equal(primary, theme === "light" ? "#5146a5" : "#b7acf2");
    tenant.push({ theme, primary });
  }
  assert.deepEqual(failures, []);
  const output = process.env.REGISTRY_THEME_REPORT ?? "/tmp/pr006-theme.json";
  await page.screenshot({
    path: output.replace(/\.json$/, ".png"),
    fullPage: true,
  });
  const report = {
    browser: browser.version(),
    faces,
    fonts,
    modes,
    tenant,
    failures,
    verdict: "pass",
  };
  await writeFile(output, JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
