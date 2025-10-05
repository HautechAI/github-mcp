export function mapError(err) {
    const status = err?.status || err?.response?.status || err?.code;
    const code = normalizeCode(status, err);
    const message = err?.message || 'Unknown error';
    const retriable = isRetriable(status);
    return { error: { code, message, retriable } };
}
function isRetriable(status) {
    const n = Number(status);
    if (Number.isNaN(n))
        return false;
    if (n === 429)
        return true;
    return n >= 500 && n < 600;
}
function normalizeCode(status, err) {
    if (typeof status === 'number')
        return `HTTP_${status}`;
    if (typeof status === 'string')
        return status.toUpperCase();
    if (err?.name)
        return String(err.name).toUpperCase();
    return 'UNKNOWN';
}
//# sourceMappingURL=errors.js.map