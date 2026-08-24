# Spellwire 한국어 문서

[English](index.md)

현재 알파 버전을 처음 확인한다면 다음 순서로 읽는 것을 권장합니다.

1. **[빠른 시작](quick-start.ko.md)** — 설치 또는 clone, 컴파일, inspect, simulator, 첫 live run을 진행합니다.
2. **[라이브 네이티브 호스트](live-host.ko.md)** — 세 명령 UX, 권한, 수명 주기, hot reload, 명명 상태, 동적 입력, 안전 종료를 설명합니다.
3. **[플랫폼 검증](platform-verification.ko.md)** — macOS, Windows, Linux별 복사 가능한 검증 명령과 예상 출력, 실패 해석을 제공합니다.
4. **[API 레퍼런스](api.ko.md)** — `spellwire`와 `spellwire/compiler`에서 실제로 export하는 API를 정리합니다.
5. **[실시간 TypeScript](typescript-runtime.ko.md)** — 영속 상태, 제어 흐름, helper 함수, 제한, 지원하지 않는 문법을 설명합니다.
6. **[아키텍처](architecture.ko.md)** — compiler, wire format, VM, native host, dynamic lane, overlay 경계를 설명합니다.
7. **[네이티브 C ABI](native-abi.ko.md)** — owned platform host, shared input ring, compatibility engine을 설명합니다.
8. **[플랫폼 상태](platforms.ko.md)** — 플랫폼별 API, 권한, 검증 상태, 알려진 한계를 정리합니다.
9. **[네이티브 오버레이](overlay.ko.md)** — state binding, Figma식 layout/style API, retained dirty renderer를 설명합니다.
10. **[문제 해결](troubleshooting.ko.md)** — 설치, compiler, simulator, host, overlay 문제를 진단합니다.
11. **[배포](publishing.ko.md)** — 네이티브 artifact와 npm 패키지 배포 절차를 설명합니다.
12. **[구현 상태](status.ko.md)** — 구현 완료 기능과 외부 검증 gate를 구분합니다.
13. **[런타임 검증](runtime-verification.ko.md)** — source tree와 release를 차단하는 검증 항목을 정리합니다.

## 목적별 바로가기

- 문법을 먼저 배우고 싶다면 **빠른 시작**에서 simulator까지 진행합니다.
- 실제 키보드/마우스 자동화를 실행하려면 **라이브 네이티브 호스트**로 이동합니다.
- 다른 OS의 결과를 전달하려면 **플랫폼 검증**의 보고서 템플릿을 사용합니다.
- 오류가 발생했다면 **문제 해결**에서 메시지를 찾은 뒤 연결된 상세 가이드를 따릅니다.
