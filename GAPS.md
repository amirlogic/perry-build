# Perry Feature Requests — From Building a Real macOS App

We ported a production macOS SwiftUI app (multi-chain EVM transaction analyzer, ~40 views) to Perry to find what's missing. The data layer ported perfectly. Most UI gaps were resolved in **v0.2.155** — this document now tracks only the **remaining gaps**.

**Perry version:** 0.2.155
**App:** 32 modules, compiles to 25MB ARM64 binary
**Updated:** 2026-02-24

---

## Status

| Category | v0.2.153 | v0.2.155 | Notes |
|----------|----------|----------|-------|
| Critical gaps | 8 | **2** | 6 resolved (SecureField, ProgressView, Image, Picker, Form/Section) |
| Important gaps | 10 | **4** | 6 resolved (onHover, onDoubleClick, onChange, animations, openURL, disabled state) |
| Nice-to-have | 6 | **3** | 3 resolved (tooltips, ZStack, control sizes) |
| System API gaps | 5 | **3** | 2 resolved (Preferences, dark mode detection) |
| **Total gaps** | **29** | **12** | **~59% resolved in one release** |

**Overall Perry readiness: ~45% -> ~80%**

---

## Resolved in v0.2.155 (no longer needed)

These all work now and are used throughout the app:

- **SecureField** — real `NSSecureTextField` password input (`AuthView.ts`)
- **ProgressView** — native `NSProgressIndicator` spinner + determinate mode (`LoadingView.ts`, all views)
- **Image / SF Symbols** — `Image("magnifyingglass")`, `Image("bell.fill")`, etc. with `setSize()` and `setTint()` (30+ icons across all views)
- **ImageFile** — file-based images via `ImageFile(path)`
- **Picker** — native `NSPopUpButton` dropdown with `addItem()` / `setSelected()` (`PoolInspector.ts`, `SettingsView.ts`)
- **Form / Section** — grouped layout with `NSBox` section headers (`SettingsView.ts`, `AlertsView.ts`)
- **ZStack** — overlay layout for notification banners (`SessionWindow.ts`)
- **NavigationStack** — push/pop navigation
- **Widget disabled state** — `setEnabled(0)` on buttons during loading (`AuthView.ts`)
- **onHover** — `setOnHover(callback)` via `NSTrackingArea` (`AddressView.ts`, `AlertsView.ts`)
- **onDoubleClick** — `setOnDoubleClick(callback)` (`AddressView.ts`, `PoolInspector.ts`)
- **onChange** — `state.onChange(callback)` for value observation
- **Animations** — `animateOpacity()` and `animatePosition()` (`NotificationBanner.ts`)
- **openURL** — `openURL("https://...")` via `NSWorkspace` (`TransactionView.ts`, `AddressView.ts`, `PoolInspector.ts`)
- **Tooltips** — `setTooltip("text")` on any widget (buttons throughout app)
- **Control sizes** — `setControlSize(1)` for small buttons (`TabBar.ts`, `AlertsView.ts`)
- **Preferences** — `preferencesSet/Get` via `NSUserDefaults` (`SettingsView.ts`)
- **Dark mode** — `isDarkMode()` detection (`SettingsView.ts`)
- **delete obj[stringKey] bug** — fixed, no longer causes verifier error

---

## Remaining Gaps (12 items)

### Critical — Still blocks functionality

#### 1. String State Binding

`State()` still only supports numbers. Every text input needs module-level `let` variables with manual `rebuild()` calls.

```typescript
// What we want:
const name = State("")
name.set("Alice")
TextField("Name", (text) => name.set(text))

// What we do instead:
let name = ""
function rebuild() { widgetClearChildren(container); buildUI(container) }
```

This is the most pervasive workaround in the app — affects every view. The manual rebuild pattern works but is error-prone and verbose.

---

#### 2. Sheet (Modal Panel)

Still no way to present a modal sheet attached to the window. The app has 6 modal workflows that are non-functional:

- Add/edit alert forms
- Pool swap detail inspector
- Address search
- Session export/import

```typescript
// What we want:
Sheet({
  title: "Add Alert",
  width: 500, height: 400,
  onDismiss: () => {},
  body: Form([ Section("Alert", [ TextField("Address", onChange) ]) ])
})
```

**Native:** `NSPanel` presented via `window.beginSheet(panel)`

---

### Important — Degrades UX

#### 3. Alert (Confirmation Dialog)

No native confirmation dialogs. Delete actions execute without confirmation. Error messages shown inline instead of as alert dialogs.

```typescript
// What we want:
Alert({
  title: "Delete Alert",
  message: "Are you sure?",
  buttons: [
    { label: "Cancel", style: "cancel" },
    { label: "Delete", style: "destructive", action: () => onDelete() }
  ]
})
```

**Native:** `NSAlert`

