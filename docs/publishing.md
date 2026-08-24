# Publishing

[한국어](publishing.ko.md)

Spellwire uses two unscoped npm packages:

- `spellwire`
- `create-spellwire`

`bun create spellwire` resolves the second package automatically.

## Preflight

Before the first release, run:

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
bun run compile:example
bun run inspect:example
bun run simulate:example
bun run pack:dry-run
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked
cargo build --workspace --release --locked
```

The package dry run must show intended source/metadata plus `native/<platform>-<arch>/` runtime, overlay, and `SHA256SUMS` artifacts when run after staging.

## Native artifacts

The publish workflow first builds `spellwire-native` and `spellwire-overlay` on x64/arm64 Linux, macOS, and Windows runners. Each runner executes `bun run stage:native`, producing:

```text
packages/spellwire/native/<process.platform>-<process.arch>/
  libspellwire_native.* or spellwire_native.dll
  spellwire-overlay[.exe]
  SHA256SUMS
```

Artifacts are uploaded separately, merged into the npm workspace in the publish job, and covered by the package `files` list.

Windows signing is enabled when `WINDOWS_SIGNING_PFX_BASE64` and `WINDOWS_SIGNING_PFX_PASSWORD` exist. macOS Developer ID signing uses `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD`, and `MACOS_SIGNING_IDENTITY`; notarization additionally uses `APPLE_ID`, `APPLE_TEAM_ID`, and `APPLE_APP_PASSWORD`. Without these secrets the workflow produces explicit unsigned artifacts rather than claiming they are signed.

## npm token for the first publish

npm requires 2FA for package creation/publishing unless the publishing credential is a **granular access token with Bypass 2FA enabled**.

Create an npm granular access token with:

- package permission: **Read and write**;
- package selection: **All Packages** for the initial publish of a new package name;
- **Bypass 2FA** enabled;
- a short expiration appropriate for release automation.

Save that token as the GitHub repository secret `NPM_TOKEN`.

A token can successfully authenticate with `npm whoami` and still fail at `npm publish` if it does not have write permission or Bypass 2FA. The publish workflow therefore has a separate credential preflight and emits the npm error at the package that actually failed.

After the packages exist, migrate to npm Trusted Publishing/OIDC and revoke the long-lived write token when practical.

## Automated release

Either run **Actions → Publish npm packages → Run workflow** on `main`, or push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

After all three native builds succeed, the workflow publishes in this order:

1. `spellwire`
2. `create-spellwire`

That order matters because generated projects depend on `spellwire`.

Publishing is idempotent per package version. If `spellwire@0.1.0` succeeds and `create-spellwire@0.1.0` fails, rerunning the workflow verifies and skips the already-published Spellwire version, then continues with `create-spellwire` instead of failing with a duplicate-version error.

## Manual release

```bash
export NODE_AUTH_TOKEN='<npm granular access token>'
npm publish ./packages/spellwire --provenance --access public
npm publish ./packages/create-spellwire --provenance --access public
```

## Verify

```bash
npm view spellwire version
npm view create-spellwire version
bun create spellwire smoke-test
cd smoke-test
bun run build
```

After download, verify checksums and run the target-machine commands in [Runtime Verification](runtime-verification.md). Source implementation of the workflow is not evidence that signing, notarization, npm credentials, or public registry propagation succeeded.
