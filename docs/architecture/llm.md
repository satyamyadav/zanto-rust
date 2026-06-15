# Architecture — LLM Integration

How zanto talks to models. Source: [chat.rs](../../crates/zanto-core/src/chat.rs).
Library: `genai 0.6`.

## genai as the abstraction

zanto does not implement provider HTTP itself. It uses `genai::Client`, which:
- routes by **model name prefix** to an adapter (Ollama, Gemini, OpenAI, Anthropic…),
- resolves each adapter's default **endpoint** and **API-key env var**,
- translates `ChatRequest` (messages + tools) to/from the provider wire format,
- surfaces structured `tool_calls` on the response.

zanto only configures: which model, and (for Ollama) which endpoint.

## Model-name routing

genai picks the adapter from the model name (`AdapterKind::from_model`):

| Prefix | Adapter | Default endpoint | API key env |
|---|---|---|---|
| `gemini*` | Gemini | `https://generativelanguage.googleapis.com/v1beta/` | `GEMINI_API_KEY` |
| `gpt*`, `o1/o3/o4*`, `chatgpt*` | OpenAI | OpenAI default | `OPENAI_API_KEY` |
| `claude*` | Anthropic | Anthropic default | `ANTHROPIC_API_KEY` |
| (unrecognized) | Ollama | `http://localhost:11434/` | — |

Only Ollama and Gemini are exercised today. Any unrecognized name (e.g.
`qwen2.5:14b`) falls through to the Ollama adapter.

## The endpoint override (and why it exists)

The user's Ollama runs on a **remote** host (a Mac), not `localhost`. genai's
Ollama default is `http://localhost:11434/`. So `chat()` installs a
`ServiceTargetResolver` that rewrites the endpoint to the configured one:

```rust
let override_endpoint = !config.model.starts_with("gemini");
ServiceTargetResolver::from_resolver_fn(move |target| {
    if !override_endpoint {
        return Ok(target);          // Gemini: use genai's own endpoint + key
    }
    // Ollama (and anything non-gemini): force the configured host
    Ok(ServiceTarget { endpoint: Endpoint::from_static(endpoint_str), auth, model })
})
```

- **Ollama**: endpoint forced to `config.endpoint`
  (`http://192.168.1.66:11434/` by default) — without this, requests would go to
  localhost and fail.
- **Gemini**: override skipped → genai uses its Gemini endpoint and reads
  `GEMINI_API_KEY` from the environment. No endpoint config needed.

This single `starts_with("gemini")` check is the entire cloud-vs-local switch. To
add another cloud provider, extend the condition so its prefix also skips the
override.

`endpoint_str` is `Box::leak`'d to `&'static str` in `main.rs` because the
resolver closure requires a `'static` endpoint.

## Selecting the model

Resolution order (in `main.rs`): `--model` flag › `settings.model` ›
`"qwen2.5:14b"`. So:

```bash
# local Ollama (default)
cargo run -p zanto-cli -- "…"

# Gemini, per-run
GEMINI_API_KEY=… cargo run -p zanto-cli -- -m gemini-flash-latest "…"

# Gemini, persisted in .zanto/settings.json  →  "model": "gemini-flash-latest"
GEMINI_API_KEY=… cargo run -p zanto-cli -- "…"
```

## Tool calls — structured vs fallback

The loop reads `res.tool_calls()`. Two paths:

1. **Structured** (Gemini, well-behaved Ollama models): genai returns parsed
   `ToolCall { call_id, fn_name, fn_arguments }`. Used directly.
2. **Fallback** (`extract_raw_tool_calls`): some models (observed: qwen2.5 via
   Ollama) emit a tool call as raw JSON text — sometimes with stray prefix text —
   that genai does **not** parse as a tool call. When `tool_calls()` is empty but
   the assistant text contains `{"name": …, "arguments": …}`, zanto brace-scans
   the text, builds synthetic `ToolCall`s (id `fallback-<8hex>`), and executes
   them. Without this the raw JSON would leak to the user and the tool would never
   run. Logged as `warn: model returned unparsed tool call(s)…`.

Gemini returns proper structured calls, so the fallback never fires for it — but
the executor/permission path after parsing is identical regardless of source.

## System prompt

A fixed `ChatMessage::system(...)` prepended every turn, never persisted
([data-model.md](data-model.md)). Current text:

> "You are a helpful assistant. Use the provided tools to answer questions about
> the filesystem."

## What's not here (current state)

- No streaming — `exec_chat` is awaited to completion per turn.
- No token accounting, no caching, no retries/backoff.
- No per-provider tuning (temperature, thinking budget) — genai defaults.
- API keys come only from environment variables (genai's default resolvers);
  zanto has no key-storage layer.
