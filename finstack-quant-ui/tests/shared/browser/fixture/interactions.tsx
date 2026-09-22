import { createRoot } from "react-dom/client";
import { useRef } from "react";
import { LinkedFigureExample } from "./components/finstack/shared/chart/figure-example/linked-figures";
import type { FigureHandle } from "./components/finstack/shared/chart/finstack-chart/finstack-chart";
function App() {
  const figure = useRef<FigureHandle>(null);
  (window as any).exportInteraction = async () =>
    (
      await figure.current!.exportSvg({
        width: 900,
        height: 480,
        theme: "light",
      })
    ).text();
  return (
    <main>
      <h1>Chart interactions</h1>
      <LinkedFigureExample ref={figure} />
      <LinkedFigureExample />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
