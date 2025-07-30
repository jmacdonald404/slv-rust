# Contributing to slv-rust

First off, thank you for considering contributing to `slv-rust`. It's people like you that make open source such a great community.

## Where to Start

If you're new to the project, a great place to start is by looking at the [open issues](https://github.com/jmacdonald404/slv-rust/issues). Issues tagged with `good first issue` are a good place to start.

## Development Workflow

1.  **Fork the repository**: Create your own fork of the `slv-rust` repository.
2.  **Clone your fork**: `git clone https://github.com/YOUR_USERNAME/slv-rust.git`
3.  **Create a branch**: `git checkout -b my-awesome-feature`
4.  **Make your changes**: Write your code and add tests.
5.  **Run the tests**: `cargo test`
6.  **Commit your changes**: `git commit -m "feat: Add my awesome feature"`
7.  **Push to your fork**: `git push origin my-awesome-feature`
8.  **Create a pull request**: Open a pull request from your fork to the `main` branch of the `slv-rust` repository.

## Coding Style

We use the standard Rust coding style, as enforced by `rustfmt`. Before committing your code, please run `cargo fmt` to ensure your code is formatted correctly.

We also use `clippy` to catch common mistakes and improve the quality of the code. Please run `cargo clippy` and address any warnings before submitting a pull request.

## Commit Messages

We use the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification for our commit messages. This allows us to automatically generate changelogs and makes the commit history easier to read.

The basic format is:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Example:**

```
feat(networking): add support for QUIC

This commit adds support for the QUIC protocol using the `quinn` library.
This will provide a more reliable and secure transport layer.

Fixes #123
```

## Pull Request Process

1.  Ensure that your pull request passes all CI checks.
2.  A core contributor will review your pull request and may suggest changes.
3.  Once your pull request has been approved, it will be merged into the `main` branch.

## Code of Conduct

We have a [Code of Conduct](CODE_OF_CONDUCT.md) that we expect all contributors to adhere to. Please be respectful and considerate of others.

Thank you for your contributions!
