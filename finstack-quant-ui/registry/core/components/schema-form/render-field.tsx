"use client";
import { useContext, useEffect, useState, useRef, type ReactNode } from "react";
import { useStore } from "@tanstack/react-form";
import { Fieldset } from "@/lib/finstack/form";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { ScalarField } from "./field-kind";
import { SheetContext, revealKey } from "./disclosure";
import { FieldRendererContext } from "./field-renderer";
import { discriminator } from "./discriminator";
import type { InstrumentModule, SchemaLocation } from "./schema";
import { propertiesOf } from "./schema";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { initialValue, nullable, objectValue, resolve } from "./schema-walk";
import type { SchemaFormApi, FieldFilter } from "./schema-form";
import { nullableFieldChrome, optionalFieldChrome } from "./field-chrome";

function TermDisclosure({
  path,
  label,
  count,
  forceOpen,
  children,
}: {
  /** Canonical path of the object that owns the grouped properties. */
  path: string;
  label: string;
  count: number;
  forceOpen: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const detailsRef = useRef<HTMLDetailsElement>(null);
  const summaryRef = useRef<HTMLElement>(null);
  const focusedError = useRef(false);
  const { revealers } = useContext(SheetContext);
  useEffect(() => {
    const key = revealKey(path, label);
    revealers.set(key, () => {
      const details = detailsRef.current;
      if (!details) return;
      details.open = true;
      summaryRef.current?.focus();
      details.scrollIntoView?.({ block: "nearest" });
    });
    return () => {
      revealers.delete(key);
    };
  }, [path, label, revealers]);
  useEffect(() => {
    if (!forceOpen) {
      focusedError.current = false;
      return;
    }
    if (focusedError.current) return;
    focusedError.current = true;
    detailsRef.current
      ?.querySelector<HTMLElement>('[aria-invalid="true"]')
      ?.focus();
  }, [forceOpen]);
  return (
    <details
      ref={detailsRef}
      className="finstack-term-disclosure"
      open={open || forceOpen}
      onBeforeToggle={(event) => {
        if (forceOpen && event.newState === "closed") event.preventDefault();
      }}
      onToggle={(event) => {
        if (forceOpen && !event.currentTarget.open) {
          event.currentTarget.open = true;
          return;
        }
        const next = event.currentTarget.open;
        setOpen((current) => (current === next ? current : next));
      }}
    >
      <summary ref={summaryRef}>{`${label} (${count})`}</summary>
      {children}
    </details>
  );
}

/** Shared renderer over generated structure; the module codec owns validation. */
export function RenderField({
  module,
  form,
  location,
  path,
  label,
  value,
  required = true,
  layout,
  fields,
  skipOverride = false,
  sectionless = false,
}: {
  module: InstrumentModule;
  form: SchemaFormApi;
  location: SchemaLocation;
  path: string;
  label: string;
  value: unknown;
  required?: boolean;
  layout: "basic" | "full";
  fields?: FieldFilter;
  skipOverride?: boolean;
  sectionless?: boolean;
}) {
  const placement = useRef(new Map<string, Map<string, string | null>>());
  const fieldMeta = useStore(form.store, (state) => state.fieldMeta);
  const sheet = useContext(SheetContext);
  const render = useContext(FieldRendererContext);
  const within = (child: string, parent: string) =>
    child === parent ||
    child.startsWith(`${parent}.`) ||
    child.startsWith(`${parent}[`);
  const denied = (fieldPath: string) =>
    fields?.deny?.some((key) => within(fieldPath, key)) ?? false;
  if (denied(path)) return null;
  if (
    fields?.allow &&
    path &&
    !fields.allow.some((key) => within(key, path) || within(path, key))
  )
    return null;
  const resolved = resolve(module.schema, location);
  const { schema, pointer } = resolved;
  const metadata = module.metadata.find((meta) => meta.path === pointer);
  const help = schema.description ?? metadata?.description;
  const info = { label, help };
  const nonNull = nullable(module.schema, resolved);
  const renderField = (next: Parameters<typeof RenderField>[0]) => (
    <RenderField {...next} />
  );
  const nullableChrome = nullableFieldChrome({
    module,
    form,
    location,
    path,
    label,
    value,
    required,
    layout,
    fields,
    skipOverride,
    sectionless,
    nonNull,
    renderField,
  });
  if (nullableChrome) return nullableChrome;
  if (Object.hasOwn(schema, "const")) return null;
  const custom = skipOverride
    ? undefined
    : render?.({
        form,
        location: resolved,
        path,
        label,
        value,
        renderDefault: () => (
          <RenderField
            {...{
              module,
              form,
              location,
              path,
              label,
              value,
              required,
              layout,
              fields,
            }}
            skipOverride
          />
        ),
      });
  if (custom !== undefined) return custom;
  const optionalChrome = optionalFieldChrome({
    module,
    form,
    location,
    path,
    label,
    value,
    required,
    nonNull,
    layout,
    fields,
    skipOverride,
    sectionless,
    renderField,
  });
  if (optionalChrome) return optionalChrome;
  const union = discriminator(module.schema, resolved);
  if (union) {
    const selected = union.selected(value);
    return (
      <Fieldset {...info}>
        <EnumField
          label={`${label} type`}
          value={selected < 0 ? "" : String(selected)}
          options={union.branches.map((branch, index) => ({
            value: String(index),
            label: labelFor(branch.label),
            description: branch.schema.description,
          }))}
          onValueChange={(index) =>
            form.setFieldValue(path, union.switchTo(Number(index), value))
          }
        />
        {selected >= 0 && (
          <RenderField
            {...{
              module,
              form,
              path,
              label,
              value,
              layout,
              fields,
              skipOverride,
            }}
            sectionless
            location={union.branches[selected]}
          />
        )}
      </Fieldset>
    );
  }
  const enumVariants =
    schema.enum ??
    (schema.oneOf ?? schema.anyOf)?.flatMap(
      (node) => node.enum ?? (Object.hasOwn(node, "const") ? [node.const] : []),
    );
  if (
    enumVariants?.length &&
    enumVariants.every((value) => typeof value === "string")
  )
    return (
      <form.AppField name={path}>
        {(field) => (
          <field.SelectField
            {...info}
            options={enumVariants.map((value) => ({
              value: String(value),
              label: String(value),
              description: (schema.oneOf ?? schema.anyOf)?.find(
                (node) => node.enum?.includes(value) || node.const === value,
              )?.description,
            }))}
          />
        )}
      </form.AppField>
    );
  if (
    schema.type === "object" &&
    typeof schema.additionalProperties === "object"
  ) {
    const valueSchema = schema.additionalProperties;
    return (
      <form.AppField name={path} mode="array">
        {(field) => (
          <field.ArrayField
            label={label}
            create={() => ({
              key: "",
              value: initialValue(module.schema, {
                schema: valueSchema,
                pointer: `${pointer}/additionalProperties`,
              }),
            })}
          >
            {(index) => (
              <div className="finstack-term-fields">
                <form.AppField name={`${path}[${index}].key`}>
                  {(keyField) => (
                    <keyField.TextField label={`${label} key ${index + 1}`} />
                  )}
                </form.AppField>
                <RenderField
                  {...{ module, form, layout, fields }}
                  location={{
                    schema: valueSchema,
                    pointer: `${pointer}/additionalProperties`,
                  }}
                  path={`${path}[${index}].value`}
                  label={`${label} value ${index + 1}`}
                  value={
                    Array.isArray(value)
                      ? objectValue(value[index]).value
                      : undefined
                  }
                />
              </div>
            )}
          </field.ArrayField>
        )}
      </form.AppField>
    );
  }
  if (schema.type === "object") {
    const properties = propertiesOf(schema);
    const requiredOrder = schema.required ?? [];
    const order = (key: string) => {
      if (key === "id") return -1;
      const index = requiredOrder.indexOf(key);
      return index < 0 ? requiredOrder.length : index;
    };
    properties.sort(([a], [b]) => order(a) - order(b));
    const empty = (entry: unknown): boolean =>
      entry == null ||
      (Array.isArray(entry)
        ? entry.length === 0
        : typeof entry === "object" && Object.values(entry).every(empty));
    const valueIsSecondary = ([key, node]: (typeof properties)[number]) => {
      const current = objectValue(value)[key];
      if (
        Object.hasOwn(node, "default") &&
        (Object.is(current, node.default) ||
          (typeof node.default === "number" &&
            current === String(node.default)) ||
          (empty(current) && empty(node.default)))
      )
        return true;
      if (!empty(current)) return false;
      if (!schema.required?.includes(key)) return true;
      return (
        Boolean(
          nullable(module.schema, {
            schema: node,
            pointer: `${pointer}/properties/${key}`,
          }),
        ) ||
        (current != null && typeof current === "object")
      );
    };
    const siblingKeys = properties.map(([key]) => key);
    let placed = placement.current.get(pointer);
    if (!placed) {
      placed = new Map();
      placement.current.set(pointer, placed);
    }
    for (const entry of properties) {
      const key = entry[0];
      if (placed.has(key)) continue;
      placed.set(
        key,
        layout === "basic"
          ? sheet.disclosure.group({
              path,
              key,
              siblingKeys,
              secondary: valueIsSecondary(entry),
            })
          : null,
      );
    }
    const fieldHasError = (key: string) => {
      const fieldPath = path ? `${path}.${key}` : key;
      return Object.entries(fieldMeta).some(
        ([name, meta]) =>
          within(name, fieldPath) && Boolean(meta?.errors.length),
      );
    };
    const render = ([key, node]: (typeof properties)[number]) => {
      const childPath = path ? `${path}.${key}` : key;
      return (
        <RenderField
          key={key}
          sectionless={sectionless && properties.length === 1}
          {...{ module, form, layout, fields }}
          location={{ schema: node, pointer: `${pointer}/properties/${key}` }}
          path={childPath}
          label={
            childPath === sheet.root?.path ? sheet.root.label : labelFor(key)
          }
          value={objectValue(value)[key]}
          required={schema.required?.includes(key) ?? false}
        />
      );
    };
    if (!path || sheet.root?.path.startsWith(`${path}.`))
      return <div className="min-w-0">{properties.map(render)}</div>;
    // Denied fields render nothing, so they neither count in a group nor keep it alive.
    const shown = properties.filter(
      ([key]) => !denied(path ? `${path}.${key}` : key),
    );
    const primary = shown.filter(([key]) => placed.get(key) == null);
    const buckets = new Map<string, (typeof properties)[number][]>();
    for (const entry of shown) {
      const group = placed.get(entry[0]);
      if (!group) continue;
      const list = buckets.get(group) ?? [];
      list.push(entry);
      buckets.set(group, list);
    }
    const groupOrder = sheet.disclosure.order;
    const groupNames = [
      ...groupOrder.filter((name) => buckets.has(name)),
      ...[...buckets.keys()].filter((name) => !groupOrder.includes(name)),
    ];
    const contents = (
      <>
        <div className="finstack-term-fields">{primary.map(render)}</div>
        {groupNames.map((name) => {
          const entries = buckets.get(name) ?? [];
          return (
            <TermDisclosure
              key={name}
              path={path}
              label={name}
              count={entries.length}
              forceOpen={entries.some(([key]) => fieldHasError(key))}
            >
              <div className="finstack-term-fields">{entries.map(render)}</div>
            </TermDisclosure>
          );
        })}
      </>
    );
    if (sectionless) return <div className="min-w-0">{contents}</div>;
    return (
      <Fieldset
        {...info}
        className={
          path === sheet.root?.path ? "finstack-schema-root" : undefined
        }
      >
        {contents}
      </Fieldset>
    );
  }
  if (schema.type === "array" && schema.prefixItems)
    return (
      <Fieldset {...info}>
        {schema.prefixItems.map((node, index) => (
          <RenderField
            key={index}
            {...{ module, form, layout, fields }}
            location={{
              schema: node,
              pointer: `${pointer}/prefixItems/${index}`,
            }}
            path={`${path}[${index}]`}
            label={node.title ?? `${label} ${index + 1}`}
            value={Array.isArray(value) ? value[index] : undefined}
          />
        ))}
      </Fieldset>
    );
  if (schema.type === "array")
    return (
      <form.AppField name={path} mode="array">
        {(field) => (
          <field.ArrayField
            label={label}
            create={() =>
              initialValue(module.schema, {
                schema: schema.items ?? {},
                pointer: `${pointer}/items`,
              })
            }
          >
            {(index) => (
              <RenderField
                {...{ module, form, layout, fields }}
                location={{
                  schema: schema.prefixItems?.[index] ?? schema.items ?? {},
                  pointer: `${pointer}/items`,
                }}
                path={`${path}[${index}]`}
                label={`${label} ${index + 1}`}
                value={Array.isArray(value) ? value[index] : undefined}
              />
            )}
          </field.ArrayField>
        )}
      </form.AppField>
    );
  return (
    <ScalarField
      form={form}
      path={path}
      schema={schema}
      metadata={metadata}
      info={info}
      label={label}
    />
  );
}
