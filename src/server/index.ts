import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio';
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { getTools } from './registry.js';
import { z } from 'zod';
import pino from 'pino';

const logger = pino({ level: process.env.LOG_LEVEL || 'info' });

async function main() {
  const transport = new StdioServerTransport();
  const server = new Server({ name: 'github-mcp', version: '0.1.0' }, { transport });

  for (const tool of getTools() as any[]) {
    server.tool(tool.name, async (args: any) => {
      try {
        const parsed = (tool.inputSchema as z.ZodTypeAny).parse(args?.arguments || args);
        const res = await tool.handler(parsed);
        return { content: [{ type: 'text', text: JSON.stringify(res) }] } as any;
      } catch (err: any) {
        logger.error({ err }, 'tool error');
        return { isError: true, content: [{ type: 'text', text: JSON.stringify({ error: { code: 'VALIDATION_ERROR', message: err.message, retriable: false } }) }] } as any;
      }
    });
  }

  await server.connect();
}

main().catch((err) => {
  logger.error(err);
  process.exit(1);
});
