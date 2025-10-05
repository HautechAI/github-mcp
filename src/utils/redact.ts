export function redactToken(value?: string): string | undefined {
  if (!value) return value;
  return value.replace(/[A-Za-z0-9_\-]{10,}/g, '***');
}
