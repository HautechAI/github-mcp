import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { getTools } from './registry.js';
import pino from 'pino';
const logger = pino({ level: process.env.LOG_LEVEL || 'info' });
async function main() {
    const transport = new StdioServerTransport();
    const mcp = new McpServer({ name: 'github-mcp', version: '0.1.0' });
    for (const tool of getTools()) {
        const schema = tool.inputSchema;
        mcp.tool(tool.name, schema.shape, async (args) => {
            try {
                const parsed = schema.parse(args);
                const res = await tool.handler(parsed);
                return { content: [{ type: 'text', text: JSON.stringify(res) }] };
            }
            catch (err) {
                logger.error({ err }, 'tool error');
                return { content: [{ type: 'text', text: JSON.stringify({ error: { code: 'VALIDATION_ERROR', message: err.message, retriable: false } }) }] };
            }
        });
    }
    await mcp.connect(transport);
}
main().catch((err) => {
    logger.error(err);
    process.exit(1);
});
//# sourceMappingURL=index.js.map