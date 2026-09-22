"use client";
import { useEffect, useState, type ReactNode } from "react";
import { useSearchParams } from "next/navigation";
import { useIsFetching } from "@tanstack/react-query";
import {
  FinstackQueryProvider,
  useFinstack,
} from "@/hooks/shared/use-finstack/use-finstack";
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
    instrument = search.get("instrument") ?? undefined,
    publication = variant === "publication",
    focused = search.get("focus") === "1",
    width = search.get("width") === "narrow" ? 360 : publication ? 1000 : 880;
  const item = inventory.visual.find((item) => item.name === name);
  const block = item?.type === "registry:block";
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.dataset.density = density;
  }, [theme, density]);
  const href = (item: string, nextTheme = theme, nextDensity = density) =>
    `?${new URLSearchParams({ item, theme: nextTheme, density: nextDensity, ...(focused ? { focus: "1" } : {}), ...(variant !== "default" ? { variant } : {}), ...(item === "pricing-workbench" && instrument ? { instrument } : {}) })}`;
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
        instrument={instrument}
      />
    )
  ) : null;
  return (
    <main
      id="main-content"
      className={`finstack-surface not-prose min-h-screen bg-background font-sans text-sm text-foreground ${focused ? "" : "p-4"}`}
      data-theme={theme}
      data-density={density}
      data-registry-focus={focused ? "" : undefined}
    >
      <header
        className={
          focused
            ? "hidden"
            : "mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-4"
        }
      >
        <h1 className="text-lg font-semibold">
          finstack{" "}
          <span className="font-normal text-muted-foreground">
            / component registry
          </span>
        </h1>

        <nav aria-label="Gallery state" className="flex flex-wrap gap-4">
          <a href={href(name, "light", density)}>Light</a>
          <a href={href(name, "dark", density)}>Dark</a>
          <a href={href(name, theme, "compact")}>Compact</a>
          <a href={href(name, theme, "comfortable")}>Comfortable</a>
          <a href={href("nonvisual")}>Nonvisual harnesses</a>
        </nav>
      </header>
      <div
        className={
          focused
            ? ""
            : "grid items-start gap-5 lg:grid-cols-[190px_minmax(0,1fr)]"
        }
      >
        <nav
          aria-label="Registry tiers"
          className={
            focused ? "hidden" : "max-h-[84vh] overflow-auto space-y-5 text-xs"
          }
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
              <h2 className="mb-2 font-medium text-muted-foreground">
                {group.title}
              </h2>
              <ul>
                {inventory.visual
                  .filter((item) => group.types.includes(item.type))
                  .map((item) => (
                    <li key={item.name}>
                      <a
                        className={`block border-l-2 px-3 py-1.5 ${name === item.name ? "border-primary bg-accent text-primary" : "border-transparent hover:bg-muted"}`}
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
        <div className="min-w-0">
          {name === "nonvisual" ? (
            <Imports />
          ) : item ? (
            <section
              key={`${name}/${variant}/${density}`}
              data-item={name}
              data-state={`${theme}-${density}`}
              className={focused ? "bg-background" : "space-y-3 bg-background"}
              style={{
                width: block || focused ? "100%" : width + 34,
                maxWidth: "100%",
              }}
            >
              {!focused && (
                <header className="space-y-2 pb-2">
                  <div className="flex items-center justify-between gap-3">
                    <h2 className="text-lg font-semibold">{name}</h2>
                    <a
                      className="rounded-sm border border-border px-3 py-1 text-xs"
                      href={`${href(name)}&focus=1`}
                    >
                      Open focused preview ↗
                    </a>
                  </div>
                  <p className="max-w-4xl text-xs text-muted-foreground">
                    {item.docs}
                  </p>
                </header>
              )}
              <div
                className={
                  block
                    ? "min-w-0 border border-border"
                    : "max-h-[1100px] overflow-auto border border-border p-4"
                }
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
