// @vitest-environment jsdom
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import {
  render,
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
} from "../registry/hooks/use-finstack/use-finstack";
import { useValidateInstrument } from "../registry/hooks/use-validate-instrument/use-validate-instrument";
import { FinstackError } from "../registry/workers/finstack-contract";
const mocks = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("../registry/hooks/use-finstack/client", () => ({
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
it("debounces native validation and hides stale results/errors on any newer revision", async () => {
  const pending: {
    resolve: (value: unknown) => void;
    reject: (error: Error) => void;
  }[] = [];
  call.mockImplementation((method: string) =>
    method === "initialize"
      ? Promise.resolve()
      : new Promise((resolve, reject) => pending.push({ resolve, reject })),
  );
  function Validation({ text, revision }: { text: string; revision: number }) {
    const state = useValidateInstrument({ instrumentJson: text, revision });
    return (
      <output>{state.error?.message ?? state.data?.json ?? "pending"}</output>
    );
  }
  const wrapper = ({ text, revision }: { text: string; revision: number }) => (
    <FinstackQueryProvider>
      <Validation text={text} revision={revision} />
    </FinstackQueryProvider>
  );
  const view = render(wrapper({ text: "first", revision: 1 }));
  await waitFor(() => expect(pending).toHaveLength(1));
  expect(call).toHaveBeenLastCalledWith("validate", {
    instrumentJson: "first",
    revision: 1,
  });
  view.rerender(wrapper({ text: "second", revision: 2 }));
  await act(async () => pending[0].reject(new Error("stale failure")));
  expect(screen.queryByText("stale failure")).toBeNull();
  await waitFor(() => expect(pending).toHaveLength(2));
  await act(async () =>
    pending[1].resolve({ json: "validated second", revision: 2 }),
  );
  await screen.findByText("validated second");
  view.rerender(wrapper({ text: "second", revision: 3 }));
  expect(screen.queryByText("validated second")).toBeNull();
});
it("transport cleanup and worker errors reject in-flight Comlink requests", async () => {
  const { createClient } = await vi.importActual<
    typeof import("../registry/hooks/use-finstack/client")
  >("../registry/hooks/use-finstack/client");
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
    typeof import("../registry/hooks/use-finstack/client")
  >("../registry/hooks/use-finstack/client");
  const { wasmVersion } =
    await import("../registry/hooks/use-finstack/version");
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
