# AGENTS.md

Guidelines for AI agents working in this repository.

## Project Overview

rhizome is a desktop application for creating and managing a personal
knowledge base.

- `src/` — React + TypeScript frontend (Vite).
- `src-tauri/` — Rust backend (Tauri 2, rusqlite, ort).
- `scripts/` — build/distribution helper scripts (e.g. `collect_dist.py`).
- `docs/` — project documentation.

Document text extraction delegates to the sibling `any2txt` project via
`any2txt --stdout --assets drop -- <path>`.

## Commands

Use `just` recipes; run them from the repository root:

- `just install` — install dependencies (pnpm, cargo fetch).
- `just check` — run all checkers (biome, tsc, clippy `-D warnings`). Must
  pass with zero errors.
- `just format` — format everything (biome via pnpm, cargo fmt).
- `just test` — run all tests. Coverage gates: frontend vitest and Rust
  tarpaulin must each reach >= 80% (tarpaulin excludes `src-tauri/src/main.rs`
  and `src-tauri/src/lib.rs`).
- `just dev` — run the Tauri app in development mode.
- `just build` — release build: bare desktop exe (`tauri build
  --no-bundle`), then collect it and the sibling `any2txt` release
  artifacts into `dist/` (`dist/rhizome.exe`, `dist/any2txt/` with the
  exe, `lib/site-packages`, and `models/base`). Build any2txt first
  (`just release` in the sibling repo); `ANY2TXT_DIST` overrides the
  artifact directory.

Direct shortcuts: `pnpm run <script>` (e.g. `lint`, `typecheck`,
`test:coverage`, `tauri`); Rust commands use
`--manifest-path src-tauri/Cargo.toml`.

## Conventions

- Keep changes minimal and incremental. Port only what the current
  milestone needs.
- Rust: clippy must pass with `-D warnings`; prefer `Result`-based error
  handling. Document text extraction lives in `src-tauri/src/extract.rs`
  and shells out to any2txt; binary resolution order is the `any2txt_path`
  setting, then the bundled copy under `dist/any2txt/`, then
  `uv run --project ../any2txt any2txt` in development. any2txt exit
  codes: 0 success, 2 unsupported input, 1 failure; only stdout is
  document text, stderr carries diagnostics.
- Frontend: TypeScript strict mode, biome for lint/format, vitest for
  tests with branch coverage >= 80%.
- Do not commit build output (`dist/`, `src-tauri/target/`,
  `coverage/`, `node_modules/`).
- Large binaries and model caches never go into git; `collect_dist.py`
  handles them at release time.

## Testing

- Never call real LLM or embedding APIs in tests; always mock them.
- any2txt's own suite lives in the sibling repository and is run there
  (`just test`), not here.
- When fixing a bug, add a regression test when practical.
- Run the narrowest relevant suite first (single test file), then the
  full `just check` + `just test` before considering work done.

## Verification Checklist

Before finishing any task:

1. `just check` passes.
2. `just test` passes (all coverage gates met).
3. No new lint/type warnings introduced.
4. No stray files left in the working tree.
