import unzipper from 'unzipper';
import fetch from 'cross-fetch';
export async function downloadZipAndAggregateText(url) {
    const res = await fetch(url);
    if (!res.ok) {
        throw new Error(`Failed to download logs: HTTP ${res.status}`);
    }
    const buffer = Buffer.from(await res.arrayBuffer());
    const directory = await unzipper.Open.buffer(buffer);
    const parts = [];
    for (const file of directory.files) {
        if (!file.path.endsWith('.txt'))
            continue;
        const content = await file.buffer();
        parts.push(content.toString('utf8'));
    }
    return parts.join('\n');
}
export function tailText(text, tail_lines) {
    if (!tail_lines || tail_lines <= 0)
        return { text, truncated: false };
    const lines = text.split(/\r?\n/);
    const truncated = lines.length > tail_lines;
    const slice = lines.slice(-tail_lines).join('\n');
    return { text: slice, truncated };
}
export function addTimestamps(text) {
    const now = new Date().toISOString();
    return text
        .split(/\r?\n/)
        .map((l) => `${now} ${l}`)
        .join('\n');
}
//# sourceMappingURL=http.js.map