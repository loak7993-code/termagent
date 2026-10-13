import { createServer, type Server, type IncomingMessage, type ServerResponse } from "node:http";
import { AddressInfo } from "node:net";

export interface MockOptions {
  rejectStreamOptions?: boolean;
  withReasoning?: boolean;
}

interface SseEvent {
  choices?: Array<Record<string, unknown>>;
  usage?: Record<string, unknown>;
  model?: string;
  error?: unknown;
}

export class FakeZen {
  private server: Server;
  options: MockOptions = {};
  requestCount = 0;

  constructor() {
    this.server = createServer((req, res) => {
      void this.handle(req, res);
    });
  }

  private async handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    this.requestCount++;
    const chunks: Buffer[] = [];
    for await (const c of req) chunks.push(c as Buffer);
    const body = JSON.parse(Buffer.concat(chunks).toString() || "{}") as Record<string, unknown>;

    if (this.options.rejectStreamOptions && "stream_options" in body) {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: { message: "Unknown field: stream_options" } }));
      return;
    }

    const hasToolResult = (body.messages as Array<{ role?: string }>).some(
      (m) => m.role === "tool",
    );
    res.writeHead(200, { "content-type": "text/event-stream" });
    const events: SseEvent[] = hasToolResult
      ? [
          { choices: [{ delta: { content: "done" } }] },
          { usage: { prompt_tokens: 1200, completion_tokens: 34 }, model: "mock-free", choices: [] },
        ]
      : [
          ...(this.options.withReasoning
            ? [{ choices: [{ delta: { reasoning_content: "thinking hard" } }] }]
            : []),
          { choices: [{ delta: { content: "Running " } }] },
          {
            choices: [
              {
                delta: {
                  tool_calls: [
                    { index: 0, id: "call_1", type: "function", function: { name: "bash", arguments: "" } },
                  ],
                },
              },
            ],
          },
          {
            choices: [
              {
                delta: {
                  tool_calls: [{ index: 0, function: { arguments: '{"command": ' } }],
                },
              },
            ],
          },
          {
            choices: [
              {
                delta: {
                  tool_calls: [{ index: 0, function: { arguments: '"echo hi"}' } }],
                },
              },
            ],
          },
        ];
    for (const ev of events) {
      res.write(`data: ${JSON.stringify(ev)}\n\n`);
    }
    res.write("data: [DONE]\n\n");
    res.end();
  }

  async start(): Promise<string> {
    await new Promise<void>((resolve) => this.server.listen(0, "127.0.0.1", resolve));
    const addr = this.server.address() as AddressInfo;
    return `http://127.0.0.1:${addr.port}/v1`;
  }

  stop(): void {
    this.server.close();
  }
}
