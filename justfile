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

# Build release artifacts (desktop exe + extractor onefile + models) into dist/
build:
    just build-desktop
    just build-extractor
    python scripts/collect_dist.py desktop extractor models

# Build desktop bare executable (Tauri, no installer bundle)
build-desktop:
    pnpm -C apps/desktop run tauri build --no-bundle

# Build extractor one-file executable (Nuitka)
build-extractor:
    uv run --project apps/extractor nuitka --onefile --assume-yes-for-downloads \
        --output-dir=apps/extractor/build --output-filename=rhizome-extract \
        --remove-output --include-package=rhizome_extract --include-package=rapidocr \
        --include-package-data=rapidocr --include-package-data=pymupdf \
        --noinclude-custom-mode=pymupdf:bytecode \
        --noinclude-custom-mode=omegaconf:bytecode --no-prefer-source-code \
        apps/extractor/build_entry.py

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
