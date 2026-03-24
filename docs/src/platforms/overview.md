# Platform Overview

Perry compiles TypeScript to native executables for 7 platforms from the same source code.

## Supported Platforms

| Platform | Target Flag | UI Toolkit | Status |
|----------|-------------|------------|--------|
| macOS | *(default)* | AppKit | Full support (127/127 FFI functions) |
| iOS | `--target ios` / `--target ios-simulator` | UIKit | Full support (127/127) |
| tvOS | `--target tvos` / `--target tvos-simulator` | UIKit | Full support (focus engine + game controllers) |
| watchOS | `--target watchos` / `--target watchos-simulator` | SwiftUI (data-driven) | Core support (15 widgets) |
| Android | `--target android` | JNI/Android SDK | Full support (112/112) |
| Windows | `--target windows` | Win32 | Full support (112/112) |
| Linux | `--target linux` | GTK4 | Full support (112/112) |
| Web | `--target web` | DOM/CSS | Full support (127/127) |
| WebAssembly | `--target wasm` | — | Core language (no UI) |

## Cross-Compilation

```bash
# Default: compile for current platform
perry app.ts -o app

# Compile for a specific target
perry app.ts -o app --target ios-simulator
perry app.ts -o app --target tvos-simulator
perry app.ts -o app --target watchos-simulator
perry app.ts -o app --target web
perry app.ts -o app --target windows
perry app.ts -o app --target linux
perry app.ts -o app --target android
perry app.ts -o app --target wasm
```

## Platform Detection

Use the `__platform__` compile-time constant to branch by platform:

```typescript
declare const __platform__: number;

// Platform constants:
// 0 = macOS
// 1 = iOS
// 2 = Android
// 3 = Windows
// 4 = Linux
// 5 = watchOS
// 6 = tvOS

if (__platform__ === 0) {
  console.log("Running on macOS");
} else if (__platform__ === 1) {
  console.log("Running on iOS");
} else if (__platform__ === 3) {
  console.log("Running on Windows");
}
```

`__platform__` is resolved at compile time. The compiler constant-folds comparisons and eliminates dead branches, so platform-specific code has zero runtime cost.

## Platform Feature Matrix

| Feature | macOS | iOS | tvOS | watchOS | Android | Windows | Linux | Web | WASM |
|---------|-------|-----|------|---------|---------|---------|-------|-----|------|
| CLI programs | Yes | — | — | — | — | Yes | Yes | — | Yes |
| Native UI | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | — |
| Game engines | Yes | Yes | Yes | — | Yes | Yes | Yes | — | — |
| File system | Yes | Sandboxed | Sandboxed | — | Sandboxed | Yes | Yes | — | — |
| Networking | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Fetch | — |
| System APIs | Yes | Partial | Partial | Minimal | Partial | Yes | Yes | Partial | — |
| Widgets (WidgetKit) | — | Yes | — | Yes | — | — | — | — | — |

## Next Steps

- [macOS](macos.md)
- [iOS](ios.md)
- [tvOS](tvos.md)
- [watchOS](watchos.md)
- [Android](android.md)
- [Windows](windows.md)
- [Linux (GTK4)](linux.md)
- [Web](web.md)
- [WebAssembly](wasm.md)
