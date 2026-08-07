# chronon-coordinator-macros

[![CI](https://github.com/deathbreakfast/chronon-coordinator-macros/actions/workflows/ci.yml/badge.svg)](https://github.com/deathbreakfast/chronon-coordinator-macros/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[GitHub](https://github.com/deathbreakfast/chronon-coordinator-macros) · `cargo doc -p chronon-coordinator-macros --open`

Proc-macro for product Chronon scripts that register with upstream Chronon inventory and wire [chronon-coordinator](https://github.com/deathbreakfast/chronon-coordinator) typed scheduling / optional `default_job(...)`.

Formerly `chronon-macros-valence` (nested under valence-platform). Attribute path:

| Old | New |
|-----|-----|
| `#[chronon_macros_valence::script(...)]` | `#[chronon_coordinator_macros::script(...)]` |

```toml
chronon-coordinator-macros = { git = "https://github.com/deathbreakfast/chronon-coordinator-macros", branch = "main" }
```

```rust
use chronon_core::ScriptContext;
use chronon_valence_identity::valence_from_context;

#[chronon_coordinator_macros::script(name = "daily_cleanup")]
pub async fn daily_cleanup(ctx: Box<dyn ScriptContext>) -> anyhow::Result<()> {
    let valence = valence_from_context(&*ctx)?;
    let _ = valence;
    Ok(())
}
```

Upstream Chronon still ships Valence-free [`chronon-macros`](https://github.com/unified-field-dev/chronon). This crate adds coordinator typed `ScriptScheduler` / `default_job` inventory on top.

Runnable demo (macro + `register_default_jobs_embedded`) lives on the
[chronon-coordinator](https://github.com/deathbreakfast/chronon-coordinator) crate:
`cargo run -p chronon-coordinator --example script_with_default_job`.

## Features

| Feature | Default | Purpose |
|---------|---------|---------|
| `default-job` | yes | Emits `DefaultJobDescriptor` inventory for embedded server startup |

## Handler contract

First parameter: `Box<dyn chronon_core::ScriptContext>`. Recover Valence:

```rust
let valence = chronon_valence_identity::valence_from_context(&*ctx)?;
```

## Verify

```bash
export CARGO_BUILD_JOBS=1
cargo test --all-features
```

See [docs/VERIFICATION.md](docs/VERIFICATION.md) for the Test Map.

## License

MIT. See [LICENSE](LICENSE), [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
