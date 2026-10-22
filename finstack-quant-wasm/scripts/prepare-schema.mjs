// json-schema-to-typescript consumes draft-07 tuple syntax. Keep the Rust
// schemas intact and lower their fixed-length draft-2020-12 tuples in memory.
export function prepareSchema(value) {
  if (Array.isArray(value)) return value.map(prepareSchema);
  if (!value || typeof value !== 'object') return value;
  const result = Object.fromEntries(
    Object.entries(value).map(([key, child]) => [key, prepareSchema(child)])
  );
  if (!Object.hasOwn(result, 'prefixItems')) return result;
  const { prefixItems, minItems, maxItems, items, type } = result;
  const arrayType = type === 'array' || (Array.isArray(type) && type.includes('array'));
  if (
    !arrayType ||
    !Array.isArray(prefixItems) ||
    prefixItems.length === 0 ||
    minItems !== prefixItems.length ||
    maxItems !== prefixItems.length ||
    (items !== undefined && typeof items !== 'boolean')
  ) {
    throw new Error('Unsupported prefixItems schema: expected a nonempty fixed-length array');
  }
  delete result.prefixItems;
  return { ...result, items: prefixItems, additionalItems: false };
}
