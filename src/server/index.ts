import { McpServer } from '@modelcontextprotocol/sdk/server/mcp';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { getTools } from './registry.js';
import { z } from 'zod';
import pino from 'pino';

const logger = pino({ level: process.env.LOG_LEVEL || 'info' });

async function main() {
  const transport = new StdioServerTransport();
  const mcp = new McpServer({ name: 'github-mcp', version: '0.1.0' });

  for (const tool of getTools() as any[]) {
    const schema = tool.inputSchema as z.ZodObject<any>;
    mcp.tool(tool.name, schema.shape, async (args: any) => {
      try {
        const parsed = schema.parse(args);
        const res = await tool.handler(parsed);
        return { content: [{ type: 'text', text: JSON.stringify(res) }] } as any;
      } catch (err: any) {
        logger.error({ err }, 'tool error');
        return { content: [{ type: 'text', text: JSON.stringify({ error: { code: 'VALIDATION_ERROR', message: err.message, retriable: false } }) }] } as any;
      }
    });
  }

  await mcp.connect(transport);
}

main().catch((err) => {
  logger.error(err);
  process.exit(1);
});
