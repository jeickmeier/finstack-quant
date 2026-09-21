import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { loadRegistry } from "shadcn/registry";
import { chromium } from "playwright";
import { checkGraph } from "./check-registry.mjs";
import { serveLocalRegistry } from "./local-registry.mjs";
import { serveExport } from "../tests/browser/static-server.mjs";
import { verifyItem } from "../tests/install/browser.mjs";
import { visualHarness } from "../tests/install/harness.mjs";
const root = fileURLToPath(new URL("../", import.meta.url)),
  repo = path.resolve(root, "..");
const registry = await loadRegistry({ cwd: root }),
  graph = checkGraph(registry.items);
const requested = process.env.REGISTRY_INSTALL_ITEMS?.split(",");
if (requested && process.env.REGISTRY_INSTALL_COMPLETE === "1")
  throw Error("The complete ui-install gate cannot use a subset");
const concurrency = Number(process.env.REGISTRY_INSTALL_CONCURRENCY ?? 1);
if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 4)
  throw Error("Install concurrency must be an integer from 1 to 4");
if (requested?.some((name) => !graph.byName.has(name)))
  throw Error("Unknown matrix item");
const items = registry.items.filter(
  (item) => !requested || requested.includes(item.name),
);
const evidence =
  process.env.REGISTRY_INSTALL_EVIDENCE ??
  (await mkdtemp(path.join(tmpdir(), "finstack-install-evidence-")));
await mkdir(evidence, { recursive: true });
console.log(`Independent install evidence: ${evidence}`);
const transport = await serveLocalRegistry(root),
  browser = await chromium.launch({ headless: true }),
  results = [];
const visualTypes = new Set([
  "registry:ui",
  "registry:component",
  "registry:block",
  "registry:theme",
]);
const workerVisuals = new Set([
  "fx-surface-chart",
  "vol-cube-explorer",
  "pricing-workbench",
]);
const run = (args, cwd, log) =>
  new Promise((resolve, reject) => {
    const output = createWriteStream(log, { flags: "a" });
    const child = spawn(args[0], args.slice(1), {
      cwd,
      env: {
        ...process.env,
        NEXT_TELEMETRY_DISABLED: "1",
        npm_config_ignore_scripts: "true",
        npm_config_install_links: "true",
        npm_config_save_exact: "true",
        npm_config_cache: "/tmp/component-registry-npm-cache",
      },
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 600000,
    });
    child.stdout.pipe(output, { end: false });
    child.stderr.pipe(output, { end: false });
    child.once("error", (error) => {
      output.end();
      reject(error);
    });
    child.once("close", (code) => {
      output.end();
      code === 0
        ? resolve()
        : reject(Error(`${args[0]} ${args[1]} exited ${code}; see ${log}`));
    });
  });
const json = (file, value) =>
  writeFile(file, JSON.stringify(value, null, 2) + "\n");
