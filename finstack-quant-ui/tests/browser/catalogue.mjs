import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import tailwind from "@tailwindcss/postcss";
import { chromium } from "playwright";
import { installBuilt } from "./consumer.mjs";
import { serveExport } from "./static-server.mjs";
const root = fileURLToPath(new URL("../../", import.meta.url));
const inventory = JSON.parse(
  await readFile(path.join(root, "src/generated/catalogue.json"), "utf8"),
);
const browser = await chromium.launch({ headless: true });
const reports = [];
try {
  for (const mode of ["package", "installed"]) {
    const directory = await mkdtemp(path.join(root, ".consumer-catalogue-"));
    let server;
    try {
      const installed =
        mode === "installed"
          ? await installBuilt(root, directory, ["instrument-form"])
          : [];
      const entry =
        mode === "installed"
          ? "./components/finstack/components/instrument-form/instrument-form"
          : path.join(
              root,
              "registry/components/instrument-form/instrument-form.tsx",
            );
      await writeFile(
        path.join(directory, "main.tsx"),
        `import {useState} from 'react';import{createRoot}from'react-dom/client';import{InstrumentForm}from ${JSON.stringify(entry)};const validate=async json=>json;function App(){const[type,setType]=useState('bond');return <main className="mx-auto max-w-4xl p-4"><h1 className="text-xl">Instrument catalogue</h1><output aria-label="Selected type">{type}</output><InstrumentForm type={type} onTypeChange={setType} validate={validate} onSubmit={()=>{}}/></main>;}createRoot(document.getElementById('root')).render(<App/>);`,
      );
      const theme =
        mode === "installed"
          ? "./styles/finstack/theme.css"
          : path.join(root, "registry/theme/finstack-theme/theme.css");
      await writeFile(
        path.join(directory, "app.css"),
        `@import "tailwindcss" source(none);\n@import ${JSON.stringify(theme)};\n@source "./main.tsx";\n@source "./components";\n@source ${JSON.stringify(path.join(root, "registry"))};\n`,
      );
      await writeFile(
        path.join(directory, "index.html"),
        '<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>Instrument catalogue</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>',
      );
      let chunks;
      await build({
        root: directory,
        configFile: false,
        logLevel: "error",
        resolve: {
          alias:
            mode === "installed"
              ? { "@": directory }
              : {
                  "@/lib/finstack/form": path.join(
                    root,
                    "registry/lib/finstack-form/form.tsx",
                  ),
                  "@/lib/finstack": path.join(root, "src"),
                },
        },
        css: { postcss: { plugins: [tailwind()] } },
        plugins: [
          {
            name: "catalogue-proof",
            transform(code, id) {
              if (id.endsWith("/codec.mjs"))
                return code.replace(
                  "export function createWireCodec(source) {",
                  "export function createWireCodec(source) { (globalThis.__codecRoots ??= []).push(source.title);",
                );
            },
            generateBundle(_, bundle) {
              chunks = Object.values(bundle)
                .filter((chunk) => chunk.type === "chunk")
                .map((chunk) => ({
                  file: chunk.fileName,
                  entry: chunk.isEntry,
                  imports: chunk.imports,
                  dynamic: chunk.dynamicImports,
                  modules: Object.keys(chunk.modules),
                }));
            },
          },
        ],
        build: { outDir: path.join(directory, "dist"), emptyOutDir: true },
      });
      const byFile = new Map(chunks.map((chunk) => [chunk.file, chunk]));
      const closure = new Set();
      function visit(file) {
        if (closure.has(file)) return;
        closure.add(file);
        for (const imported of byFile.get(file)?.imports ?? []) visit(imported);
      }
      for (const chunk of chunks.filter((chunk) => chunk.entry))
        visit(chunk.file);
      const initial = [...closure].flatMap(
        (file) => byFile.get(file)?.modules ?? [],
      );
      for (const item of inventory)
        for (const kind of ["instrument", "schemas", "examples"])
          assert(
            !initial.some((id) =>
              id.includes(`/generated/${kind}/${item.type}.`),
            ),
            `${mode}: ${item.type} ${kind} in initial static closure`,
          );
      assert(chunks.some((chunk) => chunk.dynamic.length));
      server = await serveExport(path.join(directory, "dist"));
      const page = await browser.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.goto(server.url, { waitUntil: "networkidle" });
      const input = page.getByRole("combobox", { name: "Instrument type" });
      const seen = [];
      for (const type of ["bond", "equity", "commodity_option", "bond"]) {
        if (type !== "bond" || seen.length) {
          await input.fill(type);
          await page.getByRole("option", { name: type, exact: true }).click();
        }
        await page.waitForFunction(
          (id) =>
            document.querySelector(
              '[data-field-path="instrument.spec.id"] input',
            )?.value === id,
          inventory.find((item) => item.type === type).exampleId,
        );
        seen.push(type);
      }
      const converted = await page.evaluate(() => globalThis.__codecRoots);
      const selected = Object.fromEntries(
        inventory
          .map((item) => [
            item.type,
            converted.filter((name) => name === item.type).length,
          ])
          .filter(([, count]) => count),
      );
      assert.deepEqual(selected, { commodity_option: 1, equity: 1, bond: 1 });
      assert.deepEqual(
        await page.evaluate(() =>
          JSON.parse(localStorage.getItem("finstack.recent-instruments")),
        ),
        ["bond", "commodity_option", "equity"],
      );
      await page.addScriptTag({
        path: path.join(root, "node_modules/axe-core/axe.min.js"),
      });
      const audit = await page.evaluate(() => window.axe.run(document));
      assert.deepEqual(audit.violations, []);
      assert.deepEqual(errors, []);
      reports.push({
        mode,
        installed,
        inventory: inventory.length,
        initialStaticChunks: closure.size,
        instrumentSchemasOrExamplesInInitial: [],
        selectedCodecCounts: selected,
        repeatSelectionReusesModule: true,
        violations: audit.violations,
        errors,
      });
      await page.close();
    } finally {
      await server?.close();
      await rm(directory, { recursive: true, force: true });
    }
  }
  const report = { browser: browser.version(), reports, verdict: "pass" };
  await writeFile(
    "/tmp/pr025-catalogue.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser.close();
}
