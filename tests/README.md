# Test layout

- `local_ops.rs` contains public Rust integration tests for ECUC discovery, inspection, safe editing, vendor-neutral module references, and configuration compatibility.
- `onboarding/Invoke-OnboardingSmoke.ps1` builds a disposable synthetic ECUC/SIP tree and verifies the Windows release exactly as a first-time user would run it.
- `open-source/` contains public-content, dependency-license, Git-history, and release-manifest guards.
- `release/` contains the small synthetic EXE self-test. The package script maps only this directory to `test/` in a release ZIP, so source contributors see one top-level `tests/` directory while end users see one top-level `test/` directory.

The public tests must never contain customer ARXML, DPA, DBC, generated code, Vector binaries, SIP content, licenses, screenshots, or internal paths. Real DaVinci generation tests belong in a private licensed environment and must operate on a disposable non-customer project.

The maintainer-only native Boolean probe is documented in
`docs/原生写入实验与安全撤销设计.md`. Onboarding covers its synthetic
`PrepareOnly` path and launcher/path isolation; it never launches Vector.
The real probe requires a licensed DaVinci 5 installation and a disposable
copy of a non-customer sample. Its saved values, validation counts and full
copied-file hashes are separate evidence, not a production undo guarantee.
Neither experiment script is included in the user package.

Typed ASW authoring and its licensed acceptance are documented in
`docs/ASW写入与验收.md`. Rust/onboarding tests use synthetic models to verify
creation/update/deletion, stale content, references, connector compatibility,
registered inputs and conservative external-dependency checks. The separate
`scripts/Invoke-LGKAswAcceptance.ps1` runs the public writer in a fresh sample
copy, then checks DaVinci 5 loaded objects and exported timing periods across
create/update/delete. Its script and Groovy task are excluded from packages.
Native loading/export does not prove RTE generation or RTE-OS mapping.

Run the complete public suite from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
& .\scripts\Build-LGKAutosar.ps1
& .\tests\open-source\Invoke-DependencyLicenseGuard.ps1
& .\tests\open-source\Invoke-HistoryGuardSmoke.ps1
& .\tests\open-source\Invoke-OpenSourceGuard.ps1 -IncludeHistory
& .\tests\open-source\Invoke-PackageManifestSmoke.ps1
& .\tests\onboarding\Invoke-OnboardingSmoke.ps1
```

The history guard retains two exact author-metadata exceptions for commits
already published on main before 2026-10-07 (`842a78b`, `fb10745`). These do
not exempt file contents, other commits, an email address or a whole branch.
New private authors and historical private product values remain rejected;
the regression smoke also confirms unrelated sibling history is excluded.
Preserving those two commits does not remove their original email metadata.
