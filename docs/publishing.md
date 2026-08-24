# Publishing

Spellwire uses two unscoped npm packages:

- `spellwire`
- `create-spellwire`

`bun create spellwire` resolves the second package automatically.

## Preflight

Confirm both names still resolve to 404 before the first release, then run:

```bash
bun install --frozen-lockfile
bun run typecheck
bun run test:ts
bun run compile:example
bun run pack:dry-run
cargo test --workspace
cargo build -p spellwire-native --release
```

The package dry run must show only the intended TypeScript source, README, license, and package metadata.

## Automated release

Add an npm automation token as the repository secret `NPM_TOKEN`. Then either run **Publish npm packages** manually or push a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow publishes in this order:

1. `spellwire`
2. `create-spellwire`

That order matters because newly generated projects depend on `spellwire: latest`.

## Manual release

```bash
npm publish --workspace spellwire --provenance --access public
npm publish --workspace create-spellwire --provenance --access public
```

## Verify

```bash
npm view spellwire version
npm view create-spellwire version
bun create spellwire smoke-test
cd smoke-test
bun run check
```

Do not describe direct OS input or prebuilt native libraries as available until those artifacts are actually included and validated.
