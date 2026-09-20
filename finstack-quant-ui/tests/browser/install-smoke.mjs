import { spawn } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { createHash } from "node:crypto";
import path from "node:path";
import { installProbe } from "./install-probe.mjs";
const repo = path.resolve(import.meta.dirname, "../../..");
const ui = path.join(repo, "finstack-quant-ui");
const external = await mkdtemp(path.join(tmpdir(), "finstack-next-installed-"));
const evidence =
  process.env.REGISTRY_SMOKE_DIR ??
  (await mkdtemp(path.join(tmpdir(), "pr017-install-")));
console.log(`External consumer: ${external}\nEvidence: ${evidence}`);
const run = (command, args, cwd, env = {}) =>
  new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      stdio: "inherit",
      env: {
        ...process.env,
        npm_config_ignore_scripts: "true",
        npm_config_install_links: "true",
        npm_config_save_exact: "true",
        npm_config_cache: "/tmp/component-registry-npm-cache",
        ...env,
      },
    });
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`${command} failed: ${code}`)),
    );
  });
await writeFile(
  path.join(external, "package.json"),
  JSON.stringify(
    {
      name: "finstack-installed-consumer",
      private: true,
      type: "module",
      dependencies: {
        next: "16.3.4",
        react: "19.2.8",
        "react-dom": "19.2.8",
        "finstack-quant-wasm": `file:${path.join(repo, "finstack-quant-wasm")}`,
      },
      devDependencies: {
        typescript: "5.9.3",
        "@types/node": "24.0.0",
        "@types/react": "19.2.18",
        "@types/react-dom": "19.2.5",
        tailwindcss: "4.3.3",
        "@tailwindcss/postcss": "4.3.3",
      },
    },
    null,
    2,
  ),
);
await mkdir(path.join(external, "app"));
await writeFile(
  path.join(external, "app/globals.css"),
  '@import "tailwindcss";\n',
);
const config = JSON.parse(
  await readFile(path.join(ui, "components.json"), "utf8"),
);
config.tailwind.css = "app/globals.css";
await writeFile(
  path.join(external, "components.json"),
  JSON.stringify(config, null, 2),
);
await writeFile(
  path.join(external, "tsconfig.json"),
  JSON.stringify(
    {
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
        jsx: "preserve",
        paths: { "@/*": ["./*"] },
      },
      include: ["**/*.ts", "**/*.tsx"],
      exclude: ["node_modules", "out"],
    },
    null,
    2,
  ),
);
await cp(
  path.join(repo, "docs-site/postcss.config.mjs"),
  path.join(external, "postcss.config.mjs"),
);
await writeFile(
  path.join(external, "next.config.mjs"),
  'export default { output: "export", trailingSlash: true, basePath: process.env.NEXT_PUBLIC_BASE_PATH || "", turbopack: { root: import.meta.dirname } };\n',
);
await run("npm", ["install", "--ignore-scripts", "--install-links"], external);
await run(
  process.execPath,
  [
    path.join(ui, "scripts/install-local.mjs"),
    external,
    ...(process.env.REGISTRY_INSTALL_COMPOSITION === "1"
      ? ["curve-link-example"]
      : []),
  ],
  repo,
);
await writeFile(
  path.join(external, "app/globals.css"),
  '@import "tailwindcss";\n@import "../styles/finstack/theme.css";\n',
);
await writeFile(
  path.join(external, "app/layout.tsx"),
  'import "./globals.css"; export default function Layout({ children }: {children: React.ReactNode}) { return <html lang="en"><body>{children}</body></html>; }\n',
);
await cp(
  path.join(ui, "src/fixtures/results/bond.json"),
  path.join(external, "app/bond.json"),
);
await writeFile(
  path.join(external, "app/page.tsx"),
  '"use client";\nimport { PricingWorkbench } from "@/components/finstack/blocks/pricing-workbench/pricing-workbench";\nimport { FinstackQueryProvider } from "@/hooks/use-finstack/use-finstack";\nimport fixture from "./bond.json";\nexport default function Page() { return <main className="finstack-surface mx-auto max-w-6xl p-4"><h1>Bond pricing</h1><FinstackQueryProvider><PricingWorkbench defaultRequest={fixture.request} /></FinstackQueryProvider></main>; }\n',
);
const hash = createHash("sha256")
  .update(
    await readFile(
      path.join(repo, "finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm"),
    ),
  )
  .digest("hex");
// The copied package must be independent of workspace links and retain the optimized artifact.
const installedHash = createHash("sha256")
  .update(
    await readFile(
      path.join(
        external,
        "node_modules/finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
      ),
    ),
  )
  .digest("hex");
if (installedHash !== hash) throw new Error("Installed WASM artifact changed");
if (process.env.REGISTRY_INSTALL_COMPOSITION === "1") {
  await mkdir(path.join(external, "app/registry-selection"));
  await writeFile(
    path.join(external, "app/registry-selection/page.tsx"),
    '"use client";import {CurveLinkExample} from "@/components/finstack/components/curve-link-example/curve-link-example";export default function Page(){return <main className="finstack-surface"><h1>Independent selection composition</h1><CurveLinkExample/></main>}',
  );
}
await installProbe(external);
process.env.REGISTRY_TEST_PROBE = "1";
for (const [consumer, route, build] of [
  [
    "docs",
    "/docs/registry/workbench/",
    () =>
      run(process.execPath, [path.join(ui, "scripts/build-docs.mjs")], repo),
  ],
  [
    "external",
    "/",
    () =>
      run(
        process.execPath,
        [path.join(external, "node_modules/next/dist/bin/next"), "build"],
        external,
      ),
  ],
]) {
  for (const [deployment, basePath] of [
    ["root", ""],
    ["base-path", "/finstack-quant"],
  ]) {
    process.env.NEXT_PUBLIC_BASE_PATH = basePath;
    await build();
    const app =
      consumer === "docs"
        ? path.join(repo, "docs-site/.registry-publish")
        : external;
    await run(
      process.execPath,
      [path.join(import.meta.dirname, "workbench.mjs")],
      repo,
      {
        REGISTRY_EXPORT_DIR: path.join(app, "out"),
        REGISTRY_WORKBENCH_PATH: route,
        REGISTRY_SCOPED_DOCS: consumer === "docs" ? "1" : "0",
        REGISTRY_WORKBENCH_REPORT: path.join(
          evidence,
          `${consumer}-${deployment}.json`,
        ),
        REGISTRY_EXPECTED_WASM_SHA256: hash,
      },
    );
    if (
      consumer === "external" &&
      process.env.REGISTRY_INSTALL_COMPOSITION === "1"
    )
      await run(
        process.execPath,
        [path.join(ui, "tests/install/composition.mjs")],
        repo,
        {
          REGISTRY_EXPORT_DIR: path.join(app, "out"),
          REGISTRY_SELECTION_REPORT: path.join(
            evidence,
            `selection-${deployment}.json`,
          ),
        },
      );
    await run(
      process.execPath,
      [path.join(import.meta.dirname, "smoke.mjs")],
      repo,
      {
        REGISTRY_EXPORT_DIR: path.join(app, "out"),
        REGISTRY_SMOKE_REPORT: path.join(
          evidence,
          `${consumer}-${deployment}-worker.json`,
        ),
        REGISTRY_EXPECTED_WASM_SHA256: hash,
      },
    );
  }
}
console.log(`Installed production checks passed: ${evidence}`);
