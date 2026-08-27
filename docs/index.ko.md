# Spellwire 한국어 문서

[English](index.md)

일반 사용은 한 페이지에서 시작합니다. 나머지는 목적별 guide 또는 구현 deep dive이며 필수 순차 학습 문서가 아닙니다.

## Spellwire 사용

1. **[한 페이지 API 레퍼런스](api.ko.md)** — 복사 가능한 전체 앱, 프로젝트 명령, hotkey, remap, state, 키보드/마우스 출력, `Spellwire.start`, 모든 overlay 생성 함수·속성·option·수명 주기·현재 한계를 제공합니다.
2. **[빠른 시작](quick-start.ko.md)** — 프로젝트를 생성하고 simulator로 안전하게 확인한 뒤 첫 live run을 수행합니다.
3. **[문제 해결](troubleshooting.ko.md)** — setup, compiler, native host, overlay 오류를 메시지로 찾습니다.
4. **[플랫폼 검증](platform-verification.ko.md)** — macOS, Windows, Linux acceptance checklist를 실행하고 정확한 결과를 기록합니다.

API 레퍼런스가 일반 lookup surface입니다. 자동화와 overlay API를 한곳에 합쳐 state-to-screen workflow에서 페이지를 이동할 필요가 없습니다.

## 선택형 동작·설계 상세

| 문서 | 필요할 때 |
| --- | --- |
| [자동화 의미론](automation.ko.md) | suppression 규칙, state gate 동작, AutoHotkey migration matrix가 필요할 때 |
| [Realtime TypeScript](typescript-runtime.ko.md) | compiler syntax 제한, 반복, helper, resource budget이 필요할 때 |
| [Overlay 설계](overlay.ko.md) | reconciliation/rendering을 profiling하거나 renderer 격리를 이해할 때 |
| [Live native host](live-host.ko.md) | low-level host 수명 주기, dynamic lane, reload, library 탐색이 필요할 때 |
| [플랫폼 상태](platforms.ko.md) | backend capability와 compositor caveat가 필요할 때 |
| [아키텍처](architecture.ko.md) | compiler, wire, VM, worker, renderer 경계가 필요할 때 |
| [네이티브 C ABI](native-abi.ko.md) | Bun 밖에서 Spellwire를 embed할 때 |

## Maintainer와 release 자료

- [구현 상태](status.ko.md)
- [런타임 검증](runtime-verification.ko.md)
- [배포](publishing.ko.md)
