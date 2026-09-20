import { spawn } from "node:child_process";
import { mkdtemp, rename, rm, cp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
const repo = fileURLToPath(new URL("../../", import.meta.url));
const stage = await mkdtemp(join(tmpdir(), "finstack-web-"));
const pkg = join(stage, "pkg");
const run = (command, args) =>
  new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: repo, stdio: "inherit" });
    child.once("error", reject);
    child.once("exit", (code) =>
      code === 0 ? resolve() : reject(new Error(`${command} failed: ${code}`)),
    );
  });
try {
  await run("wasm-pack", [
    "build",
    "finstack-quant-wasm",
    "--target",
    "web",
    "--profile",
    "release-size",
    "--no-opt",
    "--out-dir",
    pkg,
  ]);
  const binary = join(pkg, "finstack_quant_wasm_bg.wasm");
  const raw = join(stage, "raw.wasm");
  await rename(binary, raw);
  await run("wasm-opt", [
    "-Oz",
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "--enable-mutable-globals",
    "--enable-simd",
    raw,
    "-o",
    binary,
  ]);
  await run(process.execPath, [
    "finstack-quant-ui/tests/browser/size.mjs",
    raw,
    binary,
    join(pkg, "size-report.json"),
  ]);
  await writeFile(join(pkg, ".npmignore"), "");
  const output = resolve(repo, "finstack-quant-wasm/pkg");
  await rm(output, { recursive: true, force: true });
  await cp(pkg, output, { recursive: true });
} finally {
  await rm(stage, { recursive: true, force: true });
}
