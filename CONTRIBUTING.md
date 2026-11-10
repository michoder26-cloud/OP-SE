# Contributing to OP-SE

OP-SE is an early Rust desktop editor. Bug reports, documentation fixes,
tests, and focused pull requests are welcome.

1. Open an issue describing the problem or proposed feature. For a small fix,
   you can open a pull request directly.
2. Fork the repository, make a branch, and keep each pull request focused.
3. Run `cargo fmt -- --check` and `cargo test --locked` before submitting.
4. In the pull request, explain the change, how you tested it, and any limits.

Use your own code and assets. Do not submit code, binaries, fonts, screenshots,
or other material copied from Cursor, VS Code, or another product unless its
license permits that use and you include the required notices. Do not commit
API keys, OAuth tokens, workspace files, or personal data.

AI features must keep edits reviewable before writing to disk. If a change
alters account sign-in or provider requests, document the data sent and test
failure paths as well as the successful path.

The source is licensed under [MIT](LICENSE). By contributing, you agree that
your contribution is distributed under that license.
