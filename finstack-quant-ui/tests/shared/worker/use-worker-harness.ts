import { afterAll, beforeAll, beforeEach, vi } from "vitest";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { createClient } from "./client";
import { startWorker } from "./harness.mjs";

type Harness = Awaited<ReturnType<typeof startWorker>>;
type Route = (...args: unknown[]) => Promise<never>;

/**
 * Serves the mocked worker client (see `./client`) from the production worker
 * service for the current test file: `initialize` and the listed methods reach
 * the worker and unwrap as the real client does; any other method throws.
 * `"any"` routes every method. `call` is reset before each test, so an
 * implementation a test sets lasts for that test only.
 * @param methods - Worker methods the file's components may call, or `"any"`.
 * @returns The live worker proxy, the client's `call` mock and the methods called so far in the current test.
 */
export function useWorkerHarness(
  methods: readonly (keyof WorkerApi)[] | "any",
) {
  let harness: Harness | undefined;
  const call = vi.fn(async (method: string, ...args: unknown[]) => {
    if (
      methods !== "any" &&
      method !== "initialize" &&
      !methods.includes(method as keyof WorkerApi)
    )
      throw new Error(`Unexpected worker call ${method}`);
    const proxy = harness!.proxy as unknown as Record<string, Route>;
    return unwrap(await proxy[method]!(...args));
  });
  beforeAll(async () => {
    harness = await startWorker();
  });
  afterAll(async () => harness?.close());
  beforeEach(() => {
    vi.stubGlobal("Worker", class {});
    call.mockReset();
    createClient.mockReturnValue({ call, close: vi.fn() });
  });
  return {
    get proxy() {
      return harness!.proxy;
    },
    call,
    methods: () => call.mock.calls.map(([method]) => method),
  };
}
