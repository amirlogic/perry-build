---
name: Bug report
about: Something's not working as expected
title: ""
labels: bug
assignees: ""
---

## What happened

<!-- One or two sentences on what you saw. -->

## What you expected

<!-- What should have happened instead? -->

## Minimal reproduction

<!--
The smallest possible input that triggers the bug. Copy-pasteable TypeScript
is ideal. If it requires a package layout, link to a tiny repo or gist.

If you can't minimize it, that's okay — paste what you have and say so.
-->

```typescript
// your code here
```

Command you ran:

```bash
# e.g. perry compile main.ts -o app --target ios-simulator
```

## Environment

- **Perry version**: <!-- `perry --version` -->
- **Host OS**: <!-- e.g. macOS 14.5 (arm64), Ubuntu 24.04, Windows 11 -->
- **Target**: <!-- e.g. native, ios-simulator, android, web, wasm -->
- **Rust toolchain** (if building from source): <!-- `rustc --version` -->
- **Installed via**: <!-- npm / brew / winget / apt / from source -->

## Diagnostic output

<!--
Anything from `perry doctor`, compiler errors, runtime stack traces, or
console output. Triple-backtick fence long blobs.
-->

## Anything else

<!-- Workarounds you've tried, related issues, screenshots, etc. -->
