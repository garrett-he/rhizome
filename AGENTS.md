# AGENTS.md

Guidelines for AI agents working in this repository.

## Project Overview

rhizome is a desktop application for creating and managing a personal
knowledge base. It is a pnpm workspace monorepo:

- `apps/desktop/` — Tauri desktop app (React + TypeScript frontend in
  `apps/desktop/src/`, Rust backend in `apps/desktop/src-tauri/`).
- `apps/extractor/` — legacy Python CLI (`rhizome-extract`), kept while
  the any2txt integration settles and scheduled for deletion. Text
  extraction now delegates to the sibling `any2txt` project via
  `any2txt --stdout --assets drop -- <path>`; do not add features here.
- `deprecated/` — legacy CLI implementation, kept only as a porting
  reference. Do not modify or depend on it.
- `scripts/` — build/distribution helper scripts (e.g. `collect_dist.py`).
- `docs/` — project documentation.

## Commands

Use `just` recipes; run them from the repository root:

- `just install` — install all dependencies (pnpm, cargo fetch, uv sync).
- `just check` — run all checkers (ESLint, tsc, clippy `-D warnings`,
  ruff, pyrefly). Must pass with zero errors.
- `just format` — format everything (biome/prettier via pnpm, cargo fmt,
  ruff format + check --fix).
- `just test` — run all tests. Coverage gates: frontend vitest and Rust
  tarpaulin must each reach >= 80% (tarpaulin excludes `src/main.rs` and
  `src/lib.rs`); extractor runs pytest.
- `just dev` — run the Tauri app in development mode.
- `just build` — release build: bare desktop exe (`tauri build
  --no-bundle`), then collect it and the sibling `any2txt` release
  artifacts into `dist/` (`dist/rhizome.exe`, `dist/any2txt/` with the
  exe, `lib/site-packages`, and `models/base`). Build any2txt first
  (`just release` in the sibling repo); `ANY2TXT_DIST` overrides the
  artifact directory.

Legacy extractor shortcuts (`just extract-install`, `just extract-check`,
`just extract-test`, `just extract-format`) and the `extractor`,
`extractor-deps`, `models` collectors still exist but are transitional.

Desktop-only shortcuts: run scripts inside `apps/desktop` with
`pnpm -C apps/desktop run <script>` (e.g. `lint`, `typecheck`,
`test:coverage`); Rust commands use
`--manifest-path apps/desktop/src-tauri/Cargo.toml`.

## Conventions

- Keep changes minimal and incremental; avoid porting dead code from
  `deprecated/`. Port only what the current milestone needs.
- Rust: clippy must pass with `-D warnings`; prefer `Result`-based error
  handling. Document text extraction lives in `src-tauri/src/extract.rs`
  and shells out to any2txt; binary resolution order is the `any2txt_path`
  setting, then the bundled copy under `dist/any2txt/`, then
  `uv run --project ../any2txt any2txt` in development. any2txt exit
  codes: 0 success, 2 unsupported input, 1 failure; only stdout is
  document text, stderr carries diagnostics.
- Legacy Python (extractor): formatted and linted with ruff, type-checked
  with pyrefly (both strict). Do not extend it; changes belong in the
  any2txt repository.
- Frontend: TypeScript strict mode, biome for lint/format, vitest for
  tests with branch coverage >= 80%.
- Do not commit build output (`dist/`, `apps/extractor/build/`,
  `target/`, `coverage/`, `node_modules/`).
- Large binaries and model caches never go into git; `collect_dist.py`
  handles them at release time.

## Testing

- Never call real LLM or embedding APIs in tests; always mock them.
- Extractor fixtures (documents, golden outputs) are generated
  programmatically in `apps/extractor/tests/conftest.py`; OCR/ASR are
  monkeypatched. any2txt's own suite lives in the sibling repository and
  is run there (`just test`), not here.
- When fixing a bug, add a regression test when practical.
- Run the narrowest relevant suite first (single package or test file),
  then the full `just check` + `just test` before considering work done.

## Verification Checklist

Before finishing any task:

1. `just check` passes.
2. `just test` passes (all coverage gates met).
3. No new lint/type warnings introduced.
4. No stray files left in the working tree.
