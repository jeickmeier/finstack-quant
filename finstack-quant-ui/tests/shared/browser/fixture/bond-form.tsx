import { createRoot } from "react-dom/client";
import { useEffect, useState } from "react";
import {
  FinstackProvider,
  useFinstack,
} from "./hooks/shared/use-finstack/use-finstack";
import { useInstrumentValidator } from "./hooks/valuations/use-instrument-validator/use-instrument-validator";
import {
  SchemaForm,
  type InstrumentModule,
} from "./components/finstack/core/components/schema-form/schema-form";
function Form() {
  const [module, setModule] = useState<InstrumentModule | null>(null);
  const [output, setOutput] = useState("");
  const worker = useFinstack();
  const validate = useInstrumentValidator();
  useEffect(() => {
    void import("./lib/finstack/generated/instrument/bond").then(setModule);
  }, []);
  return (
    <main className="mx-auto max-w-5xl space-y-4 p-4">
      <h1>Bond term sheet</h1>
      <p role="status">Worker: {worker.status}</p>
      {worker.error && <p role="alert">{worker.error.message}</p>}
      {module && (
        <SchemaForm module={module} validate={validate} onSubmit={setOutput} />
      )}
      <label>
        Canonical output
        <textarea aria-label="Canonical output" readOnly value={output} />
      </label>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <FinstackProvider>
    <Form />
  </FinstackProvider>,
);
