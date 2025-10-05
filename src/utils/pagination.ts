import { Meta } from './types.js';

export function restNextCursor(
  headers: Record<string, any>,
  page?: number,
  per_page?: number,
  itemsLength?: number,
): Pick<Meta, 'next_cursor' | 'has_more'> {
  const link: string | undefined = (headers as any)['link'] || (headers as any)['Link'];
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

export function decodeCursorPage(cursor?: string): number | undefined {
  if (!cursor) return undefined;
  const m = cursor.match(/^page:(\d+)$/);
  return m ? Number(m[1]) : undefined;
}
