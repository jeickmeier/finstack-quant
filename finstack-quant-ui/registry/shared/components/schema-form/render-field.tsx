"use client";
import { useContext, useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { useStore } from "@tanstack/react-form";
import { Fieldset } from "@/lib/finstack/form";
import { EnumField } from "../../primitives/enum-field/enum-field";
import { FieldRendererContext } from "./field-renderer";
import { discriminator } from "./discriminator";
import {
  resolve,
  propertiesOf,
  nullable,
  initialValue,
  objectValue,
  labelFor,
  type InstrumentModule,
  type SchemaLocation,
} from "./schema";
import type { SchemaFormApi, FieldFilter } from "./schema-form";
import { nullableFieldChrome, optionalFieldChrome } from "./field-chrome";

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
  const [more, setMore] = useState(false);
  const visibleTerms = useRef(new Map<string, Set<string>>());
  const fieldMeta = useStore(form.store, (state) => state.fieldMeta);
  const render = useContext(FieldRendererContext);
  const within = (child: string, parent: string) =>
    child === parent ||
    child.startsWith(`${parent}.`) ||
    child.startsWith(`${parent}[`);
  if (fields?.deny?.some((key) => within(path, key))) return null;
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
    const secondaryTerm = ([key, node]: (typeof properties)[number]) => {
      const fieldPath = path ? `${path}.${key}` : key;
      if (
        Object.entries(fieldMeta).some(
          ([name, meta]) =>
            within(name, fieldPath) && Boolean(meta?.errors.length),
        )
      )
        return false;
      const current = objectValue(value)[key];
      // Keep supplied non-default values and invalid fields visible. Empty
      // optional objects and nullable branches remain under More terms.
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
    // Once a term is visible through a supplied value, edit or error, keep it
    // available for this editor session. Matching a default must not hide the
    // control beneath the user's cursor. Each schema branch owns its own set.
    let visible = visibleTerms.current.get(pointer);
    if (!visible) {
      visible = new Set<string>();
      visibleTerms.current.set(pointer, visible);
    }
    for (const entry of properties)
      if (!secondaryTerm(entry)) visible.add(entry[0]);
    const secondary = properties.filter(([key]) => !visible.has(key));
    const render = ([key, node]: (typeof properties)[number]) => (
      <RenderField
        key={key}
        sectionless={sectionless && properties.length === 1}
        {...{ module, form, layout, fields }}
        location={{ schema: node, pointer: `${pointer}/properties/${key}` }}
        path={path ? `${path}.${key}` : key}
        label={
          path === "instrument" && key === "spec"
            ? "Instrument terms"
            : labelFor(key)
        }
        value={objectValue(value)[key]}
        required={schema.required?.includes(key) ?? false}
      />
    );
    if (!path || (path === "instrument" && schema.properties?.spec))
      return <div className="min-w-0">{properties.map(render)}</div>;
    const contents = (
      <>
        <div className="finstack-term-fields">
          {properties
            .filter(([key]) => layout === "full" || more || visible.has(key))
            .map(render)}
        </div>
        {layout === "basic" && secondary.length > 0 && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            aria-expanded={more}
            onClick={() => setMore(!more)}
          >
            {more ? "Fewer" : "More"} {label.toLowerCase()} fields (
            {secondary.length})
          </Button>
        )}
      </>
    );
    if (sectionless) return <div className="min-w-0">{contents}</div>;
    return (
      <Fieldset
        {...info}
        className={
          path === "instrument.spec" ? "finstack-schema-root" : undefined
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
    <form.AppField name={path}>
      {(field) => {
        if (schema.type === "boolean") return <field.BooleanField {...info} />;
        if (schema.format === "date") return <field.DateField {...info} />;
        if (
          schema.type === "string" &&
          (metadata?.source.includes("/decimal") || schema.title === "Decimal")
        )
          return <field.DecimalField {...info} pattern={schema.pattern} />;
        if (["string", "integer", "number"].includes(String(schema.type)))
          return (
            <field.TextField
              {...info}
              inputMode={
                schema.type === "integer"
                  ? "numeric"
                  : schema.type === "number"
                    ? "decimal"
                    : "text"
              }
            />
          );
        return (
          <p className="text-sm text-muted-foreground">
            {label}: unsupported generated schema construct.
          </p>
        );
      }}
    </form.AppField>
  );
}
