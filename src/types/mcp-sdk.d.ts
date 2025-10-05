declare module '@modelcontextprotocol/sdk/server/mcp' {
  export class McpServer {
    constructor(info: { name: string; version: string }, options?: any);
    tool(name: string, schema: any, cb: (args: any) => Promise<any> | any): void;
    connect(transport: any): Promise<void>;
  }
}

declare module '@modelcontextprotocol/sdk/server/stdio' {
  export class StdioServerTransport {
    constructor();
  }
}
