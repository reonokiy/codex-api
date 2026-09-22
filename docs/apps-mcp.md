# Apps mcp

`POST / GET / DELETE /backend-api/ps/mcp` · Alias: `/api/codex/ps/mcp`

**Request:** MCP Streamable HTTP JSON-RPC initialize,notifications/initialized,tools/list,tools/call,resources/list/read,elicitation replies; MCP session lifecycle.

**Headers:** `x-openai-product-sku`, `originator`, `mcp-session-id`, `mcp-protocol-version`, `last-event-id`, `accept`.

**Response:** application/json or text/event-stream; MCP session headers must be preserved.

Preserve `Mcp-Session-Id`, `MCP-Protocol-Version`, `Last-Event-ID` and negotiated content types. `POST` carries JSON-RPC requests/notifications, `GET` opens the event stream, and `DELETE` ends the session. This forwards the native Apps MCP service; public Responses `mcp` tool orchestration has a different contract.

[Pinned source](https://github.com/openai/codex/blob/be2951ea34f0d295ed0becf97079f92fa5f6950e/codex-rs/codex-mcp/src/mcp/mod.rs#L592) · [Native rules](native.md)
