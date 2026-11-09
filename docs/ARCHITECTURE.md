# Architecture

OpenCode is a Rust desktop app built with `eframe` and `egui`. It is an
independent editor, not a VS Code OSS fork. The source contains no Electron or
web frontend. The current code is an early prototype, not a full replacement
for VS Code or Cursor.

| Module | Responsibility |
| --- | --- |
| `src/main.rs` | Startup and native window |
| `src/app.rs` | Editor state, workspace files, shortcuts, AI jobs, reviewable edits |
| `src/view.rs`, `src/chat_view.rs`, `src/agents_view.rs` | Editor, chat, agent UI |
| `src/ai.rs` | Streaming provider requests |
| `src/chatgpt.rs` | Sign in with ChatGPT and in-memory session |
| `src/index.rs` | Local codebase index and search |
| `src/diff.rs` | Proposed file changes and diff display |
| `src/settings.rs`, `src/history.rs` | Local user settings and chat history |

The codebase index is local. A request to an AI provider can include the
current file, selected text, mentioned files, or search hits, depending on the
user's settings and action. Agent output is shown for review before files are
written. ChatGPT OAuth tokens are held in memory and are not persisted, so the
user signs in again after restarting the app. API keys entered in Settings
are currently saved in the user's local settings JSON file; protecting them
with the operating system's credential store is future work.

Contributions that improve editor basics, accessibility, error handling,
secure credential storage, and tests are especially useful.