let reportWrite = Promise.resolve();
async function installItem(index, item) {
  const consumer = await mkdtemp(
    path.join(tmpdir(), `finstack-item-${item.name}-`),
  );
  const log = path.join(evidence, `${item.name}.log`),
    started = Date.now(),
    closure = [...graph.closures.get(item.name)];
  const files = closure.flatMap((name) => graph.byName.get(name).files ?? []);
  const needsWasm = closure
    .flatMap((name) => graph.byName.get(name).dependencies ?? [])
    .some((name) => name.startsWith("finstack-quant-wasm@"));
  let server, context;
  console.log(
    `[${index + 1}/${items.length}] ${item.name}: fresh scaffold and one-item CLI install`,
  );
  try {
    await mkdir(path.join(consumer, "app"));
    await json(path.join(consumer, "package.json"), {
      name: `installed-${item.name}`,
      private: true,
      type: "module",
      dependencies: {
        next: "16.3.4",
        react: "19.2.8",
        "react-dom": "19.2.8",
        "@base-ui/react": "1.8.0",
        ...(needsWasm
          ? { "finstack-quant-wasm": `file:${transport.wasm}` }
          : {}),
      },
      devDependencies: {
        typescript: "5.9.3",
        "@types/node": "24.10.1",
        "@types/react": "19.2.14",
        "@types/react-dom": "19.2.3",
        tailwindcss: "4.3.3",
        "@tailwindcss/postcss": "4.3.3",
      },
    });
    await json(path.join(consumer, "tsconfig.json"), {
      compilerOptions: {
        target: "ES2022",
        lib: ["dom", "dom.iterable", "esnext"],
        allowJs: true,
        skipLibCheck: true,
        strict: true,
        noEmit: true,
        esModuleInterop: true,
        module: "esnext",
        moduleResolution: "bundler",
        resolveJsonModule: true,
        isolatedModules: true,
        jsx: "react-jsx",
        paths: { "@/*": ["./*"] },
      },
      include: ["**/*.ts", "**/*.tsx"],
      exclude: ["node_modules", "out"],
    });
    await writeFile(
      path.join(consumer, "app/globals.css"),
      '@import "tailwindcss";\n',
    );
    await writeFile(
      path.join(consumer, "app/page.tsx"),
      "export default function Page(){return <main>Initializing Base UI</main>}",
    );
    const layout =
      'import "./globals.css";export default function Layout({children}:{children:React.ReactNode}){return <html lang="en" data-theme="light" data-density="compact"><body>{children}</body></html>}';
    await writeFile(path.join(consumer, "app/layout.tsx"), layout);
    await writeFile(
      path.join(consumer, "next.config.mjs"),
      'export default {output:"export",trailingSlash:true,productionBrowserSourceMaps:true,basePath:process.env.NEXT_PUBLIC_BASE_PATH||"",turbopack:{root:import.meta.dirname}};\n',
    );
    await cp(
      path.join(repo, "docs-site/postcss.config.mjs"),
      path.join(consumer, "postcss.config.mjs"),
    );
    await run(
      ["npm", "install", "--ignore-scripts", "--install-links"],
      consumer,
      log,
    );
    const cli = path.join(root, "node_modules/shadcn/dist/index.js");
    await run(
      [
        process.execPath,
        cli,
        "init",
        "--base",
        "base",
        "--preset",
        "nova",
        "--yes",
        "--no-monorepo",
        "--no-rtl",
        "--no-pointer",
        "--cwd",
        consumer,
      ],
      consumer,
      log,
    );
    const config = JSON.parse(
      await readFile(path.join(consumer, "components.json"), "utf8"),
    );
    config.registries = { "@finstack": `${transport.url}/{name}.json` };
    await json(path.join(consumer, "components.json"), config);
    await run(
      [
        process.execPath,
        cli,
        "add",
        `@finstack/${item.name}`,
        "--yes",
        "--overwrite",
        "--cwd",
        consumer,
      ],
      consumer,
      log,
    );
    const targets = [];
    for (const file of files) {
      const target = file.target.replace(/^~\//, "");
      const content = await readFile(path.join(consumer, target));
      targets.push({
        target,
        owner: graph.owners.get(file.target),
        sha256: createHash("sha256").update(content).digest("hex"),
      });
    }
    if (
      needsWasm &&
      !(
        await realpath(path.join(consumer, "node_modules/finstack-quant-wasm"))
      ).startsWith((await realpath(consumer)) + path.sep)
    )
      throw Error("WASM dependency must be copied, not a workspace symlink");
    await writeFile(
      path.join(consumer, "app/globals.css"),
      '@import "tailwindcss";\n' +
        (closure.includes("finstack-theme")
          ? '@import "../styles/finstack/theme.css";\n'
          : ""),
    );
    await writeFile(path.join(consumer, "app/layout.tsx"), layout);
    const visual = visualTypes.has(item.type);
    if (visual) {
      const harness = await visualHarness(repo, item),
        source = harness.source;
      await writeFile(path.join(consumer, "app/harness.tsx"), source);
      for (const [name, text] of Object.entries(harness.extra)) {
        await mkdir(path.dirname(path.join(consumer, "app", name)), {
          recursive: true,
        });
        await writeFile(path.join(consumer, "app", name), text);
      }
      await cp(
        path.join(repo, "docs-site/src/components/registry-gallery/data.json"),
        path.join(consumer, "app/data.json"),
      );
      const native = workerVisuals.has(item.name);
      await writeFile(
        path.join(consumer, "app/page.tsx"),
        `"use client";import {InstalledItem} from "./harness";${native ? 'import {FinstackQueryProvider} from "@/hooks/use-finstack/use-finstack";' : ""}export default function Page(){return <main className="finstack-surface" style={{padding:24,width:960}} data-installed-item=${JSON.stringify(item.name)}><h1>${item.name}</h1>${native ? "<FinstackQueryProvider>" : ""}<InstalledItem/>${native ? "</FinstackQueryProvider>" : ""}</main>}`,
      );
    } else {
      const modules = (item.files ?? [])
        .map((file) => file.target.replace(/^~\//, ""))
        .filter(
          (target) =>
            /\.(?:[cm]?js|tsx?|json)$/.test(target) &&
            !target.endsWith(".d.ts") &&
            !target.endsWith(".worker.ts"),
        );
      await writeFile(
        path.join(consumer, "app/page.tsx"),
        `"use client";import {useEffect,useState} from "react";export default function Page(){const [status,setStatus]=useState("Loading");useEffect(()=>{Promise.all([${modules.map((target) => `import(${JSON.stringify("@/" + target.replace(/\.tsx?$/, ""))})`).join(",")}]).then(()=>setStatus("Ready"),error=>setStatus(String(error)))},[]);return <main data-installed-item=${JSON.stringify(item.name)}><h1>${item.name}</h1><output aria-label="Import state">{status}</output></main>}`,
      );
    }
    console.log(
      `[${index + 1}/${items.length}] ${item.name}: type-check and production build`,
    );
    await run(
      [
        process.execPath,
        path.join(consumer, "node_modules/typescript/bin/tsc"),
        "--noEmit",
      ],
      consumer,
      log,
    );
    await run(
      [
        process.execPath,
        path.join(consumer, "node_modules/next/dist/bin/next"),
        "build",
      ],
      consumer,
      log,
    );
    server = await serveExport(path.join(consumer, "out"));
    context = await browser.newContext({
      viewport: { width: 1200, height: 1200 },
    });
    const page = await context.newPage(),
      errors = [],
      requests = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("response", (response) => {
      requests.push(response.url());
      if (response.status() >= 400)
        errors.push(`${response.status()} ${response.url()}`);
    });
    await page.goto(server.url, { waitUntil: "networkidle" });
    await page.locator(`[data-installed-item="${item.name}"]`).waitFor();
    if (!visual)
      await page
        .getByLabel("Import state")
        .filter({ hasText: "Ready" })
        .waitFor();
    if (visual) {
      await page.waitForFunction(
        () =>
          ![...document.querySelectorAll('[role="status"]')].some((node) =>
            /Loading|Evaluating|Waiting|Pricing…/.test(node.textContent ?? ""),
          ),
      );
      await page.evaluate(() => document.fonts.ready);
      await page.screenshot({
        path: path.join(evidence, `${item.name}.png`),
        fullPage: true,
      });
    }
    const checks = await verifyItem(page, item, evidence, repo);
    if (
      item.name === "pricing-workbench" &&
      process.env.REGISTRY_INSTALL_PUBLISHING === "1"
    ) {
      const { verifyPublishing } =
        await import("../tests/publishing/browser.mjs");
      checks.push(...(await verifyPublishing(page, evidence, repo)));
    }
    if (errors.length) throw Error(JSON.stringify(errors));
    const alerts = (
      await page.locator('[role="alert"]').allTextContents()
    ).filter((text) => text.trim());
    if (alerts.length)
      throw Error(`Installed harness displayed an error: ${alerts.join("; ")}`);
    const wasmRequests = requests.filter((url) => url.endsWith(".wasm"));
    if (
      !workerVisuals.has(item.name) &&
      item.name !== "pricing-forms-example" &&
      wasmRequests.length
    )
      throw Error("Data/presentation-only item initialized WASM");
    const sources = new Set();
    for (const url of requests.filter((url) =>
      new URL(url).pathname.endsWith(".js"),
    )) {
      try {
        const script = await readFile(
          path.join(consumer, "out", decodeURIComponent(new URL(url).pathname)),
          "utf8",
        );
        const reference = script.match(/\/\/# sourceMappingURL=(\S+)/)?.[1];
        if (!reference) continue;
        const map = JSON.parse(
          await readFile(
            path.join(
              consumer,
              "out",
              decodeURIComponent(new URL(reference, url).pathname),
            ),
            "utf8",
          ),
        );
        for (const source of map.sources ?? []) sources.add(source);
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }
    }
    if (
      item.name === "instrument-catalogue" &&
      [...sources].some((source) => source.includes("generated/schemas/"))
    )
      throw Error("Lazy instrument catalogue eagerly loaded schema payloads");
    if (!sources.size)
      throw Error("No production source maps available for runtime inspection");
    if (
      item.name === "finstack-table" &&
      [...sources].some((source) => source.includes("@tanstack/charts"))
    )
      throw Error("Table-only consumer imported charts");
    if (
      item.name === "finstack-chart" &&
      [...sources].some((source) =>
        /\/@tanstack\/(?:react-)?table/.test(source),
      )
    )
      throw Error("Chart-only consumer imported tables");
    if (
      item.name === "use-linked-selection" &&
      [...sources].some((source) =>
        /\/@tanstack\/(?:react-)?(?:table|charts)/.test(source),
      )
    )
      throw Error("Selection helper imported a visual runtime");
    results.push({
      name: item.name,
      verdict: "pass",
      durationMs: Date.now() - started,
      closure,
      targets,
      wasmRequests: wasmRequests.length,
      loadedSourceCount: sources.size,
      checks,
    });
    await context.close();
    context = undefined;
    await server.close();
    server = undefined;
    await rm(consumer, { recursive: true, force: true });
  } catch (error) {
    results.push({
      name: item.name,
      verdict: "fail",
      durationMs: Date.now() - started,
      error: String(error),
      consumer,
      log,
    });
    console.error(`${item.name}: ${error}`);
    if (process.env.REGISTRY_INSTALL_FAIL_FAST === "1") stop = true;
  } finally {
    await context?.close();
    await server?.close();
    const report = {
      scope: requested ? "subset" : "complete catalogue",
      concurrency,
      catalogueCount: registry.items.length,
      requestedCount: items.length,
      completedCount: results.length,
      results: [...results].sort(
        (a, b) =>
          items.findIndex((item) => item.name === a.name) -
          items.findIndex((item) => item.name === b.name),
      ),
    };
    reportWrite = reportWrite.then(() =>
      json(path.join(evidence, "results.json"), report),
    );
    await reportWrite;
  }
}
let next = 0,
  stop = false;
try {
  await Promise.all(
    Array.from({ length: concurrency }, async () => {
      while (!stop && next < items.length) {
        const index = next++;
        await installItem(index, items[index]);
      }
    }),
  );
} finally {
  await browser.close();
  await transport.close();
}
const failures = results.filter((result) => result.verdict !== "pass");
console.log(
  `${results.length - failures.length}/${items.length} isolated items passed. Evidence: ${evidence}`,
);
if (failures.length || results.length !== items.length) process.exitCode = 1;
