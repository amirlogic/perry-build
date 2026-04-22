---
name: release
description: Create a new Perry release — tag the current HEAD (version already in Cargo.toml / CLAUDE.md from prior commits), push the tag, create a GitHub release, and watch the gated release-packages workflow
disable-model-invocation: true
argument-hint: [optional: "minor" to request a minor bump instead of just tagging HEAD, or free-text release highlights]
allowed-tools: Bash, Read, Edit, Write, Glob, Grep
---

# New Perry Release

## Model (important — read before acting)

Perry's day-to-day workflow already bumps the patch version on **every** commit that lands on `main`. The CLAUDE.md workflow rule is explicit: each on-main commit touches `[workspace.package] version` in `Cargo.toml` and prepends a `Recent Changes` entry in `CLAUDE.md`. That means by the time `/release` runs, the current HEAD already knows what version it is.

`/release` therefore does **not**:
- bump the version (it's already bumped)
- commit anything (the work is already committed, usually across many commits)
- write a changelog entry (already in CLAUDE.md)

What `/release` **does**:
1. Reads the current version off HEAD
2. Tags HEAD with `vX.Y.Z` and pushes the tag
3. Creates a GitHub Release (notes assembled from the commits since the previous tag)
4. Watches `release-packages.yml` — which now gates on `Tests` + `Simulator Tests (iOS)` — and reports when publish succeeds or fails

There will typically be tens of commits between releases. That's normal; releases are a distribution event, not a commit event.

## Steps

### 1. Sanity checks

- `git status` — **must be clean**. If there are uncommitted changes, STOP and report. Those changes belong in regular commits (with version bumps + CLAUDE.md entries), not folded silently into a release.
- `git rev-parse --abbrev-ref HEAD` — must be `main`. If not, STOP.
- `git fetch origin && git log HEAD..origin/main --oneline` — must be empty. If origin is ahead, pull/resolve first.

### 2. Read the current version

Grep `Cargo.toml` for `^version = "` under `[workspace.package]`. This is the version to tag. Do **not** bump it here — the `$ARGUMENTS` "minor" handling is only for the edge case where the most recent on-main commit forgot to bump, which is rare.

Verify: `perry --version` (if installed locally) should match, and the `**Current Version:**` line in CLAUDE.md should match. If the three sources disagree, STOP and report — something is off in the history.

### 3. Verify the tag doesn't exist

```bash
git rev-parse "vX.Y.Z" 2>/dev/null && echo "tag exists — aborting" && exit 1
```

If the tag already exists locally or on origin, STOP. Either the release already shipped, or someone started it and didn't finish; either way, don't silently duplicate.

### 4. Survey commits since the last tag

```bash
last_tag=$(git describe --tags --abbrev=0)
git log "$last_tag"..HEAD --oneline
```

There may be 20–60 commits. Read their subjects, mentally group by `fix:` / `feat:` / `docs:` / `chore:`. The GitHub Release body will summarize them. Don't paste the raw commit list — the release notes should be a *summary*.

### 5. Tag + push

```bash
git tag "vX.Y.Z"
git push origin "vX.Y.Z"
```

This tag push fires three workflows in parallel (see `.github/workflows/`):
- `test.yml` — runs the full PR test matrix on the tag SHA
- `simctl-tests.yml` — tier-2 iOS simulator doc-example verification
- `release-packages.yml` — waits for the two above via its `await-tests` gate job, then builds binaries and publishes brew / apt / npm / GitHub release assets

### 6. Create the GitHub Release

```bash
gh release create "vX.Y.Z" \
  --title "vX.Y.Z" \
  --notes "$(cat <<'EOF'
## Highlights
- ...

## Fixes
- ...

## Features
- ...

## Infrastructure
- ...
EOF
)"
```

Group the commits since the last tag by type. Keep each bullet to one line. Link to issue numbers where the commit message mentions them. If `$ARGUMENTS` was provided, use it as the "Highlights" seed.

**Note**: `release-packages.yml` also triggers on `release: published`. Since we already pushed the tag in step 5, both the tag-push and release-published paths converge — `concurrency` at the workflow level deduplicates.

### 7. Watch the gated publish

```bash
gh run watch $(gh run list --workflow="Release Packages" --limit 1 --json databaseId --jq '.[0].databaseId')
```

Expected timeline on the tag push:
- `Tests` finishes in ~12 min
- `Simulator Tests (iOS)` finishes in ~15 min
- `release-packages` `await-tests` unblocks when both are green, then build+publish runs ~20 min

If `await-tests` shows a failed gate workflow in its logs, the release is blocked. Investigate the failed run, fix on main, cut the next patch. Do **not** re-tag vX.Y.Z — bump to vX.Y.(Z+1) and tag that. Retagging a published tag is a cardinal sin.

### 8. Verify locally (optional but recommended)

After the release is published:
```bash
cargo install --path crates/perry --force
perry --version  # should print vX.Y.Z
```

Report back:
- GitHub release URL
- Whether `release-packages.yml` was green end-to-end
- Local `perry --version` confirming the new build

## Failure modes

- **Dirty worktree**: STOP. Don't commit; hand control back to the user.
- **Tag already exists**: STOP. Investigate before deciding whether to bump further or resume a partial release.
- **Gate workflow failed (Tests or Simulator Tests)**: the tag is public but the release can't publish. Fix on main, push a new patch commit (with its own version bump + CLAUDE.md entry), then `/release` again. The stale vX.Y.Z tag is harmless — no assets, no GH release body; some git log noise.
- **`release-packages.yml` build leg failed**: similar — investigate, patch forward, never retag.

## What NOT to do

- Do not create commits during the release. If CLAUDE.md needs an entry, it needed it at commit time; it's too late now.
- Do not rewrite or amend the version-bump commit. Just tag HEAD as-is.
- Do not use `git add -A` anywhere in this skill — a dirty worktree is a STOP condition, not a thing to sweep into a release.
- Do not force-push tags.
