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
    cargo tarpaulin --manifest-path src-tauri/Cargo.toml --exclude-files src/main.rs --skip-clean --out Stdout --fail-under 80 --timeout 300

# Run Tauri
dev:
    pnpm tauri dev

# Build Tauri
build:
    pnpm tauri build
