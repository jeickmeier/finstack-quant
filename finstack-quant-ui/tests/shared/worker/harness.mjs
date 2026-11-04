import { Worker } from "node:worker_threads";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { inject } from "vitest";
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
  const worker = new Worker(inject("workerBundle"), {
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
  const proxy = wrap(nodeEndpoint(worker));
  try {
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
    proxy[releaseProxy]();
    await worker.terminate();
  }
}
