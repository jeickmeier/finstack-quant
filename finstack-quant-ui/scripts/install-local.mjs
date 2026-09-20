import { spawn } from "node:child_process";
import {
  readFile,
  writeFile,
  mkdtemp,
  readdir,
  rm,
  mkdir,
  cp,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { serveExport } from "../tests/browser/static-server.mjs";

// Exercise the actual pinned installer against a fresh local registry build.
const root = fileURLToPath(new URL("../", import.meta.url));
const consumer = path.resolve(process.argv[2]);
const configPath = path.join(consumer, "components.json");
const original = await readFile(configPath, "utf8");
const isDocs = consumer === path.resolve(root, "../docs-site");
const cssPath = path.join(consumer, JSON.parse(original).tailwind.css);
const originalCss = isDocs ? await readFile(cssPath, "utf8") : null;
const staged = await mkdtemp(path.join(tmpdir(), "finstack-local-registry-"));
// The npm release is not published yet. Only dependency transport changes;
// registry source content and the pinned dependency version remain untouched.
const wasm = path.resolve(root, "../finstack-quant-wasm");
for (const name of await readdir(path.join(root, "public/r"))) {
  if (!name.endsWith(".json")) continue;
  const item = JSON.parse(
    await readFile(path.join(root, "public/r", name), "utf8"),
  );
  if (item.dependencies)
    item.dependencies = item.dependencies.map((value) =>
      value === "finstack-quant-wasm@0.8.0" ? `file:${wasm}` : value,
    );
  await writeFile(path.join(staged, name), JSON.stringify(item));
}
const server = await serveExport(staged);
try {
  const config = JSON.parse(original);
  config.registries["@finstack"] = `${server.url}/{name}.json`;
  await writeFile(configPath, JSON.stringify(config, null, 2) + "\n");
  await new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [
        path.join(root, "node_modules/shadcn/dist/index.js"),
        "add",
        "@finstack/pricing-workbench",
        "--yes",
        "--overwrite",
        "--cwd",
        consumer,
      ],
      {
        cwd: consumer,
        stdio: "inherit",
        env: {
          ...process.env,
          npm_config_ignore_scripts: "true",
          npm_config_save_exact: "true",
          npm_config_cache: "/tmp/component-registry-npm-cache",
        },
      },
    );
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`shadcn add exited ${code}`)),
    );
  });
  if (isDocs) {
    await mkdir(path.join(consumer, "public"), { recursive: true });
    await rm(path.join(consumer, "public/r"), { recursive: true, force: true });
    await cp(path.join(root, "public/r"), path.join(consumer, "public/r"), {
      recursive: true,
    });
  }
} finally {
  // The docs host already imports the theme file, which owns its tokens.
  if (originalCss !== null) await writeFile(cssPath, originalCss);
  await writeFile(configPath, original);
  await server.close();
  await rm(staged, { recursive: true, force: true });
}
