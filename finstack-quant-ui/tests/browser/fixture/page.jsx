"use client";
import { useEffect, useState } from "react";
import {
  FinstackProvider,
  useFinstack,
} from "../../hooks/use-finstack/use-finstack";

function Probe() {
  const worker = useFinstack();
  const [started] = useState(() => performance.now());
  useEffect(() => {
    const proxy = Object.fromEntries(
      [
        "price",
        "cashflows",
        "exportResult",
        "models",
        "metrics",
        "calendars",
        "validate",
      ].map((method) => [
        method,
        (...args) =>
          worker.client
            .call(method, ...args)
            .then((value) => ({ ok: true, value }))
            .catch((error) => ({ ok: false, error: error.payload })),
      ]),
    );
    window.registryProbe = {
      proxy,
      state: worker.status === "error" ? "failed" : worker.status,
      initialization:
        worker.status === "ready"
          ? { ok: true, value: { state: "ready", worker: true } }
          : worker.error
            ? { ok: false, error: worker.error.payload }
            : undefined,
      initializationMs: performance.now() - started,
      instrumentTypes: async () =>
        (await import("./footprint.js")).instrumentTypes(),
      measureInstrument: async (type) =>
        (await import("./footprint.js")).measureInstrument(type),
    };
    return () => {
      delete window.registryProbe;
    };
  }, [worker.status, worker.client, worker.error, started]);
  return (
    <main id="main-content">
      <h1>Component registry worker probe</h1>
      <output data-testid="worker-state">{worker.status}</output>
    </main>
  );
}
export default function WorkerProbe() {
  const [config, setConfig] = useState(null);
  useEffect(() => {
    setConfig({
      wasmUrl: new URLSearchParams(location.search).get("wasm") ?? undefined,
    });
  }, []);
  return config ? (
    <FinstackProvider wasmUrl={config.wasmUrl}>
      <Probe />
    </FinstackProvider>
  ) : (
    <main id="main-content">Starting worker…</main>
  );
}