---

#### 4. Save File Dialog

Perry has `openFileDialog` but no `saveFileDialog`. Blocks CSV export in Pool Inspector.

```typescript
// What we want:
const path = FileDialog.save({
  title: "Export CSV",
  allowedTypes: ["csv"],
  defaultName: "swaps.csv"
})
```

**Native:** `NSSavePanel` — counterpart to the existing open dialog.

---

#### 5. Multi-Window Support

Can't open content in separate windows. Session management needs independent windows.

```typescript
const win = Window({ title: "Session", width: 1200, height: 800, body: content })
win.show()
```

**Native:** Multiple `NSWindow` instances

---

#### 6. Toolbar

No native `NSToolbar` integration. Using HStack with buttons as workaround.

```typescript
Toolbar([
  ToolbarItem("Add", "system:plus", onAdd),
  ToolbarItem("Refresh", "system:arrow.clockwise", onRefresh),
])
```

**Native:** `NSToolbar` + `NSToolbarItem`

---

### Nice-to-Have

#### 7. Monospaced Font

`setFontFamily` exists but is a no-op stub. Ethereum addresses and hashes are hard to read in proportional font.

```typescript
Text("0xABC123").setFontFamily("monospaced")
```

**Native:** `NSFont.monospacedSystemFont(ofSize:weight:)`

---

#### 8. LazyVStack / Virtualized Lists

Pool swaps list can have 1000+ items. Regular ForEach creates all views upfront.

```typescript
LazyVStack(items.length, (index) => SwapRow(items[index]))
```

**Native:** `NSTableView` with virtual row loading

---

#### 9. String Preferences

`preferencesSet/Get` only supports numbers. Need string values for things like saved API URLs, user names, theme names.

```typescript
preferencesSet("apiUrl", "https://api.chainblick.com")  // currently only numbers
const url = preferencesGet("apiUrl")
```

---

### System API

#### 10. Keychain (Secure Storage)

Auth tokens stored in memory only — lost on restart. No encrypted persistent storage.

```typescript
import { Keychain } from "perry/system"
Keychain.save("auth_token", jwtToken)
const token = Keychain.get("auth_token")
```

**Native:** `Security` framework — `SecItemAdd`, `SecItemCopyMatching`, `SecItemDelete`

---

#### 11. Local Notifications

Can configure alert rules but can't deliver notifications to the user.

```typescript
import { Notifications } from "perry/system"
Notifications.send({ title: "Swap Detected", body: "0xABC... swapped 10 ETH" })
```

**Native:** `UNUserNotificationCenter`

---

#### 12. App Lifecycle Hooks

No `onTerminate` callback to save state / close database on quit. No `onActivate` to refresh data when app comes to foreground.

```typescript
App({
  title: "Chainblick",
  onLaunch: () => { initDatabase() },
  onTerminate: () => { closeDatabase() },
  body: mainView,
})
```

**Native:** `NSApplicationDelegate` methods

---

## What Perry Handles Well (v0.2.155)

Everything below works without issues:

**Layout:** VStack, HStack, ZStack, ScrollView, Spacer, Divider, Form, Section, NavigationStack
**Controls:** Text, Button, TextField, SecureField, Toggle, Slider, Picker, Image, ImageFile, ProgressView
**State:** Numeric State() with reactive binding, state.onChange()
**Widget APIs:** addChild, clearChildren, setHidden, setEnabled, setTooltip, setControlSize, setOnHover, setOnDoubleClick, animateOpacity, animatePosition, setTint, setSize
**Text:** fontSize, fontWeight, color, selectable
**System:** openURL, isDarkMode, preferencesSet/Get (numbers), clipboardRead/Write, openFileDialog, addKeyboardShortcut, context menus
**Data layer:** axios, better-sqlite3, fs, crypto, async/await, Promises — all via `--enable-js-runtime`

---

## Priority for remaining items

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| **High** | String State | Medium | Eliminates manual rebuild boilerplate across entire app |
| **High** | Sheet (modal) | Medium | Unlocks all 6 modal workflows |
| **Medium** | Alert dialog | Small | Native confirmations and error display |
| **Medium** | Save File Dialog | Small | Counterpart to existing open dialog |
| **Medium** | Monospaced font | Small | Complete the setFontFamily stub |
| **Low** | Toolbar | Medium | Native toolbar (HStack workaround is acceptable) |
| **Low** | Multi-window | Large | Multiple NSWindow management |
| **Low** | LazyVStack | Large | NSTableView virtual scrolling |
| **Low** | String preferences | Small | Extend existing preferencesSet/Get |
| **Low** | Keychain | Medium | Security framework integration |
| **Low** | Notifications | Medium | UNUserNotificationCenter |
| **Low** | Lifecycle hooks | Small | NSApplicationDelegate callbacks |
