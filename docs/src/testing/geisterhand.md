# Geisterhand — In-Process UI Fuzzer

Geisterhand embeds a lightweight HTTP server inside your Perry app that lets you interact with every widget programmatically. Click buttons, type into text fields, drag sliders, toggle switches, capture screenshots, and run chaos-mode random input — all via simple HTTP calls.

It works on **macOS, iOS, and Android** with zero external dependencies. The server starts automatically when you compile with `--enable-geisterhand`.

## Quick Start

```bash
# Compile with geisterhand (libraries are built automatically on first use)
perry app.ts -o app --enable-geisterhand

# Run the app
./app
# [geisterhand] listening on http://127.0.0.1:7676

# In another terminal:
curl http://127.0.0.1:7676/widgets          # List all widgets
curl -X POST http://127.0.0.1:7676/click/3   # Click a button
curl http://127.0.0.1:7676/screenshot -o s.png  # Capture window
```

## API Reference

All endpoints return JSON (except `/screenshot` which returns `image/png`).

### List Widgets

```
GET /widgets
```

Returns an array of all registered widgets with their handles, types, and labels.

```json
[
  {"handle": 3, "widget_type": 0, "callback_kind": 0, "label": "Click Me"},
  {"handle": 4, "widget_type": 1, "callback_kind": 1, "label": "Type here..."},
  {"handle": 5, "widget_type": 2, "callback_kind": 1, "label": ""},
  {"handle": 6, "widget_type": 3, "callback_kind": 1, "label": "Enable"}
]
```

**Widget types**: 0 = Button, 1 = TextField, 2 = Slider, 3 = Toggle, 4 = Picker, 5 = Menu, 6 = Shortcut, 7 = Table

**Callback kinds**: 0 = onClick, 1 = onChange, 2 = onSubmit, 3 = onHover, 4 = onDoubleClick

### Click a Widget

```
POST /click/:handle
```

Fires the `onClick` callback for the widget.

```bash
curl -X POST http://127.0.0.1:7676/click/3
# {"ok":true}
```

### Type into a TextField

```
POST /type/:handle
Content-Type: application/json

{"text": "hello world"}
```

Creates a NaN-boxed string and fires the `onChange` callback.

```bash
curl -X POST http://127.0.0.1:7676/type/4 -d '{"text":"hello"}'
```

### Move a Slider

```
POST /slide/:handle
Content-Type: application/json

{"value": 0.75}
```

Fires the `onChange` callback with the given value.

```bash
curl -X POST http://127.0.0.1:7676/slide/5 -d '{"value":0.75}'
```

### Toggle a Switch

```
POST /toggle/:handle
```

Fires the `onChange` callback with a boolean value.

```bash
curl -X POST http://127.0.0.1:7676/toggle/6
```

### Set State

```
POST /state/:handle
Content-Type: application/json

{"value": 42}
```

Directly sets a State cell's value.

### Hover / Double-Click

```
POST /hover/:handle
POST /doubleclick/:handle
```

### Capture Screenshot

```
GET /screenshot
```

Returns a PNG image of the app window. Works on all platforms:
- **macOS**: `CGWindowListCreateImage` (retina resolution)
- **iOS**: `UIGraphicsImageRenderer` + `drawViewHierarchyInRect`
- **Android**: `View.draw()` on Canvas + `Bitmap.compress(PNG)`

```bash
curl http://127.0.0.1:7676/screenshot -o screenshot.png
```

### Chaos Mode

Start random input fuzzing — geisterhand picks random widgets and fires appropriate inputs:

```
POST /chaos/start
Content-Type: application/json

{"interval_ms": 200}
```

```bash
# Start chaos (fires random inputs every 200ms)
curl -X POST http://127.0.0.1:7676/chaos/start -d '{"interval_ms":200}'

# Check stats
curl http://127.0.0.1:7676/chaos/status
# {"running":true,"events_fired":42,"uptime_secs":8}

# Stop
curl -X POST http://127.0.0.1:7676/chaos/stop
```

Random inputs by widget type:
- **Button**: fires onClick (no args)
- **TextField**: random alphanumeric string (5-20 chars)
- **Slider**: random value 0.0-1.0
- **Toggle**: random true/false
- **Picker**: random index 0-9

### Health Check

```
GET /health
```

```json
{"status":"ok"}
```

## Platform Setup

### macOS

No extra setup needed. The server binds to `127.0.0.1:7676`.

```bash
perry app.ts -o app --enable-geisterhand
./app
curl http://127.0.0.1:7676/widgets
```

### iOS Simulator

The simulator shares the host's network — access the server directly on `localhost`.

```bash
perry app.ts -o app --target ios-simulator --enable-geisterhand
xcrun simctl install booted app.app
xcrun simctl launch booted com.perry.app
curl http://127.0.0.1:7676/widgets
```

### Android

Use `adb forward` to bridge the port from the emulator/device to your host. The app's `AndroidManifest.xml` must include `INTERNET` permission.

```bash
perry app.ts -o app --target android --enable-geisterhand
# Package into APK and install (see Android platform docs)
adb forward tcp:7676 tcp:7676
curl http://127.0.0.1:7676/widgets
```

## Example App

```typescript
import { App, VStack, Text, Button, TextField, Slider, Toggle } from "perry/ui";

const label = Text("Hello Geisterhand");

let count = 0;
const countLabel = Text("Count: 0");

const btn = Button("Click Me", () => {
  count++;
  countLabel.setText("Click count: " + count);
});

const field = TextField("Type here...", (text: string) => {
  console.log("TextField:", text);
});

const slider = Slider(0, 100, 50, (value: number) => {
  console.log("Slider:", value);
});

const toggle = Toggle("Enable", false, (on: boolean) => {
  console.log("Toggle:", on);
});

const stack = VStack(8, [label, countLabel, btn, field, slider, toggle]);
App({ title: "Geisterhand Test", width: 400, height: 350, body: stack });
```

## How It Works

Geisterhand is fully feature-gated — **zero code, zero overhead** in normal builds.

1. **Callback Registry** (`perry-runtime`): Every widget registers its callbacks in a global `Mutex<Vec>` at creation time. Only compiled when `--features geisterhand` is enabled.

2. **Main-Thread Dispatch**: The HTTP server runs on a background thread, but UI callbacks must execute on the main thread. A `Mutex<Vec<PendingAction>>` queue is drained by the platform's pump timer (every 8ms).

3. **HTTP Server** (`perry-ui-geisterhand`): A `tiny-http` server parses requests, looks up callbacks by widget handle, and queues actions for main-thread execution.

4. **Screenshot Capture**: Uses a `Condvar` for cross-thread synchronization — the HTTP thread queues a capture request, the main thread executes the platform-specific capture, and signals completion.

## Build Details

When you pass `--enable-geisterhand`, Perry automatically builds the required libraries on first use:

- `perry-runtime` with `--features geisterhand` (callback registry + dispatch queue)
- `perry-ui-{platform}` with `--features geisterhand` (widget registration hooks)
- `perry-ui-geisterhand` (HTTP server + chaos mode)

These are built into a separate target directory (`target/geisterhand/`) to avoid interfering with normal builds. Subsequent compilations reuse the cached libraries.

## Security

Geisterhand binds to `0.0.0.0:7676` — it's accessible from the local network. **Do not ship geisterhand-enabled binaries to production.** It is a debug/testing tool only.
