# QFT SDK Publishing Guide

Complete instructions for publishing all QFT SDKs to their respective package registries.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Rust (crates.io)](#rust-cratesio)
3. [Python (PyPI)](#python-pypi)
4. [TypeScript (npm)](#typescript-npm)
5. [WebAssembly (npm)](#webassembly-npm)
6. [Go (Go Proxy)](#go-go-proxy)
7. [Java (Maven Central)](#java-maven-central)
8. [Scala (Maven Central)](#scala-maven-central)
9. [C/C++ (Conan)](#cc-conan)
10. [Automated CI/CD](#automated-cicd)

---

## Prerequisites

### Required Accounts

| Registry | Account URL | SDK |
|----------|-------------|-----|
| crates.io | https://crates.io | qft-rs |
| PyPI | https://pypi.org | qft-python |
| npm | https://npmjs.com | qft-ts, qft-wasm |
| Sonatype OSSRH | https://central.sonatype.org | qft-java, qft-scala |
| Conan Center | https://conan.io/center | libqft |

### Required Tools

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python (uv)
curl -LsSf https://astral.sh/uv/install.sh | sh

# Bun (TypeScript)
curl -fsSL https://bun.sh/install | bash

# Go
# Download from https://go.dev/dl/

# Java
# Install JDK 17+ from https://adoptium.net/

# Scala (sbt)
# Install from https://www.scala-sbt.org/download.html

# GPG (for signing)
# Windows: https://gpg4win.org/
# macOS: brew install gnupg
# Linux: apt install gnupg
```

### GPG Key Setup (Required for Maven Central)

```bash
# Generate key
gpg --gen-key

# List keys to get key ID
gpg --list-keys

# Export public key to keyserver
gpg --keyserver keyserver.ubuntu.com --send-keys YOUR_KEY_ID

# Export private key for CI/CD
gpg --armor --export-secret-keys YOUR_KEY_ID > private.key
```

---

## Rust (crates.io)

### Manual Publishing

```bash
cd crates/qft-rs

# Login (first time only)
cargo login

# Verify package
cargo package --list

# Publish
cargo publish
```

### Verification

```bash
# Test installation
cargo add qft-rs
```

---

## Python (PyPI)

### Using maturin (recommended for Rust bindings)

```bash
cd crates/qft-python

# Install maturin
pip install maturin

# Build wheels
maturin build --release

# Publish to PyPI
maturin publish --username __token__ --password pypi-YOUR_TOKEN
```

### Using uv (pure Python)

```bash
# Build
uv build

# Publish
uv publish --token pypi-YOUR_TOKEN
```

### Verification

```bash
pip install qft
python -c "import qft; print(qft.version())"
```

---

## TypeScript (npm)

### Using Bun

```bash
cd crates/qft-ts

# Install dependencies
bun install

# Build
bun run build

# Login to npm
bun login

# Publish
bun publish --access public
```

### Verification

```bash
npm install @qft/core
node -e "const qft = require('@qft/core'); console.log(qft.VERSION)"
```

---

## WebAssembly (npm)

### Using wasm-pack

```bash
cd crates/qft-wasm

# Build for web
wasm-pack build --target web --release

# Publish
cd pkg
npm publish --access public
```

### Verification

```html
<script type="module">
  import init, { QftFile } from 'qft-wasm';
  await init();
  const f = new QftFile(4);
  console.log(f.numQubits);
</script>
```

---

## Go (Go Proxy)

Go modules are automatically published via the Go Proxy when you push tags to GitHub.

### Publishing

```bash
cd crates/qft-go

# Ensure go.mod has correct module path
# module github.com/WeaveITMeta/Quantum-Operating-System/qft-go

# Commit and tag
git add .
git commit -m "feat(qft-go): v1.0.0"
git tag qft-go/v1.0.0
git push origin main --tags
```

### Verification

```bash
go get github.com/WeaveITMeta/Quantum-Operating-System/qft-go@v1.0.0
```

The module becomes available on https://pkg.go.dev automatically.

---

## Java (Maven Central)

### Prerequisites

1. Create Sonatype OSSRH account: https://central.sonatype.org
2. Create Jira ticket to claim namespace `dev.v-os.qft`
3. Set up GPG signing

### Configuration

Create `~/.m2/settings.xml`:

```xml
<settings>
  <servers>
    <server>
      <id>ossrh</id>
      <username>YOUR_SONATYPE_USERNAME</username>
      <password>YOUR_SONATYPE_TOKEN</password>
    </server>
  </servers>
  <profiles>
    <profile>
      <id>ossrh</id>
      <activation>
        <activeByDefault>true</activeByDefault>
      </activation>
      <properties>
        <gpg.executable>gpg</gpg.executable>
        <gpg.passphrase>YOUR_GPG_PASSPHRASE</gpg.passphrase>
      </properties>
    </profile>
  </profiles>
</settings>
```

### Publishing

```bash
cd crates/qft-java

# Use release POM
cp pom-release.xml pom.xml

# Deploy to staging
mvn clean deploy -P release

# Release will auto-close and release to Maven Central
```

### Verification

```xml
<dependency>
    <groupId>dev.v-os.qft</groupId>
    <artifactId>qft-java</artifactId>
    <version>1.0.0</version>
</dependency>
```

---

## Scala (Maven Central)

### Prerequisites

Same as Java (Sonatype OSSRH account, GPG key).

### Configuration

Create `~/.sbt/1.0/sonatype.sbt`:

```scala
credentials += Credentials(
  "Sonatype Nexus Repository Manager",
  "s01.oss.sonatype.org",
  "YOUR_SONATYPE_USERNAME",
  "YOUR_SONATYPE_TOKEN"
)
```

### Publishing

```bash
cd crates/qft-scala

# Publish signed artifacts
sbt publishSigned

# Release to Maven Central
sbt sonatypeBundleRelease
```

### Verification

```scala
libraryDependencies += "dev.v-os" %% "qft-scala" % "1.0.0"
```

---

## C/C++ (Conan)

### Local Testing

```bash
cd crates/libqft

# Install Conan
pip install conan

# Create package locally
conan create . --build=missing

# Test installation
conan install --requires=libqft/1.0.0
```

### Publishing to Conan Center

Conan Center requires a pull request to the conan-center-index repository:

1. Fork https://github.com/conan-io/conan-center-index
2. Create recipe in `recipes/libqft/all/`
3. Submit PR with recipe and test package

### Alternative: Private Conan Remote

```bash
# Add remote
conan remote add myremote https://your-artifactory.com/artifactory/api/conan/conan-local

# Upload
conan upload libqft/1.0.0 -r myremote --all
```

---

## Automated CI/CD

### GitHub Actions Workflow

The repository includes `.github/workflows/publish-all.yml` for automated publishing.

### Required Secrets

Configure these in GitHub repository settings → Secrets:

| Secret | Description |
|--------|-------------|
| `CRATES_IO_TOKEN` | crates.io API token |
| `PYPI_TOKEN` | PyPI API token |
| `NPM_TOKEN` | npm access token |
| `OSSRH_USERNAME` | Sonatype OSSRH username |
| `OSSRH_TOKEN` | Sonatype OSSRH token |
| `GPG_PRIVATE_KEY` | GPG private key (armored) |
| `GPG_PASSPHRASE` | GPG key passphrase |

### Triggering a Release

1. **Via GitHub Release**:
   - Create a new release on GitHub
   - The workflow triggers automatically

2. **Via Manual Dispatch**:
   - Go to Actions → Publish QFT SDKs
   - Click "Run workflow"
   - Enter version and packages to publish

### Selective Publishing

```yaml
# Publish only specific packages
packages: "rust,python,typescript"

# Publish all
packages: "all"
```

---

## Version Management

### Semantic Versioning

All SDKs follow [SemVer](https://semver.org/):

- **MAJOR**: Breaking API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

### Coordinated Releases

For coordinated releases across all SDKs:

```bash
# Update versions in all manifests
./scripts/bump-version.sh 1.1.0

# Commit and tag
git add .
git commit -m "chore: bump version to 1.1.0"
git tag v1.1.0
git push origin main --tags
```

---

## Troubleshooting

### crates.io

```
error: crate version already exists
```
→ Bump version in `Cargo.toml`

### PyPI

```
HTTPError: 403 Forbidden
```
→ Check API token permissions, ensure package name is available

### npm

```
npm ERR! 402 Payment Required
```
→ Use `--access public` for scoped packages

### Maven Central

```
Could not find artifact
```
→ Wait 10-30 minutes for sync after release

### Go Proxy

```
module not found
```
→ Ensure tag format is `module/vX.Y.Z` (e.g., `qft-go/v1.0.0`)

---

## Support

- GitHub Issues: https://github.com/WeaveITMeta/Quantum-Operating-System/issues
- Documentation: https://docs.v-os.dev/qft
