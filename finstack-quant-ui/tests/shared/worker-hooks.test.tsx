// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import {
  render,
  renderHook,
  screen,
  cleanup,
  waitFor,
  act,
  fireEvent,
} from "@testing-library/react";
import { renderToString } from "react-dom/server";
import {
  FinstackProvider,
  FinstackQueryProvider,
  useFinstack,
} from "@/hooks/shared/use-finstack/use-finstack";
import { useInstrumentValidator } from "@/hooks/valuations/use-instrument-validator/use-instrument-validator";
import { usePriceInstrument } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import { FinstackError } from "@/workers/finstack-contract";
const mocks = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mocks.createClient,
}));
let call: ReturnType<typeof vi.fn>, close: ReturnType<typeof vi.fn>;
let construct = vi.fn(() => {});
beforeEach(() => {
  construct = vi.fn();
  vi.stubGlobal(
    "Worker",
    class {
      constructor() {
        construct();
      }
    },
  );
  call = vi.fn(async () => undefined);
  close = vi.fn();
  mocks.createClient.mockReset().mockReturnValue({ call, close });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
function Status() {
  const worker = useFinstack();
  return (
    <>
      <output>{worker.status}</output>
      <span>{worker.error?.message}</span>
      <button onClick={worker.reset}>Reset</button>
    </>
  );
}
it("does not construct or initialize a worker while rendering on the server", () => {
  expect(
    renderToString(
      <FinstackProvider>
        <Status />
      </FinstackProvider>,
    ),
  ).toContain("starting");
  expect(construct).not.toHaveBeenCalled();
});
it("shares one worker, exposes ready state and replaces/cleans it on reset/unmount", async () => {
  const view = render(
    <FinstackProvider>
      <Status />
      <Status />
    </FinstackProvider>,
  );
  await waitFor(() => expect(screen.getAllByText("ready")).toHaveLength(2));
  expect(construct).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getAllByRole("button", { name: "Reset" })[0]);
  await waitFor(() => expect(construct).toHaveBeenCalledTimes(2));
  expect(close).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(close).toHaveBeenCalledTimes(2);
});
it("exposes initialization and later worker crashes as errors", async () => {
  call.mockRejectedValueOnce(
    new FinstackError({
      name: "InitError",
      message: "bad asset",
      kind: "init",
    }),
  );
  render(
    <FinstackProvider>
      <Status />
    </FinstackProvider>,
  );
  await screen.findByText("bad asset");
  expect(screen.getByText("error")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Reset" }));
  await screen.findByText("ready");
  act(() =>
    mocks.createClient.mock.calls.at(-1)![1](
      new FinstackError({
        name: "WorkerError",
        message: "worker crashed",
        kind: "worker",
      }),
    ),
  );
  expect(screen.getByText("worker crashed")).toBeTruthy();
});
it("returns canonical text and rejects a superseded validation response", async () => {
  const hook = renderHook(
    () => ({ validate: useInstrumentValidator(), worker: useFinstack() }),
    {
      wrapper: ({ children }) => (
        <FinstackProvider>{children}</FinstackProvider>
      ),
    },
  );
  await waitFor(() => expect(hook.result.current.worker.status).toBe("ready"));
  call.mockResolvedValueOnce("canonical text");
  expect(await hook.result.current.validate("original text")).toBe(
    "canonical text",
  );
  expect(call).toHaveBeenLastCalledWith("validate", "original text");
  let finish!: (value: string) => void;
  call.mockImplementationOnce(
    () =>
      new Promise<string>((resolve) => {
        finish = resolve;
      }),
  );
  const controller = new AbortController();
  const pending = hook.result.current.validate("superseded", controller.signal);
  const rejected = expect(pending).rejects.toMatchObject({
    name: "AbortError",
  });
  controller.abort();
  finish("old canonical text");
  await rejected;
  call.mockRejectedValueOnce(new Error("native validation failure"));
  await expect(hook.result.current.validate("invalid")).rejects.toThrow(
    "native validation failure",
  );
});
it("transport cleanup and worker errors reject in-flight Comlink requests", async () => {
  const { createClient } = await vi.importActual<
    typeof import("@/hooks/shared/use-finstack/client")
  >("@/hooks/shared/use-finstack/client");
  class Endpoint extends EventTarget {
    postMessage() {}
    terminate = vi.fn();
  }
  const endpoint = new Endpoint();
  const client = createClient(endpoint as unknown as Worker);
  const pending = client.call("calendars");
  const rejected = expect(pending).rejects.toMatchObject({
    name: "WorkerClosedError",
  });
  client.close();
  await rejected;
  expect(endpoint.terminate).toHaveBeenCalledOnce();
  const other = new Endpoint();
  const failure = vi.fn();
  const next = createClient(other as unknown as Worker, failure);
  const request = next.call("calendars");
  const failed = expect(request).rejects.toMatchObject({
    name: "WorkerError",
    message: "crash",
  });
  other.dispatchEvent(new ErrorEvent("error", { message: "crash" }));
  await failed;
  expect(failure).toHaveBeenCalledOnce();
  next.close();
});

it("warns once per returned mismatched WASM version without changing valuations", async () => {
  const { createClient } = await vi.importActual<
    typeof import("@/hooks/shared/use-finstack/client")
  >("@/hooks/shared/use-finstack/client");
  const { wasmVersion } = await import("@/hooks/shared/use-finstack/version");
  let result = {
    meta: { version: wasmVersion },
    value: { amount: "123456789.125", currency: "USD" },
  };
  class Endpoint extends EventTarget {
    postMessage(message: { id: string }) {
      queueMicrotask(() =>
        this.dispatchEvent(
          new MessageEvent("message", {
            data: {
              id: message.id,
              type: "RAW",
              value: { ok: true, value: result },
            },
          }),
        ),
      );
    }
    terminate() {}
  }
  const client = createClient(new Endpoint() as unknown as Worker);
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  const request = {
    instrumentJson: "{}",
    marketJson: "{}",
    asOf: "2025-01-01",
  };
  try {
    expect(await client.call("price", request)).toEqual(result);
    expect(warn).not.toHaveBeenCalled();
    result = { ...result, meta: { version: "0.0.0-mismatch" } };
    expect(await client.call("price", request)).toEqual(result);
    await client.call("price", request);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]![0]).toContain(wasmVersion);
    expect(warn.mock.calls[0]![0]).toContain("0.0.0-mismatch");
  } finally {
    client.close();
    warn.mockRestore();
  }
});
it("shares query readiness, session isolation and explicit disabling", async () => {
  const request = {
    instrumentJson: "{}",
    marketJson: "{}",
    asOf: "2025-01-01",
  };
  const value = {
    instrument_id: "POLICY",
    value: { amount: "1.00", currency: "USD" },
  };
  call.mockImplementation(async (method: string) =>
    method === "price" ? value : undefined,
  );
  const hook = renderHook(
    ({ enabled }) => ({
      query: usePriceInstrument(request, enabled),
      worker: useFinstack(),
    }),
    {
      initialProps: { enabled: false },
      wrapper: ({ children }) => (
        <FinstackQueryProvider>{children}</FinstackQueryProvider>
      ),
    },
  );
  await waitFor(() =>
    expect(hook.result.current.query.workerStatus).toBe("ready"),
  );
  expect(call.mock.calls.filter(([method]) => method === "price")).toHaveLength(
    0,
  );
  hook.rerender({ enabled: true });
  await waitFor(() => expect(hook.result.current.query.data).toEqual(value));
  expect(call.mock.calls.filter(([method]) => method === "price")).toHaveLength(
    1,
  );
  const session = hook.result.current.worker.session;
  act(() => hook.result.current.worker.reset());
  await waitFor(() =>
    expect(hook.result.current.worker.session).not.toBe(session),
  );
  await waitFor(() =>
    expect(
      call.mock.calls.filter(([method]) => method === "price"),
    ).toHaveLength(2),
  );
});
