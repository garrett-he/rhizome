set positional-arguments

@_default:
    just --list --unsorted

# Install dependencies
install:
    pnpm install
    cargo fetch --manifest-path apps/desktop/src-tauri/Cargo.toml
    uv sync --project apps/extractor

# Run checkers
check:
    pnpm -C apps/desktop run lint
    pnpm -C apps/desktop run typecheck
    cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
    just extract-check

# Run formatters
format:
    pnpm -C apps/desktop run format
    cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
    just extract-format

# Run tests
test:
    pnpm -C apps/desktop run test:coverage
    cargo tarpaulin --manifest-path apps/desktop/src-tauri/Cargo.toml --exclude-files "src/main.rs" --exclude-files "src/lib.rs" --skip-clean --out Stdout --fail-under 80 --timeout 300
    just extract-test

# Run Tauri
dev:
    pnpm -C apps/desktop run tauri dev

# Build release artifacts (desktop exe + any2txt release) into dist/
build:
    just build-desktop
    python scripts/collect_dist.py desktop any2txt

# Build desktop bare executable (Tauri, no installer bundle)
build-desktop:
    pnpm -C apps/desktop run tauri build --no-bundle

# Build extractor executable (app + Python runtime) and external deps directory
build-extractor:
    uv run --project apps/extractor python scripts/build_extractor.py

# Collect model caches into dist/models
build-models:
    python scripts/collect_dist.py models

# Install extractor dependencies
extract-install:
    uv sync --project apps/extractor

# Run extractor tests
extract-test:
    uv run --project apps/extractor pytest apps/extractor/tests -q

# Format extractor code
extract-format:
    uv run --project apps/extractor ruff format apps/extractor/src apps/extractor/tests
    uv run --project apps/extractor ruff check --fix apps/extractor/src apps/extractor/tests

# Check extractor code (ruff + pyrefly)
extract-check:
    uv run --project apps/extractor ruff check apps/extractor/src apps/extractor/tests
    uv run --project apps/extractor pyrefly check apps/extractor/src
