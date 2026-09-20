import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, writeFile, rm } from "node:fs/promises";
import path from "node:path";
import { build } from "vite";
import tailwind from "@tailwindcss/postcss";
import { chromium } from "playwright";
import { serveExport } from "./static-server.mjs";
import { shadcnItems, copyShadcn } from "../../scripts/shadcn.mjs";

/** Build the installed closure with the same alias and dependency instrumentation. */
export async function buildConsumer(consumer, { title, html, sources = [] }) {
  await writeFile(
    path.join(consumer, "app.css"),
    '@import "tailwindcss" source(none);\n@import "./styles/finstack/theme.css";\n' +
      ["./index.html", "./main.tsx", "./components", ...sources]
        .map((source) => `@source "${source}";\n`)
        .join(""),
  );
  await writeFile(
    path.join(consumer, "index.html"),
    html ??
      `<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>${title}</title><link rel="stylesheet" href="/app.css"></head><body class="finstack-surface"><div id="root"></div><script type="module" src="/main.tsx"></script></body></html>`,
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
  return modules;
}

/** Open a built consumer, releasing partial startup resources on failure. */
export async function openConsumer(directory, { viewport, basePath = "" }) {
  let server, browser;
  try {
    server = await serveExport(directory, basePath);
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport });
    return { server, browser, page };
  } catch (error) {
    await closeConsumer({ browser, server });
    throw error;
  }
}

/** Release every owned resource even if another resource fails to close. */
export async function closeConsumer({ browser, server }, ...directories) {
  try {
    await browser?.close();
  } finally {
    try {
      await server?.close();
    } finally {
      await Promise.all(
        directories.map((directory) =>
          rm(directory, { recursive: true, force: true }),
        ),
      );
    }
  }
}
/** Extract a fresh pinned CLI build and its declared closure into consumer-owned targets. */
export async function installBuilt(root, consumer, names) {
  const output = path.join(consumer, "r");
  await promisify(execFile)(
    process.execPath,
    [
      path.join(root, "node_modules/shadcn/dist/index.js"),
      "build",
      "--output",
      output,
    ],
    { cwd: root },
  );
  const installed = new Set();
  async function install(name) {
    if (installed.has(name)) return;
    if (shadcnItems.has(name)) {
      await copyShadcn(root, consumer, [name]);
      installed.add(name);
      return;
    }
    const item = JSON.parse(
      await readFile(path.join(output, `${name}.json`), "utf8"),
    );
    installed.add(name);
    for (const dependency of item.registryDependencies ?? [])
      await install(dependency.replace("@finstack/", ""));
    for (const file of item.files ?? []) {
      const target = path.join(consumer, file.target.replace(/^~\//, ""));
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, file.content);
    }
  }
  for (const name of names) await install(name);
  return [...installed];
}
