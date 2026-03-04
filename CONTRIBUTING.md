<div align="center">

  <img src=".gitlab/kairos.svg" alt="Kairos" width="400" />

  <p align="center">
    Contributing to Kairos
  </p>

</div>

---

## Getting Started

Clone the repo and make sure you can build and test locally.

```bash
cargo build --release
cargo test
cargo clippy --features heatmap -- -D warnings
cargo fmt --check
```

You'll need a [Databento](https://databento.com) API key (`DATABENTO_API_KEY`) if you're working on anything that touches historical data. Rithmic credentials are optional and managed through the app's UI.

---

## How to Contribute

There's no formal process — just open an issue or a PR.

- **Bug reports** — describe what happened, what you expected, and how to reproduce it
- **Feature ideas** — open an issue to discuss before building something large
- **Code contributions** — fork, branch, and open a merge request
- **Docs, typos, cleanups** — always welcome, no issue needed

If you're unsure whether something is worth working on, open an issue first and we'll figure it out.

---

## Code Style

- **Rust edition 2024**
- **rustfmt** — run `cargo fmt` before committing
- **Clippy** — `cargo clippy --features heatmap -- -D warnings` (this is what CI runs)
- **100-character line width** (configured in `rustfmt.toml`)
- **thiserror** for error types — implement `user_message()`, `is_retriable()`, `severity()`
- Never block the UI thread — use `Task::perform` for async work

See [CLAUDE.md](CLAUDE.md) for detailed architecture and conventions.

---

## Pull Requests

Keep it simple:

1. Describe what you changed and why
2. Make sure CI passes (`fmt`, `clippy`, `test`)
3. Keep changes focused — one concern per MR

That's it. No templates, no checklists.

---

## License

Kairos is licensed under [GPL-3.0-or-later](LICENSE). By contributing, you agree that your contributions will be licensed under the same terms.
