export function restNextCursor(headers, page, per_page, itemsLength) {
    const link = headers['link'] || headers['Link'];
    if (link) {
        const hasNext = /rel="next"/.test(link);
        if (hasNext) {
            const match = link.match(/\bpage=(\d+)/);
            const nextPage = match ? Number(match[1]) : (page || 1) + 1;
            return { next_cursor: `page:${nextPage}`, has_more: true };
        }
        return { next_cursor: null, has_more: false };
    }
    if (per_page && itemsLength != null && itemsLength >= per_page) {
        const nextPage = (page || 1) + 1;
        return { next_cursor: `page:${nextPage}`, has_more: true };
    }
    return { next_cursor: null, has_more: false };
}
export function decodeCursorPage(cursor) {
    if (!cursor)
        return undefined;
    const m = cursor.match(/^page:(\d+)$/);
    return m ? Number(m[1]) : undefined;
}
//# sourceMappingURL=pagination.js.map