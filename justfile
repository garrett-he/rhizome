set positional-arguments

@_default:
    just --list --unsorted

# Install dependencies
install:
    pnpm install
    cargo fetch --manifest-path apps/desktop/src-tauri/Cargo.toml

# Run checkers
check:
    pnpm -C apps/desktop run lint
    pnpm -C apps/desktop run typecheck
    cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings

# Run formatters
format:
    pnpm -C apps/desktop run format
    cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml

# Run tests
test:
    pnpm -C apps/desktop run test:coverage
    cargo tarpaulin --manifest-path apps/desktop/src-tauri/Cargo.toml --exclude-files "apps/desktop/src-tauri/src/main.rs" --exclude-files "apps/desktop/src-tauri/src/lib.rs" --skip-clean --out Stdout --fail-under 80 --timeout 300

# Run Tauri
dev:
    pnpm -C apps/desktop run tauri dev

# Build Tauri
build:
    pnpm -C apps/desktop run tauri build
