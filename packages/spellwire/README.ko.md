# spellwire

[English](README.md)

Bun과 TypeScript를 위한 상태 기반 실시간 입력 자동화 패키지입니다.

```bash
bun add spellwire
```

```ts
import { Key, rt, tapKey } from "spellwire";

let count = 0;

rt.onKeyDown(Key.Q, () => {
  count += 1;
  if (count % 2 === 0) tapKey(Key.E);
});
```

CLI는 세 가지 일반 작업만 제공합니다.

```bash
bunx spellwire run macro.spellwire.ts
bunx spellwire watch macro.spellwire.ts
bunx spellwire compile macro.spellwire.ts
```

`run`과 `watch`는 소스를 메모리에서 AOT 컴파일하고 플랫폼 권한을 준비한 뒤 동일한 네이티브 호스트를 시작합니다. `watch`가 추가하는 작업은 control-plane 파일 감시와 직렬화된 reload뿐이며 네이티브 실시간 dispatch 경로는 JavaScript callback 없이 유지됩니다.

자세한 내용:

- [라이브 네이티브 호스트](https://github.com/eunhhu/spellwire/blob/main/docs/live-host.ko.md)
- [플랫폼 검증](https://github.com/eunhhu/spellwire/blob/main/docs/platform-verification.ko.md)
- [API 레퍼런스](https://github.com/eunhhu/spellwire/blob/main/docs/api.ko.md)
