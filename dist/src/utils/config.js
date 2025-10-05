import { z } from 'zod';
const ConfigSchema = z.object({
    GITHUB_TOKEN: z.string().min(1),
    GITHUB_BASE_URL: z.string().url().optional(),
    LOG_LEVEL: z.string().optional(),
});
let cached = null;
export function getConfig() {
    if (cached)
        return cached;
    const parsed = ConfigSchema.safeParse(process.env);
    if (!parsed.success) {
        const msg = parsed.error.flatten().fieldErrors;
        throw new Error(`Invalid configuration: ${JSON.stringify(msg)}`);
    }
    cached = parsed.data;
    return cached;
}
//# sourceMappingURL=config.js.map