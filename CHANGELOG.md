# Changelog

Timestamps: `YYYY-MM-DD_HHMMSS_<unix epoch>` (Europe/Budapest). Current work: [PLAN.md](PLAN.md).

## 2026-10-31_222834_1793482114: Project design and plan

- Chose the stack and architecture with the owner: Windows first with a
  platform trait for Linux, Rust + Tauri 2 single tray app built from six
  workspace crates, rusqlite storage, axum local HTTP API with a bearer token,
  Svelte 5 UI, MV3 browser extension. Decisions D1-D8 in PLAN.md.
- Wrote the full design into PLAN.md: crate contracts, SQLite schema, runtime
  wiring, rules model with an example config, API endpoints, extension design,
  milestones M0-M6, risks.
- No code yet. Toolchain checked: cargo/rustc 1.90, node 25.2, WebView2 present;
  Tauri CLI will be a project devDependency.
- Repository initialized with this plan as the first commit.
