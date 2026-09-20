"use client";
import { useEffect, useState } from "react";
import { wrap, releaseProxy } from "comlink";

export default function WorkerProbe() {
  const [state, setState] = useState("starting");
  useEffect(() => {
    const worker = new Worker(new URL("./probe.worker.js", import.meta.url), {
      type: "module",
    });
    const proxy = wrap(worker);
    const failed = (event) => {
      window.registryProbe.state = "failed";
      window.registryProbe.initialization = {
        ok: false,
        error: { name: "WorkerError", message: event.message, kind: "worker" },
      };
      setState("failed");
    };
    worker.addEventListener("error", failed);
    const wasmUrl =
      new URLSearchParams(location.search).get("wasm") ?? undefined;
    window.registryProbe = {
      proxy,
      state: "starting",
      instrumentTypes: async () =>
        (await import("./footprint.js")).instrumentTypes(),
      measureInstrument: async (type) =>
        (await import("./footprint.js")).measureInstrument(type),
    };
    const initializationStarted = performance.now();
    proxy
      .initialize(wasmUrl)
      .then((value) => {
        window.registryProbe.initializationMs =
          performance.now() - initializationStarted;
        window.registryProbe.initialization = value;
        window.registryProbe.state = value.ok ? "ready" : "failed";
        setState(window.registryProbe.state);
      })
      .catch((error) => {
        window.registryProbe.state = "failed";
        window.registryProbe.initialization = {
          ok: false,
          error: {
            name: error.name,
            message: error.message,
            kind: "transport",
          },
        };
        setState("failed");
      });
    return () => {
      delete window.registryProbe;
      proxy[releaseProxy]();
      worker.terminate();
    };
  }, []);
  return (
    <main id="main-content">
      <h1>Component registry worker probe</h1>
      <output data-testid="worker-state">{state}</output>
    </main>
  );
}
