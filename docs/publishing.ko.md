# 배포

[English](publishing.md)

Spellwire는 두 unscoped npm package를 사용합니다.

- `spellwire`
- `create-spellwire`

`bun create spellwire`는 두 번째 package를 자동으로 resolve합니다.

## 사전 검증

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

native staging 후 package dry-run에는 source/metadata와 `native/<platform>-<arch>/` runtime, overlay, `SHA256SUMS`가 모두 보여야 합니다. 한국어 package README도 tarball에 포함되는지 확인하십시오.

## Native artifact

publish workflow는 x64/arm64 Linux, macOS, Windows runner에서 `spellwire-native`와 `spellwire-overlay`를 먼저 빌드합니다. 각 runner의 `bun run stage:native` 결과:

```text
packages/spellwire/native/<process.platform>-<process.arch>/
  libspellwire_native.* or spellwire_native.dll
  spellwire-overlay[.exe]
  SHA256SUMS
```

artifact를 별도 upload한 뒤 publish job에서 npm workspace로 merge합니다.

Windows signing은 `WINDOWS_SIGNING_PFX_BASE64`, `WINDOWS_SIGNING_PFX_PASSWORD`가 있을 때 활성화됩니다. macOS Developer ID signing은 `MACOS_CERTIFICATE_P12_BASE64`, `MACOS_CERTIFICATE_PASSWORD`, `MACOS_SIGNING_IDENTITY`를 사용하며 notarization에는 `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`가 추가로 필요합니다. secret이 없으면 signed라고 주장하지 않고 명시적 unsigned artifact를 생성합니다.

## 첫 npm publish token

새 package 생성/publish에는 2FA를 통과하거나 **Bypass 2FA가 켜진 granular access token**이 필요합니다.

권장 설정:

- package permission: **Read and write**
- 초기 새 이름 publish: **All Packages**
- **Bypass 2FA** 활성화
- release automation에 맞는 짧은 expiration

GitHub repository secret `NPM_TOKEN`으로 저장합니다. `npm whoami`가 성공해도 write permission 또는 Bypass 2FA가 없으면 publish는 실패할 수 있습니다. workflow는 credential preflight와 package별 실제 error를 출력합니다.

package가 생성된 뒤 가능하면 npm Trusted Publishing/OIDC로 전환하고 장기 write token을 revoke하십시오.

## 자동 release

Actions의 **Publish npm packages**를 `main`에서 실행하거나 version tag를 push합니다.

```bash
git tag v0.1.0
git push origin v0.1.0
```

native build가 모두 성공하면 다음 순서로 publish합니다.

1. `spellwire`
2. `create-spellwire`

생성 프로젝트가 `spellwire`에 의존하므로 순서가 중요합니다. workflow는 package version별 idempotent입니다. 첫 package만 성공한 뒤 재실행해도 이미 이 저장소에서 publish한 같은 version은 skip하고 다음 package를 계속합니다.

## 수동 release

```bash
export NODE_AUTH_TOKEN='<npm granular access token>'
npm publish ./packages/spellwire --provenance --access public
npm publish ./packages/create-spellwire --provenance --access public
```

## 공개 결과 검증

```bash
npm view spellwire version
npm view create-spellwire version
bun create spellwire smoke-test
cd smoke-test
bun run build
# 대상 장비에서 live input도 확인할 때만:
bun run start
```

download 후 checksum을 확인하고 [런타임 검증](runtime-verification.ko.md)의 대상 장비 명령을 실행하십시오. workflow source가 존재한다는 사실만으로 signing, notarization, npm credential, registry propagation 성공을 주장하면 안 됩니다.
