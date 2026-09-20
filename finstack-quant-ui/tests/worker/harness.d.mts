import type { Worker } from "node:worker_threads";
import type { Remote } from "comlink";
import type { WorkerApi } from "../../registry/workers/finstack-contract";
export function startWorker(options?: { fail?: boolean }): Promise<{
  proxy: Remote<WorkerApi>;
  worker: Worker;
  close(): Promise<void>;
}>;
