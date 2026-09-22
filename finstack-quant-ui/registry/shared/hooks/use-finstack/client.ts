import { wrap, releaseProxy } from "comlink";
import { wasmVersion } from "./version";
import {
  FinstackError,
  errorValue,
  unwrap,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
type Value<K extends keyof WorkerApi> =
  Awaited<ReturnType<WorkerApi[K]>> extends Envelope<infer T> ? T : never;
/** Browser transport with rejection on worker failure/cleanup, including in-flight calls. */
export function createClient(
  worker: Worker,
  onFailure?: (error: FinstackError) => void,
) {
  const proxy = wrap<WorkerApi>(worker);
  let closed = false;
  const warnedVersions = new Set<string>();
  let rejectFailure: (error: Error) => void;
  const failure = new Promise<never>((_, reject) => {
    rejectFailure = reject;
  });
  void failure.catch(() => {});
  const fail = (event: ErrorEvent | MessageEvent) => {
    const error = new FinstackError({
      name: "WorkerError",
      message:
        "message" in event
          ? event.message
          : "Worker message could not be decoded",
      kind: "worker",
    });
    rejectFailure(error);
    onFailure?.(error);
  };
  worker.addEventListener("error", fail);
  worker.addEventListener("messageerror", fail);
  return {
    async call<K extends keyof WorkerApi>(
      method: K,
      ...args: Parameters<WorkerApi[K]>
    ): Promise<Value<K>> {
      if (closed) throw new Error("Worker client is closed");
      try {
        const invoke = proxy[method] as (
          ...args: Parameters<WorkerApi[K]>
        ) => Promise<Envelope<Value<K>>>;
        const result = unwrap(await Promise.race([invoke(...args), failure]));
        if (method === "price") {
          const version = (result as Value<"price">).meta?.version;
          if (
            typeof version === "string" &&
            version !== wasmVersion &&
            !warnedVersions.has(version)
          ) {
            warnedVersions.add(version);
            console.warn(
              `Finstack registry expects WASM ${wasmVersion}, but the valuation reports ${version}. Install the matching WASM package and re-add the registry items.`,
            );
          }
        }
        return result;
      } catch (error) {
        throw error instanceof FinstackError
          ? error
          : new FinstackError(errorValue(error));
      }
    },
    close() {
      if (closed) return;
      closed = true;
      rejectFailure(
        new FinstackError({
          name: "WorkerClosedError",
          message: "Worker client was closed",
          kind: "worker",
        }),
      );
      worker.removeEventListener("error", fail);
      worker.removeEventListener("messageerror", fail);
      proxy[releaseProxy]();
      // Terminating discards the worker's WASM instance and all market handles, even during a long call.
      worker.terminate();
    },
  };
}
export type FinstackClient = ReturnType<typeof createClient>;
