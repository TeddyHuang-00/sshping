import "recipes/release.just"

# Format code
format:
    cargo +nightly fmt --all
    cargo sort --workspace
    cargo sort-derives

# Check unused dependencies
deps:
    cargo +nightly udeps 

# Check for errors
check: format
    cargo clippy --fix --allow-staged
    @just format

# Unit tests
test: check
    cargo test 

# Coverage report
coverage: check
    cargo tarpaulin --out Html --output-dir coverage

