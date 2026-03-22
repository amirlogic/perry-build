# i18n CLI Tools

## `perry i18n extract`

Scans your TypeScript source files for localizable strings and generates or updates locale JSON files.

```bash
perry i18n extract src/main.ts
```

### What It Does

1. Recursively scans all `.ts` and `.tsx` files in the source directory
2. Detects string literals in UI component calls: `Button("...")`, `Text("...")`, `Label("...")`, etc.
3. Also detects `t("...")` calls from `perry/i18n`
4. Creates `locales/` directory if it doesn't exist
5. For each configured locale, creates or updates a JSON file:
   - **Default locale**: New keys are pre-filled with themselves as values
   - **Non-default locales**: New keys are added with empty string values (indicating "needs translation")

### Example Output

```
Scanning for localizable strings...
  Found 12 localizable string(s)
  Updated locales/en.json (3 new, 1 unused)
  Updated locales/de.json (3 new, 1 unused)
  Updated locales/fr.json (3 new, 1 unused)
Done.
```

### Workflow

Typical translation workflow:

```bash
# 1. Write code with English strings
#    Button("Next"), Text("Hello, {name}!", { name })

# 2. Extract strings to locale files
perry i18n extract src/main.ts

# 3. Send locales/de.json to translators (empty values need filling)

# 4. Build — Perry validates everything
perry compile src/main.ts -o myapp
```

### Detected Patterns

The scanner detects these UI component patterns:

- `Button("string")`
- `Text("string")`
- `Label("string")`
- `TextField("string")`
- `TextArea("string")`
- `Tab("string")`
- `NavigationTitle("string")`
- `SectionHeader("string")`
- `SecureField("string")`
- `Alert("string")`
- `t("string")` (explicit i18n API)

Both double-quoted and single-quoted strings are supported. Escaped quotes are handled correctly.

## Build Output

During compilation, Perry reports i18n status:

```
  i18n: 2 locale(s) [en, de], default: en
    Loaded locales/en.json (12 keys)
    Loaded locales/de.json (12 keys)
  i18n: 12 localizable string(s) detected
  i18n warning: Missing translation for key "Settings" in locale "de"
  i18n warning: Unused i18n key "Old Label" in locale "en"
```

## Key Registry

Perry maintains a `.perry/i18n-keys.json` file, updated on every build:

```json
{
  "keys": [
    { "key": "Next", "string_idx": 0 },
    { "key": "Hello, {name}!", "string_idx": 1 },
    { "key": "You have {count} items", "string_idx": 2 }
  ]
}
```

This file serves as the source of truth for what strings exist in the codebase.
