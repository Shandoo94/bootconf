# AGENTS.md

This file provides guidelines for agentic coding agents operating in this repository.

## Build, Lint, and Test Commands

```bash
# Build the project
cargo build

# Build in release mode
cargo build --release

# Run all tests
cargo test

# Run a single test by name
cargo test test_parse_valid_config
cargo test test_ssh_key_idempotency

# Type check without building
cargo check

# Format code
cargo fmt

# Run clippy (if configured)
cargo clippy

# Run clippy with auto-fix suggestions
cargo clippy --fix --allow-dirty

# View documentation
cargo doc --open
```

## Code Style Guidelines

### Imports and Namespaces
- Use full module paths to preserve namespaces, e.g., `path::PathBuf` instead of `PathBuf`
- Group std library imports first, then external crates, then local modules
- Use `use` statements for clarity; prefer explicit paths in function signatures

### Formatting
- Follow standard Rust formatting (4 spaces, not tabs)
- Use `cargo fmt` to ensure consistent formatting
- Maximum line length: 100 characters (Rust default)
- Use trailing commas in multi-line function calls and struct literals

### Types and Generics
- Prefer explicit type annotations when it improves readability
- Use generic bounds where they improve type safety
- Prefer `&str` over `&String` for string slices in function parameters
- Use `PathBuf` for owned paths, `&Path` for borrowed paths

### Naming Conventions
- **Functions and variables**: `snake_case`
- **Types and traits**: `PascalCase`
- **Constants**: `SCREAMING_SCREAM_CASE`
- **Enums and variants**: `PascalCase`
- Prefix unused parameters with underscore: `fn foo(_unused: Type)`

### Error Handling
- Use `Result<T, Box<dyn std::error::Error>>` for functions that may fail
- Use the `?` operator for error propagation
- Return meaningful error messages
- In CLI: use `eprintln!` for errors and exit with code 1

### Documentation
- No comments unless explicitly requested by the user
- Document public APIs with doc comments (`///` or `//!`) only when necessary
- Focus on "why" rather than "what" in any documentation

### Testing
- Tests are located in `src/tests/` module
- Use `tempfile::TempDir` for isolated filesystem tests
- Test idempotency: operations should be safe to run multiple times
- Use descriptive test names: `test_<feature>_<expected_behavior>`

## Project Conventions

### Dependencies
- `clap`: CLI argument parsing
- `toml`: Configuration file parsing
- `serde`: Serialization/deserialization
- `nix`: Unix system calls (hostname)
- `tempfile`: Temporary directories for tests

### Configuration Format
- TOML-based configuration files
- Host config: `hostname`, `[ssh_host_keys.ed25519]`
- Users config: `[[users]]` array with name, uid, groups, shell, etc.

### Idempotency Principle
All operations must be idempotent:
- Don't overwrite SSH keys if they already exist with correct content
- Don't change hostname if it's already set correctly
- This is tested explicitly in `test_ssh_key_idempotency`

### Conditional Compilation
- Use `#[cfg(test)]` to exclude test-only code from production
- Use `#[cfg(not(test))]` to exclude code that shouldn't run during tests (e.g., `unistd::sethostname`)

### CLI Design
- Uses clap derive macros for subcommand parsing
- Subcommands: `host`, `users`
- Each subcommand takes `--file` argument for config path

## Commit Messages

Adhere to the Conventional Commits specification: https://www.conventionalcommits.org/en/v1.0.0/

- Always specify type: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
- Specify scope where applicable: `feat: add users config`, `fix: hostname parsing`
- Use imperative mood: "add feature" not "added feature"

Examples:
```
feat: add SSH key idempotency check
fix: handle missing hostname in config
test: add test for SSH key permissions
refactor: extract hostname logic to separate function
```

## Additional Resources

- Rust API Guidelines: https://rust-lang.github.io/api-guidelines/about.html
- Rust Book: https://doc.rust-lang.org/book/
- This project's README.md contains design rationale and configuration schemas
