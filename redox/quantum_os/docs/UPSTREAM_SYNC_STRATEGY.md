# Upstream Sync Strategy: Redox OS + V-OS

## Repository Structure

```
V-OS-Rust-X/
├── redox/                    # Fork of redox-os/redox (upstream)
│   ├── rust/                 # Submodule: redox-os/rust
│   ├── recipes/              # Upstream package recipes
│   ├── src/                  # Upstream bootloader/kernel stubs
│   └── quantum_os/           # PROPRIETARY - Our additions
│       ├── crates/           # qdb-rs, qft-rs, qos-shell, etc.
│       ├── docs/             # Our documentation
│       └── external/         # Submodule: EustressEngine
```

## What Gets Overwritten on Upstream Sync?

| Directory | Upstream Owned | Safe from Sync |
|-----------|----------------|----------------|
| `redox/recipes/` | ✅ Yes | ❌ No |
| `redox/src/` | ✅ Yes | ❌ No |
| `redox/mk/` | ✅ Yes | ❌ No |
| `redox/config/` | ✅ Yes | ❌ No |
| `redox/quantum_os/` | ❌ No | ✅ Yes |
| `redox/V-OS.md` | ❌ No | ✅ Yes |
| `redox/rust/` | ✅ Submodule | ⚠️ Depends |

**Key insight**: `quantum_os/` is entirely our addition - upstream Redox has no such directory. It will **never** be overwritten by upstream syncs.

## Branch Strategy

### Recommended: Three-Branch Model

```
main (default)
├── Tracks: origin/main (our fork)
├── Contains: quantum_os/ + upstream redox
└── Weekly: Merge from upstream/main

upstream/main
├── Tracks: redox-os/redox main
├── Read-only reference
└── Never commit directly

feature/*
├── Short-lived feature branches
├── Merge to main when complete
└── Delete after merge
```

### Setup Commands

```bash
# Add upstream remote (one-time)
git remote add upstream https://github.com/redox-os/redox.git

# Weekly sync procedure
git fetch upstream
git checkout main
git merge upstream/main --no-edit

# If conflicts in quantum_os/ (shouldn't happen):
git checkout --ours quantum_os/
git add quantum_os/
git commit
```

## Conflict Prevention Rules

### 1. Never Modify Upstream Files

```
❌ DO NOT EDIT:
- redox/recipes/core/*
- redox/src/bootloader/*
- redox/mk/*.mk
- redox/Makefile (use quantum_os/Makefile instead)

✅ SAFE TO EDIT:
- redox/quantum_os/**
- redox/V-OS.md
- redox/Start.md (our addition)
- redox/Ports.md (our addition)
```

### 2. Recipe Isolation

For custom packages, use a separate recipe directory:

```toml
# redox/quantum_os/recipes/quantum-shell/recipe.toml
[source]
git = "../../crates/qos-shell"

[build]
template = "cargo"
```

Then symlink or include in build:

```bash
# In quantum_os/Makefile
include ../mk/config.mk
RECIPES += quantum_os/recipes/*
```

### 3. Config Overlay Pattern

```
redox/config/x86_64/desktop.toml     # Upstream (don't edit)
redox/quantum_os/config/quantum.toml # Our overlay (safe)
```

Build with: `make CONFIG=quantum_os/config/quantum.toml`

## Proprietary vs Open Source Boundaries

### Open Source (Can Upstream)

| Component | Upstream Candidate | Notes |
|-----------|-------------------|-------|
| `qft-rs` | ✅ Yes | Already on crates.io |
| `qdb-rs` | ✅ Yes | Already on crates.io |
| `logosq` | ✅ Yes | MIT licensed |

### Proprietary (Keep Private)

| Component | Reason |
|-----------|--------|
| `qos-shell` | V-OS branding/UX |
| `eustress-quantum` | Integration layer |
| Direct Eustress integration | Competitive advantage |
| Kardashev simulation configs | Research IP |

## Weekly Sync Checklist

- [ ] `git fetch upstream`
- [ ] `git merge upstream/main`
- [ ] Resolve any conflicts (should be rare)
- [ ] `cargo check -p qdb-rs -p qft-rs -p qos-shell`
- [ ] Run tests: `cargo test --workspace`
- [ ] Update `CHANGELOG.md` if upstream has breaking changes
- [ ] Push to origin: `git push origin main`

## Handling Breaking Upstream Changes

### Scenario: Upstream changes kernel API

```bash
# 1. Create compatibility branch
git checkout -b compat/kernel-api-v2

# 2. Update quantum_os crates
# Edit: quantum_os/crates/kernel_services/src/lib.rs

# 3. Test
cargo test -p kernel_services

# 4. Merge back
git checkout main
git merge compat/kernel-api-v2
```

### Scenario: Upstream removes feature we depend on

```bash
# 1. Pin to last working upstream commit
git log upstream/main --oneline | head -20
# Find last good commit

# 2. Create pinned branch
git checkout -b pinned/pre-removal <commit>

# 3. Cherry-pick our changes
git cherry-pick main~10..main

# 4. Document in UPSTREAM_PINS.md
```

## Automation (Optional)

### GitHub Actions: Weekly Sync

```yaml
# .github/workflows/upstream-sync.yml
name: Upstream Sync
on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly Sunday midnight
  workflow_dispatch:

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      
      - name: Fetch upstream
        run: |
          git remote add upstream https://github.com/redox-os/redox.git
          git fetch upstream
      
      - name: Merge upstream
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git merge upstream/main --no-edit || echo "Conflicts detected"
      
      - name: Check for conflicts
        run: |
          if git diff --name-only --diff-filter=U | grep -q .; then
            echo "::error::Merge conflicts detected"
            exit 1
          fi
      
      - name: Push
        run: git push origin main
```

## Summary

| Question | Answer |
|----------|--------|
| Will upstream sync overwrite `quantum_os/`? | **No** - it doesn't exist upstream |
| Will upstream sync overwrite `qdb-rs`? | **No** - it's inside `quantum_os/` |
| What might conflict? | Only if we edit upstream files |
| Best practice? | Keep all changes in `quantum_os/` |
| How to track upstream? | Add `upstream` remote, merge weekly |
