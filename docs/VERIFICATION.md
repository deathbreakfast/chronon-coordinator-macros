# chronon-coordinator-macros verification

Re-run after code or doc changes. Packaging move from `chronon-macros-valence`
(valence-platform member) → standalone `chronon-coordinator-macros`.

## Environment

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-chronon-coordinator-macros
```

## Unit + integration (CI)

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

### TEST_MAP

| Behavior | Level | Happy | Sad | Notes |
|----------|-------|-------|-----|-------|
| `#[script]` expand valid fn | unit | params struct + handle + ScriptDescriptor | — | `script::tests::script_impl_impl_expands_valid_function_happy` |
| `default_job` expand | unit | DefaultJobDescriptor + ensure fn | — | `script_impl_impl_expands_default_job_happy` |
| reject non-async | unit | — | compile-time error text | `validate_signature_rejects_non_async_function_sad` |
| reject wrong first param | unit | — | must be `Box<dyn ScriptContext>` | `validate_signature_rejects_first_param_not_context_sad` |
| reject wrong return | unit | — | must be `Result<()>` | `validate_signature_rejects_wrong_return_type_sad` |
| reject destructured params | unit | — | simple identifiers only | `script_impl_impl_rejects_non_identifier_params_sad` |
| reject missing name | unit | — | parse error | `script_impl_impl_rejects_missing_name_attribute_sad` |
| expanded handle / NAME | integ | `ContractTickScript::NAME` | — | `tests/script_contract.rs` |
| expanded default_job surface | integ | `ContractSweepScript` compiles | — | `tests/script_contract.rs` |

### DOCUMENTATION_MAP

| Surface | Discovery path | Notes |
|---------|----------------|-------|
| Crate root | `src/lib.rs` rustdoc + README | Getting-started `ignore` example |
| `script` attr | rustdoc on `script` | Requirements listed |
| Migration | README old→new attr table | From `chronon-macros-valence` |
| Verify | `docs/VERIFICATION.md` | This file |

### SECURITY_MAP

| Concern | Status | Notes |
|---------|--------|-------|
| Git/trust remote | deps on `unified-field-dev` chronon/coordinator | package `repository` may still say deathbreakfast |
| Secrets in repo | none expected | Examples are compile-only contracts |
| Supply-chain pins | uf-dev chronon rev `6c9ec6d` + coordinator `main` | patch deathbreakfast→uf-dev (coordinator still deathbreakfast) |
| Authz redesign | out of scope | Handlers still use host Valence factory trust |

## Campaign / bench

AWS campaign inventory and L0/L4 bench studies: **waived** for this L1 packaging rename (no runtime topology change).

## Notes

- Tests may `unwrap`/`expect`; production expanded paths map failures to Chronon errors.
- Sad-path unit tests assert error message content, (stronger than `is_err()` alone).
