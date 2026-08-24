# create-spellwire

[English](README.md)

Spellwire TypeScript 매크로 프로젝트를 생성합니다.

```bash
bun create spellwire my-automation
cd my-automation
bun run start
```

생성된 프로젝트는 세 가지 작업만 노출합니다.

```bash
bun run start  # 한 번 실행
bun run watch  # 네이티브 hot reload와 함께 실행
bun run build  # dist에 네이티브 프로그램과 상태 manifest 생성
```

`src/main.spellwire.ts`에는 realtime VM 로직, `src/app.ts`에는 생성된 상태 기반 modern overlay와 통합 `Spellwire.start()` lifecycle이 있습니다. 수동 render/update loop는 필요 없습니다.

의존성 설치를 건너뛰려면 다음 옵션을 사용합니다.

```bash
bun create spellwire my-automation --no-install
```
