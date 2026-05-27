/**
 * Read a named field from a `FormData` as a string.
 *
 * `formData.get(name)` returns `FormDataEntryValue | null`, where
 * `FormDataEntryValue` is `string | File`. For our forms the named field is
 * always an `<input type="text">` / `password` / hidden with `required`, so
 * the value is always a non-empty string at runtime. TypeScript can't see
 * that from the call, so this helper narrows once and the action functions
 * stay readable.
 */
export function readString(formData: FormData, name: string): string {
  const value = formData.get(name);
  return typeof value === 'string' ? value : '';
}
