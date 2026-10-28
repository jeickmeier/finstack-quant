import { mkdtemp, rm } from "node:fs/promises";
import { Worker } from "node:worker_threads";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { build } from "vite";
import { wrap, releaseProxy } from "comlink";
import nodeEndpoint from "comlink/dist/esm/node-adapter.mjs";

/** Production Comlink service in a real worker, over the published facade initialized from the web WASM bytes. */
export async function startWorker({
  fail = false,
  failMarketSerialization = false,
} = {}) {
  const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  const directory = await mkdtemp(path.join(root, ".worker-test-"));
  let worker;
  let proxy;
  try {
    await build({
      root,
      configFile: false,
      logLevel: "error",
      resolve: {
        alias: {
          "@/lib/finstack": path.join(root, "src"),
          "@/workers": path.join(root, "registry/shared/file"),
        },
      },
      build: {
        ssr: true,
        target: "node24",
        outDir: directory,
        emptyOutDir: false,
        rolldownOptions: { output: { entryFileNames: "node.mjs" } },
        lib: {
          entry: path.join(root, "tests/shared/worker/node.mjs"),
          formats: ["es"],
          fileName: () => "node.mjs",
        },
      },
    });
    worker = new Worker(path.join(directory, "node.mjs"), {
      workerData: {
        fail,
        failMarketSerialization,
        facadePath: path.resolve(root, "../finstack-quant-wasm/index.js"),
        wasmPath: path.resolve(
          root,
          "../finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
        ),
      },
    });
    proxy = wrap(nodeEndpoint(worker));
    const initialized = await Promise.race([
      proxy.initialize(),
      new Promise((_, reject) => worker.once("error", reject)),
    ]);
    if (!initialized.ok && !fail) throw new Error(initialized.error.message);
    return { proxy, worker, close };
  } catch (error) {
    await close();
    throw error;
  }
  async function close() {
    proxy?.[releaseProxy]();
    await worker?.terminate();
    await rm(directory, { recursive: true, force: true });
  }
}
