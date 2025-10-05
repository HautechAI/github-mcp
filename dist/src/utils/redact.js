export function redactToken(value) {
    if (!value)
        return value;
    return value.replace(/[A-Za-z0-9_\-]{10,}/g, '***');
}
//# sourceMappingURL=redact.js.map