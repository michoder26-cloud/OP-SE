# Model providers

The Code OSS fork does not yet include OP-SE's AI adapter. The separate Rust
prototype supports OpenAI-compatible endpoints, Anthropic API, OpenRouter,
Ollama, and the official Sign in with ChatGPT flow. That prototype stores API
keys in a local JSON settings file; this must move to OS secure storage before
it becomes the production AI integration.

Production adapter contract to implement in the fork:

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
