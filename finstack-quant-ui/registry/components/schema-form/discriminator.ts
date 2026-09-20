import {
  resolve,
  propertiesOf,
  matchesBranch,
  initialValue,
  objectValue,
  type Schema,
  type SchemaLocation,
} from "./schema";
/** Structural tag information from the generated canonical schema and its resolved metadata paths. */
export function discriminator(root: Schema, location: SchemaLocation) {
  const { schema, pointer } = resolve(root, location);
  const variants = schema.oneOf ?? schema.anyOf;
  if (!variants) return null;
  const branches = variants.map((variant, index) => {
    const location = resolve(root, {
      schema: variant,
      pointer: `${pointer}/${schema.oneOf ? "oneOf" : "anyOf"}/${index}`,
    });
    const properties = propertiesOf(location.schema);
    if (Object.hasOwn(location.schema, "const"))
      return {
        ...location,
        label: String(location.schema.const),
        tag: "",
        external: false,
        unit: true,
      };
    const tag = properties.find(
      ([, node]) => Object.hasOwn(node, "const") || node.enum?.length === 1,
    );
    if (tag)
      return {
        ...location,
        label: String(tag[1].const ?? tag[1].enum![0]),
        tag: tag[0],
        external: false,
        unit: false,
      };
    if (
      properties.length === 1 &&
      location.schema.required?.includes(properties[0][0])
    )
      return {
        ...location,
        label: properties[0][0],
        tag: properties[0][0],
        external: true,
        unit: false,
      };
    return null;
  });
  if (branches.some((branch) => !branch)) return null;
  const supported = branches.filter((branch) => branch !== null);
  if (supported.every((branch) => branch.unit)) return null;
  return {
    branches: supported,
    selected: (value: unknown) =>
      supported.findIndex((branch) =>
        matchesBranch(root, branch.schema, value),
      ),
    switchTo(index: number, value: unknown) {
      const branch = supported[index];
      if (branch.unit) return branch.schema.const;
      const next = objectValue(initialValue(root, branch));
      if (!branch.external)
        for (const key of Object.keys(branch.schema.properties ?? {}))
          if (key !== branch.tag && Object.hasOwn(objectValue(value), key))
            next[key] = objectValue(value)[key];
      return next;
    },
  };
}
