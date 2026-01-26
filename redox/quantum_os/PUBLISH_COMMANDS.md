# QFT SDK Publishing Commands - Complete Step-by-Step

Sequential command-line instructions for publishing each SDK to its respective package registry.

---

## 1. qft-rs (Rust → crates.io)

```bash
cd redox/quantum_os/crates/qft-rs

# Login (first time only)
cargo login
# Paste token from https://crates.io/settings/tokens

# Build and test
cargo build --release
cargo test

# Dry run
cargo publish --dry-run

# Publish
cargo publish
```

---

## 2. qft-python (Python → PyPI)

```bash
cd redox/quantum_os/crates/qft-python

# Install maturin
pip install maturin

# Build wheels
maturin build --release

# Publish (use token from https://pypi.org/manage/account/token/)
maturin publish --username __token__ --password pypi-YOUR_API_TOKEN
```

---

## 3. qft-ts (TypeScript → npm)

```bash
cd redox/quantum_os/crates/qft-ts

# Install bun (if not installed)
# Windows: powershell -c "irm bun.sh/install.ps1 | iex"

# Install dependencies
bun install

# Build
bun run build

# Login to npm
npm login

# Publish
bun publish --access public
```

---

## 4. qft-wasm (WebAssembly → npm)

```bash
cd redox/quantum_os/crates/qft-wasm

# Install wasm-pack
cargo install wasm-pack

# Build
wasm-pack build --target web --release

# Navigate to pkg directory
cd pkg

# Login to npm (if not already)
npm login

# Publish
npm publish --access public
```

---

## 5. qft-go (Go → Go Proxy)

```bash
cd redox/quantum_os/crates/qft-go

# Run tests
go test -v

# Commit changes
git add .
git commit -m "feat(qft-go): release v1.0.0"

# Tag with module path prefix
git tag qft-go/v1.0.0

# Push tag (triggers Go Proxy indexing)
git push origin main --tags

# Verify at https://pkg.go.dev/github.com/WeaveITMeta/Quantum-Operating-System/qft-go
```

---

## 6. qft-java (Java → Maven Central)

### Prerequisites

1. Create Sonatype OSSRH account: https://central.sonatype.org
2. Generate GPG key: `gpg --gen-key`
3. Configure `~/.m2/settings.xml`:

```xml
<settings>
  <servers>
    <server>
      <id>ossrh</id>
      <username>YOUR_SONATYPE_USERNAME</username>
      <password>YOUR_SONATYPE_TOKEN</password>
    </server>
  </servers>
</settings>
```

### Publish Commands

```bash
cd redox/quantum_os/crates/qft-java

# Use release POM
copy pom-release.xml pom.xml

# Build
mvn clean package

# Run tests
mvn test

# Deploy to staging (auto-releases to Maven Central)
mvn clean deploy -P release
```

---

## 7. qft-scala (Scala → Maven Central)

### Prerequisites

1. Sonatype OSSRH account (same as Java)
2. GPG key configured
3. Configure `~/.sbt/1.0/sonatype.sbt`:

```scala
credentials += Credentials(
  "Sonatype Nexus Repository Manager",
  "s01.oss.sonatype.org",
  "YOUR_SONATYPE_USERNAME",
  "YOUR_SONATYPE_TOKEN"
)
```

### Publish Commands

```bash
cd redox/quantum_os/crates/qft-scala

# Compile
sbt compile

# Run tests
sbt test

# Publish signed artifacts to staging
sbt publishSigned

# Release to Maven Central
sbt sonatypeBundleRelease
```

---

## 8. libqft (C/C++ → Conan)

```bash
cd redox/quantum_os/crates/libqft

# Install Conan
pip install conan

# Build the Rust library first
cargo build --release

# Create Conan package locally
conan create . --build=missing

# Test the package
conan test test_package libqft/1.0.0

# Upload to remote (private Artifactory or Conan Center PR)
conan upload libqft/1.0.0 -r=YOUR_REMOTE --all
```

### For Conan Center (public)

1. Fork https://github.com/conan-io/conan-center-index
2. Add recipe to `recipes/libqft/all/`
3. Submit pull request

---

## Quick Reference Table

| SDK | Directory | Publish Command |
|-----|-----------|-----------------|
| qft-rs | `crates/qft-rs` | `cargo publish` |
| qft-python | `crates/qft-python` | `maturin publish` |
| qft-ts | `crates/qft-ts` | `bun publish --access public` |
| qft-wasm | `crates/qft-wasm/pkg` | `npm publish --access public` |
| qft-go | `crates/qft-go` | `git tag qft-go/v1.0.0 && git push --tags` |
| qft-java | `crates/qft-java` | `mvn deploy -P release` |
| qft-scala | `crates/qft-scala` | `sbt publishSigned && sbt sonatypeBundleRelease` |
| libqft | `crates/libqft` | `conan create . && conan upload` |

---

## Token/Credential Sources

| Registry | Get Token At |
|----------|--------------|
| crates.io | https://crates.io/settings/tokens |
| PyPI | https://pypi.org/manage/account/token/ |
| npm | https://www.npmjs.com/settings/~/tokens |
| Sonatype | https://s01.oss.sonatype.org (after JIRA ticket) |
| GPG | `gpg --gen-key` locally |

---

## Publish All (Sequential Script)

```bash
#!/bin/bash
# publish-all.sh - Publish all QFT SDKs

set -e
ROOT=$(pwd)

echo "=== Publishing qft-rs ==="
cd $ROOT/redox/quantum_os/crates/qft-rs
cargo publish

echo "=== Publishing qft-python ==="
cd $ROOT/redox/quantum_os/crates/qft-python
maturin publish

echo "=== Publishing qft-ts ==="
cd $ROOT/redox/quantum_os/crates/qft-ts
bun install && bun run build && bun publish --access public

echo "=== Publishing qft-wasm ==="
cd $ROOT/redox/quantum_os/crates/qft-wasm
wasm-pack build --target web --release
cd pkg && npm publish --access public

echo "=== Publishing qft-go ==="
cd $ROOT
git tag qft-go/v1.0.0
git push origin main --tags

echo "=== Publishing qft-java ==="
cd $ROOT/redox/quantum_os/crates/qft-java
mvn clean deploy -P release

echo "=== Publishing qft-scala ==="
cd $ROOT/redox/quantum_os/crates/qft-scala
sbt publishSigned
sbt sonatypeBundleRelease

echo "=== Publishing libqft ==="
cd $ROOT/redox/quantum_os/crates/libqft
cargo build --release
conan create . --build=missing

echo "=== All SDKs published! ==="
```
