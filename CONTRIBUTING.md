# Contributing to slv-rust

First off, thank you for considering contributing to `slv-rust`. It's people like you that make open source such a great community.

## Mission Critical Development Principles

**🔥 SEPARATION OF CONCERNS**: Every component must be in individual files with single, well-defined responsibilities. This is non-negotiable for maintainability.

**🔥 SECONDLIFE PROTOCOL COMPLIANCE**: All networking code must strictly follow SecondLife protocols. Use `homunculus/` and `hippolyzer/` as reference implementations. Protocol violations cause connection failures.

**🔥 DEVELOPMENT JOURNAL**: Document ALL roadblocks, recurring bugs, and development slowdowns in `DEVELOPMENT_JOURNAL.md`. Include context, attempted solutions, and final resolutions. This creates institutional knowledge and prevents repeated mistakes.

**🦀 RUST STRENGTHS**: Leverage Rust's type system, memory safety, and zero-cost abstractions. Prefer compile-time guarantees over runtime checks.

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

## Coding Style and Architecture Requirements

### File Organization (Separation of Concerns)
- **One responsibility per file**: Each `.rs` file must have a single, well-defined purpose
- **Comprehensive documentation**: Every file needs module-level docs explaining purpose and integration
- **Clear module boundaries**: Use `mod.rs` files only for exports, never for implementation

### SecondLife Protocol Compliance
- **Reference implementations**: Always consult `homunculus/` (TypeScript) and `hippolyzer/` (Python) before implementing networking features
- **Message format adherence**: Use `message_template.msg` as the canonical source for all protocol messages
- **Authentication**: Follow XML-RPC patterns from `homunculus/packages/homunculus-core/src/network/authenticator.ts`

### Rust Code Standards
We use the standard Rust coding style, as enforced by `rustfmt`. Before committing your code, please run `cargo fmt` to ensure your code is formatted correctly.

We also use `clippy` to catch common mistakes and improve the quality of the code. Please run `cargo clippy` and address any warnings before submitting a pull request.

- **Error handling**: Use `Result<T, E>` instead of panics
- **Type safety**: Leverage Rust's type system for compile-time correctness
- **Performance**: Use zero-cost abstractions where possible

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

## Development Journal Requirements

When encountering significant issues during development:

1. **Document immediately**: Add entries to `DEVELOPMENT_JOURNAL.md` as issues arise
2. **Include context**: What were you trying to accomplish? What environment/conditions?
3. **Track attempts**: Document each solution attempt and its outcome
4. **Record resolution**: Final working solution with explanation of why it works
5. **Add prevention notes**: How to avoid this issue in the future

### Journal Entry Template
```markdown
## [Date] - [Issue Title]

**Context**: What were you working on?
**Problem**: Specific issue encountered
**Attempts**: 
- Attempt 1: [Description] → [Outcome]
- Attempt 2: [Description] → [Outcome]
**Resolution**: Final working solution
**Prevention**: How to avoid this in the future
**References**: Links to docs, issues, or code that helped
```

## Pull Request Process

1.  Ensure that your pull request passes all CI checks.
2.  **Update DEVELOPMENT_JOURNAL.md** if you encountered and resolved significant issues
3.  A core contributor will review your pull request and may suggest changes.
4.  Once your pull request has been approved, it will be merged into the `main` branch.

## Code of Conduct

We have a [Code of Conduct](CODE_OF_CONDUCT.md) that we expect all contributors to adhere to. Please be respectful and considerate of others.

Thank you for your contributions!
