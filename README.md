# OpenCode (Rust editor)

OpenCode is an open-source, local-first AI code editor built entirely in Rust.
It uses `egui/eframe` for the desktop window: there is no browser, webview,
HTML, TypeScript, Electron, or Tauri layer.

This is an independent community project and is not affiliated with
[anomalyco/opencode](https://github.com/anomalyco/opencode), Cursor, or Microsoft.
It is an early prototype. The software is free under the MIT license; a
hosted AI provider may have its own subscription, usage limits, or API charges.
Ollama can be used with a local model.

## Features

| Shortcut | What it does |
|---|---|
| **Ctrl+K** | Select code and type an instruction (Thai or English): the AI rewrites the selection in place. With nothing selected it generates new code at the cursor. The result appears as an inline diff — **Ctrl+Enter** accepts, **Ctrl+Backspace** rejects, or type a follow-up. |
| **Ctrl+L** | Opens the chat panel (Ask mode) and adds the selected code. Mention files and folders with `@path/to/file`, `@folder/`, or search the whole project with `@codebase`. |
| **Ctrl+I** | Agent mode: one request can create, change and delete many files at once. Every file is shown as a diff to accept or reject (or accept all). |
| **Ctrl+Shift+L** | New agent chat. |
| **Ctrl+Shift+F** | Search the codebase index ("where is the payment code?"). |
| **Ctrl+P / Ctrl+S / Ctrl+W / Ctrl+B / Ctrl+,** | Go to file / save / close tab / side bar / settings. |

**Codebase indexing.** When a folder opens, every text file is split into
40-line chunks and indexed on your computer (BM25 ranking, identifiers split
on camelCase and snake_case). Questions in any language are turned into code
keywords by your AI model first, so Thai questions find English code. Files in
`.gitignore`, `.opencodeignore` or `.cursorignore` are skipped; unchanged files
are reused when you re-sync. Nothing is uploaded.

**AI accounts.** OpenCode can use an eligible ChatGPT Plus or Pro subscription
through OpenAI's official [Sign in with ChatGPT for open-source apps](https://developers.openai.com/siwc/token-sharing-open-source)
flow. In Settings, choose **ChatGPT subscription** and click **Continue with
ChatGPT**. The browser handles sign-in and consent; no API key is needed. OpenCode
keeps OAuth tokens only in memory, so sign in again after restarting it. It
stores the non-secret client and host IDs under `%APPDATA%\OpenCode` so later
sign-ins reuse the same registration. The model picker loads models from the
signed-in account. Plan usage is subject to OpenAI's eligibility and limits.

OpenAI API keys, Anthropic API keys, OpenRouter, Ollama (local, no key), and
OpenAI-compatible servers remain available. Answers stream in as they are written.
Claude subscription sign-in is not embedded: Anthropic's [credential policy](https://code.claude.com/docs/en/legal-and-compliance)
does not permit a third-party app to route requests through Claude plan credentials.
Use an Anthropic API key here, or use the unmodified Claude Code app separately.

See [architecture](docs/ARCHITECTURE.md) and [contributing](CONTRIBUTING.md)
for the current implementation and ways to help.

## Run

1. Install the Rust stable toolchain (and Visual Studio C++ Build Tools on Windows).
2. `cargo run` — or `cargo run -- path/to/project` to open a folder.
3. Press **Ctrl+,** and sign in with ChatGPT, add an API key, or pick Ollama
   to stay fully offline.

`opencode --screenshot out.png` opens the window without taking focus, draws it
and saves a picture (used for automated checks).

## Safety model

OpenCode only reads from a folder that you open. It refuses any write whose
path would leave that workspace, and no AI change reaches disk until you
accept it. The API key lives in your user settings folder
(`%APPDATA%\OpenCode\settings.json`), never in the project. Only the files you
attach (the current file, mentions, codebase search hits) are sent to the AI
provider you choose.

## Tests

`cargo test` checks the index ranking, ignore patterns, the agent's multi-file
answer format and the streaming client (against a local stand-in server).
