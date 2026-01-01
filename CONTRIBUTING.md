# Contributing to i18next-turbo

Thank you for your interest in contributing to i18next-turbo! This document provides guidelines and instructions for contributing.

## Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) before participating.

## How to Contribute

### Reporting Bugs

If you find a bug, please open an issue with:

- A clear, descriptive title
- Steps to reproduce the issue
- Expected behavior
- Actual behavior
- Your environment (OS, Rust version, etc.)
- Any relevant error messages or logs

### Suggesting Features

Feature suggestions are welcome! Please open an issue with:

- A clear description of the feature
- Use cases and examples
- Why this feature would be useful

### Pull Requests

1. **Fork the repository** and create a feature branch from `main`
2. **Make your changes** following the coding standards below
3. **Add tests** for new features or bug fixes
4. **Update documentation** if needed
5. **Run tests** to ensure everything passes
6. **Submit a pull request** with a clear description

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Building

```bash
# Clone the repository
git clone https://github.com/your-username/i18next-turbo.git
cd i18next-turbo

# Build the project
cargo build

# Run tests
cargo test

# Run with examples
cargo run -- extract
```

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Use `cargo clippy` to check for common issues
- Write meaningful commit messages

### Code Organization

- Keep functions focused and small
- Add comments for complex logic
- Use descriptive variable and function names
- Follow existing code patterns

### Testing

- Write unit tests for new features
- Test edge cases and error conditions
- Ensure tests pass before submitting PRs

## Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Example:
```
feat(extractor): add support for Trans component children

Extract translation keys from Trans component children when
i18nKey is not specified.

Closes #123
```

## Review Process

1. All PRs require at least one maintainer review
2. CI must pass (tests, linting, formatting)
3. Maintainers may request changes
4. Once approved, a maintainer will merge

## Questions?

Feel free to open an issue for questions or reach out to maintainers.

Thank you for contributing to i18next-turbo! 🎉

