# Validation status

## Included automated checks

Run on Windows from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\check.ps1
```

The script runs:

1. `cargo fmt --all -- --check`
2. `cargo test --all-targets`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo build --release`

The GitHub Actions workflow performs the same checks on `windows-latest` and uploads the resulting executable.

## Checks performed while preparing this archive

- Rust source delimiter and string/comment balance
- Pure coordinate and zoom calculations against their unit-test expectations
- Application manifest XML parsing
- Resource, script, documentation, and source-tree completeness

A native Windows/MSVC compiler was not available in the archive-generation environment, so the final Windows link and live desktop behavior must be confirmed by the included Windows CI and manual checklist before publishing a binary release.
