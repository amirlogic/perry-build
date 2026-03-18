# CLI Commands

Perry provides 9 commands for compiling, checking, running, publishing, and managing your projects.

## compile

Compile TypeScript to a native executable.

```bash
perry compile main.ts -o app
# Or shorthand (auto-detects compile):
perry main.ts -o app
```

| Flag | Description |
|------|-------------|
| `-o, --output <PATH>` | Output file path |
| `--target <TARGET>` | Platform target (see [Compiler Flags](flags.md)) |
| `--output-type <TYPE>` | `executable` (default) or `dylib` (plugin) |
| `--print-hir` | Print HIR intermediate representation |
| `--no-link` | Produce object file only, skip linking |
| `--keep-intermediates` | Keep `.o` and `.asm` files |
| `--enable-js-runtime` | Enable V8 JavaScript runtime fallback |
| `--type-check` | Enable type checking via tsgo |
| `--minify` | Minify and obfuscate output (auto-enabled for `--target web`) |
| `--app-bundle-id <ID>` | Bundle ID (required for widget targets) |
| `--bundle-extensions <DIR>` | Bundle TypeScript extensions from directory |

```bash
# Basic compilation
perry compile app.ts -o app

# Cross-compile for iOS Simulator
perry compile app.ts -o app --target ios-simulator

# Build a plugin
perry compile plugin.ts --output-type dylib -o plugin.dylib

# Debug: view intermediate representation
perry compile app.ts --print-hir

# Build an iOS widget
perry compile widget.ts --target ios-widget --app-bundle-id com.myapp.widget
```

## run

Compile and launch your app in one step.

```bash
perry run                          # Auto-detect entry file
perry run ios                      # Run on iOS device/simulator
perry run android                  # Run on Android device
perry run -- --port 3000           # Forward args to your program
```

| Argument / Flag | Description |
|------|-------------|
| `ios` | Target iOS (device or simulator) |
| `macos` | Target macOS (default on macOS host) |
| `web` | Target web (opens in browser) |
| `android` | Target Android device |
| `--simulator <UDID>` | Specify iOS simulator by UDID |
| `--device <UDID>` | Specify iOS physical device by UDID |
| `--local` | Force local compilation (no remote fallback) |
| `--remote` | Force remote build via Perry Hub |
| `--enable-js-runtime` | Enable V8 JavaScript runtime |
| `--type-check` | Enable type checking via tsgo |
| `--` | Separator for program arguments |

**Entry file detection** (checked in order):
1. `perry.toml` → `[project] entry` field
2. `src/main.ts`
3. `main.ts`

**Device detection**: When targeting iOS, Perry auto-discovers available simulators (via `simctl`) and physical devices (via `devicectl`). For Android, it uses `adb`. When multiple targets are found, an interactive prompt lets you choose.

**Remote build fallback**: If cross-compilation toolchains aren't installed locally (e.g., iOS targets on a machine without Xcode), `perry run ios` automatically falls back to Perry Hub's build server — it packages your project, uploads it, streams build progress via WebSocket, downloads the `.ipa`, and installs it on your device. Use `--local` or `--remote` to force either path.

```bash
# Run a CLI program
perry run

# Run on a specific simulator
perry run ios --simulator 12345-ABCDE

# Force remote build
perry run ios --remote

# Run web target
perry run web
```

## check

Validate TypeScript for Perry compatibility without compiling.

```bash
perry check src/
```

| Flag | Description |
|------|-------------|
| `--check-deps` | Check `node_modules` for compatibility |
| `--deep-deps` | Scan all transitive dependencies |
| `--all` | Show all issues including hints |
| `--strict` | Treat warnings as errors |
| `--fix` | Automatically apply fixes |
| `--fix-dry-run` | Preview fixes without modifying files |
| `--fix-unsafe` | Include medium-confidence fixes |

```bash
# Check a single file
perry check src/index.ts

# Check with dependency analysis
perry check . --check-deps

# Auto-fix issues
perry check . --fix

# Preview fixes without applying
perry check . --fix-dry-run
```

## init

Create a new Perry project.

```bash
perry init my-project
cd my-project
```

| Flag | Description |
|------|-------------|
| `--name <NAME>` | Project name (defaults to directory name) |

Creates `perry.toml`, `src/main.ts`, and `.gitignore`.

## doctor

Check your Perry installation and environment.

```bash
perry doctor
```

| Flag | Description |
|------|-------------|
| `--quiet` | Only report failures |

Checks:
- Perry version
- System linker availability (cc/MSVC)
- Runtime library
- Project configuration
- Available updates

## explain

Get detailed explanations for error codes.

```bash
perry explain U001
```

Error code families:
- **P** — Parse errors
- **T** — Type errors
- **U** — Unsupported features
- **D** — Dependency issues

Each explanation includes the error description, example code, and suggested fix.

## publish

Build, sign, and distribute your app.

```bash
perry publish macos
perry publish ios
perry publish android
```

| Argument / Flag | Description |
|------|-------------|
| `macos` | Build for macOS (App Store/notarization) |
| `ios` | Build for iOS (App Store/TestFlight) |
| `android` | Build for Android (Google Play) |
| `linux` | Build for Linux (AppImage/deb/rpm) |
| `--server <URL>` | Build server (default: `https://hub.perryts.com`) |
| `--license-key <KEY>` | Perry Hub license key |
| `--project <PATH>` | Project directory |
| `-o, --output <PATH>` | Artifact output directory (default: `dist`) |
| `--no-download` | Skip artifact download |

Apple-specific flags:

| Flag | Description |
|------|-------------|
| `--apple-team-id <ID>` | Developer Team ID |
| `--apple-identity <NAME>` | Signing identity |
| `--apple-p8-key <PATH>` | App Store Connect .p8 key |
| `--apple-key-id <ID>` | App Store Connect API Key ID |
| `--apple-issuer-id <ID>` | App Store Connect Issuer ID |
| `--certificate <PATH>` | .p12 certificate bundle |
| `--provisioning-profile <PATH>` | .mobileprovision file (iOS) |

Android-specific flags:

| Flag | Description |
|------|-------------|
| `--android-keystore <PATH>` | .jks/.keystore file |
| `--android-keystore-password <PASS>` | Keystore password |
| `--android-key-alias <ALIAS>` | Key alias |
| `--android-key-password <PASS>` | Key password |
| `--google-play-key <PATH>` | Google Play service account JSON |

On first use, `publish` auto-registers a free license key.

## setup

Interactive credential wizard for app distribution.

```bash
perry setup          # Show platform menu
perry setup macos    # macOS setup
perry setup ios      # iOS setup
perry setup android  # Android setup
```

Stores credentials in `~/.perry/config.toml`.

## update

Check for and install Perry updates.

```bash
perry update             # Update to latest
perry update --check-only  # Check without installing
perry update --force       # Ignore 24h cache
```

Update sources (checked in order):
1. Custom server (env/config)
2. Perry Hub
3. GitHub API

Opt out of automatic update checks with `PERRY_NO_UPDATE_CHECK=1` or `CI=true`.

## Next Steps

- [Compiler Flags](flags.md) — Complete flag reference
- [Getting Started](../getting-started/installation.md) — Installation
