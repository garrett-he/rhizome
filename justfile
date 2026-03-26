set positional-arguments

@_default:
    just --list --unsorted

# Install dependencies
install:
    pnpm install
    cargo fetch --manifest-path src-tauri/Cargo.toml

# Run checkers
check:
    pnpm run lint
    pnpm run typecheck
    cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

# Run formatters
format:
    pnpm run format
    cargo fmt --manifest-path src-tauri/Cargo.toml

# Run tests
test:
    pnpm run test:coverage
    cargo tarpaulin --manifest-path src-tauri/Cargo.toml --exclude-files "src/main.rs" --exclude-files "src/lib.rs" --skip-clean --out Stdout --fail-under 80 --timeout 300

# Run Tauri
dev:
    pnpm run tauri dev

# Build release artifacts (desktop exe + any2txt release) into dist/
build:
    just build-desktop
    python scripts/collect_dist.py desktop any2txt

# Build desktop bare executable (Tauri, no installer bundle)
build-desktop:
    pnpm run tauri build --no-bundle
