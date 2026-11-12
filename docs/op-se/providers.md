# Model providers

The Code OSS fork includes an initial OP-SE AI sidebar extension at `extensions/op-se-ai`. It sends chat requests to a user-configured OpenAI-compatible `/chat/completions` endpoint, reads SSE response chunks, supports cancellation and a configurable timeout (120 seconds by default), and stores an optional API key in Code OSS SecretStorage. Remote endpoints require HTTPS and show a context/destination confirmation before sending. Unit tests cover the SSE parser, HTTP errors, and cancellation; a direct request to a local Ollama model streamed a response. A complete response from the webview UI has not yet been verified. The earlier Rust prototype had OpenAI-compatible endpoints, Anthropic API, OpenRouter, Ollama, and the official Sign in with ChatGPT flow, but its working tree currently contains deleted files and none of those integrations are part of the Code OSS fork. Its JSON API key storage must not be carried into production.

Production adapter contract to complete in the fork:

- Stream deltas and explicit terminal success/failure.
- Cancel an in-flight request and enforce connect/read deadlines.
- Retry only bounded transient failures, never silently switch billing paths.
- Show provider, model, context files, and estimated context budget before send.
- Keep requests inside the opened workspace and redact likely secrets and
  binary data from automatic context.
- Allow local Ollama without any OP-SE account or hosting purchase.

Provider configuration is per user and must never be committed to the repo.
See `.env.example` for non-secret development placeholders. The client should
not use environment variables as a persistent credential store.
