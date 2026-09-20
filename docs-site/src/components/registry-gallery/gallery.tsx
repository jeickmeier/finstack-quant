"use client";
import { useEffect, useState, type ReactNode } from "react";
import { useSearchParams } from "next/navigation";
import { useIsFetching } from "@tanstack/react-query";
import {
  FinstackQueryProvider,
  useFinstack,
} from "@/hooks/use-finstack/use-finstack";
import { PrimitiveDemo } from "./primitives";
import { ComponentDemo, nativeItems } from "./components";
import { importHarnesses } from "./imports";
import inventory from "./inventory.json";
function Ready({ children }: { children: ReactNode }) {
  const worker = useFinstack(),
    fetching = useIsFetching();
  return (
    <div data-worker-state={worker.status} data-fetching={fetching}>
      {worker.error && <p role="alert">{worker.error.message}</p>}
      {children}
    </div>
  );
}
function Imports() {
  const [results, setResults] = useState<{ name: string; modules: number }[]>(
      [],
    ),
    [error, setError] = useState<string | null>(null);
  return (
    <section data-nonvisual>
      <h2>Nonvisual import harnesses</h2>
      <p>
        Every nonvisual item is imported from consumer-owned targets. Worker
        entry execution is covered by the native visual harnesses; base
        configuration is checked through this installed consumer.
      </p>
      <button
        onClick={() =>
          void Promise.all(Object.values(importHarnesses).map((load) => load()))
            .then(setResults)
            .catch((error) => setError(String(error)))
        }
      >
        Import all nonvisual items
      </button>
      {error && <p role="alert">{error}</p>}
      <output aria-label="Imported item count">{results.length}</output>
      <ul>
        {inventory.nonvisual.map((item) => (
          <li
            key={item.name}
            data-import-item={item.name}
            data-imported={results.some((result) => result.name === item.name)}
          >
            {item.name}:{" "}
            {results.some((result) => result.name === item.name)
              ? "Imported"
              : "Not run"}
          </li>
        ))}
      </ul>
    </section>
  );
}
export function RegistryGallery() {
  const search = useSearchParams(),
    name = search.get("item") ?? "field-frame",
    theme = search.get("theme") === "dark" ? "dark" : "light",
    density =
      search.get("density") === "comfortable" ? "comfortable" : "compact",
    variant = search.get("variant") ?? "default",
    publication = variant === "publication",
    width = search.get("width") === "narrow" ? 360 : publication ? 1000 : 880;
  const item = inventory.visual.find((item) => item.name === name);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.density = density;
  }, [theme, density]);
  const href = (item: string, nextTheme = theme, nextDensity = density) =>
    `?${new URLSearchParams({ item, theme: nextTheme, density: nextDensity })}`;
  const content = item ? (
    item.type === "registry:ui" || item.type === "registry:theme" ? (
      <PrimitiveDemo
        name={name}
        density={density}
        width={width}
        publication={publication}
      />
    ) : (
      <ComponentDemo
        name={name}
        density={density}
        width={width}
        variant={variant}
      />
    )
  ) : null;
  return (
    <main
      id="main-content"
      className="finstack-surface min-h-screen bg-background p-4 font-sans text-foreground"
      data-theme={theme}
      data-density={density}
    >
      <header className="mb-6 space-y-3">
        <h1 className="text-2xl font-semibold">Component registry gallery</h1>
        <p>
          Installed components with explicit fixtures. Light/dark and
          compact/comfortable states share the same public APIs.
        </p>
        <nav aria-label="Gallery state" className="flex flex-wrap gap-4">
          <a href={href(name, "light", density)}>Light</a>
          <a href={href(name, "dark", density)}>Dark</a>
          <a href={href(name, theme, "compact")}>Compact</a>
          <a href={href(name, theme, "comfortable")}>Comfortable</a>
          <a href={href("nonvisual")}>Nonvisual harnesses</a>
        </nav>
      </header>
      <div className="grid items-start gap-6 lg:grid-cols-[220px_minmax(0,1fr)]">
        <nav
          aria-label="Registry tiers"
          className="max-h-[80vh] overflow-auto space-y-4"
        >
          {[
            {
              title: "Primitive Components",
              types: ["registry:ui", "registry:theme"],
            },
            { title: "Individual Components", types: ["registry:component"] },
            { title: "Blocks", types: ["registry:block"] },
          ].map((group) => (
            <section key={group.title}>
              <h2 className="font-semibold">{group.title}</h2>
              <ul>
                {inventory.visual
                  .filter((item) => group.types.includes(item.type))
                  .map((item) => (
                    <li key={item.name}>
                      <a
                        aria-current={name === item.name ? "page" : undefined}
                        href={href(item.name)}
                      >
                        {item.name}
                      </a>
                    </li>
                  ))}
              </ul>
            </section>
          ))}
        </nav>
        <div className="min-w-0 overflow-auto">
          {name === "nonvisual" ? (
            <Imports />
          ) : item ? (
            <section
              key={`${name}/${variant}/${density}`}
              data-item={name}
              data-state={`${theme}-${density}`}
              className="space-y-4 border border-border bg-background p-4"
              style={{ width: width + 34, maxWidth: "100%" }}
            >
              <h2 className="text-xl font-semibold">{name}</h2>
              <p className="text-sm text-muted-foreground">{item.docs}</p>
              <div
                className="max-h-[1100px] overflow-auto"
                data-preview
                tabIndex={0}
                role="region"
                aria-label={`${name} preview`}
              >
                {nativeItems.has(name) ? (
                  <FinstackQueryProvider>
                    <Ready>{content}</Ready>
                  </FinstackQueryProvider>
                ) : (
                  content
                )}
              </div>
            </section>
          ) : (
            <p role="alert">Unknown visual item: {name}</p>
          )}
        </div>
      </div>
    </main>
  );
}
