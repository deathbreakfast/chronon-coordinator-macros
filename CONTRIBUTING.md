# Contributing to chronon-coordinator-macros

Thank you for improving this crate.

## Development setup

1. Clone [deathbreakfast/chronon-coordinator-macros](https://github.com/deathbreakfast/chronon-coordinator-macros)
2. Install Rust stable
3. From the repository root:

```bash
cargo fmt --all -- --check
cargo check
cargo test --all-features
```

## Code of conduct

Participation is governed by [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Security reports: [`SECURITY.md`](SECURITY.md).

## Pull requests

- Prefer small, focused PRs.
- Update rustdoc and [`README.md`](README.md) when public API or host wiring steps change.
- Do not redesign the macro surface or merge with upstream `#[chronon::script]` without an explicit plan.
