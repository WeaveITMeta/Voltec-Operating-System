# qft-java - Java SDK for Quantum File Type

Production-grade Java library for working with quantum states in the .qft format.

## Installation

### Maven

```xml
<dependency>
    <groupId>dev.macrohard.qft</groupId>
    <artifactId>qft-java</artifactId>
    <version>1.0.0</version>
</dependency>
```

### Gradle

```groovy
implementation 'dev.macrohard.qft:qft-java:1.0.0'
```

## Quick Start

```java
import dev.macrohard.qft.QftFile;

public class Example {
    public static void main(String[] args) throws Exception {
        // Create a 4-qubit state
        QftFile state = new QftFile(4);
        System.out.println("Qubits: " + state.getNumQubits());
        System.out.println("Dimension: " + state.getDimension());
        
        // Set Bell-like state |0000⟩ + |1111⟩
        double sqrt2Inv = 1.0 / Math.sqrt(2.0);
        state.setAmplitude(0, sqrt2Inv, 0);
        state.setAmplitude(15, sqrt2Inv, 0);
        
        // Save to disk
        state.save("state.qft");
        
        // Load and verify
        QftFile loaded = QftFile.load("state.qft");
        System.out.println("Fidelity: " + state.fidelity(loaded));
    }
}
```

## Features

### Common States

```java
QftFile bell = QftFile.bellState();           // |00⟩ + |11⟩
QftFile ghz = QftFile.ghzState(4);            // |0000⟩ + |1111⟩
QftFile uniform = QftFile.uniformState(3);    // Equal superposition
QftFile basis = QftFile.basisState(4, 5);     // |0101⟩
```

### State Operations

```java
QftFile state = new QftFile(4);

// Normalization
state.normalize();
System.out.println(state.isNormalized(1e-10));  // true

// Fidelity
QftFile other = new QftFile(4);
double fid = state.fidelity(other);

// Inner product
double[] overlap = state.innerProduct(other);  // [real, imag]

// Trace distance
double dist = state.traceDistance(other);
```

### Amplitude Access

```java
QftFile state = new QftFile(4);

// Get/set single amplitude
double[] amp = state.getAmplitude(0);  // [real, imag]
state.setAmplitude(0, 0.5, 0.5);

// Get all amplitudes
double[] real = state.getAmplitudesReal();
double[] imag = state.getAmplitudesImag();
```

### Configuration

```java
QftConfig config = new QftConfig(128, true, 1e-12);
QftFile state = new QftFile(4, config);

// Or modify later
state.getConfig().setBondDimension(256);
```

### Metadata

```java
QftFile state = new QftFile(4);

state.setMetadata("author", "Alice");
state.setMetadata("date", "2026-01-25");

String author = state.getMetadata("author");
Map<String, String> all = state.getAllMetadata();
```

### Serialization

```java
// Binary format
state.save("state.qft");
QftFile loaded = QftFile.load("state.qft");

// Bytes
byte[] bytes = state.toBytes();
QftFile fromBytes = QftFile.fromBytes(bytes);

// JSON
String json = state.toJson();
QftFile fromJson = QftFile.fromJson(json);
```

## API Reference

### QftFile

| Method | Description |
|--------|-------------|
| `new QftFile(n)` | Create n-qubit state |
| `new QftFile(n, config)` | Create with config |
| `fromAmplitudes(real, imag)` | Create from arrays |
| `load(path)` | Load from file |
| `fromBytes(data)` | Deserialize |
| `fromJson(json)` | Parse JSON |
| `getNumQubits()` | Get qubit count |
| `getDimension()` | Get dimension (2^n) |
| `getAmplitudesReal()` | Get real parts |
| `getAmplitudesImag()` | Get imaginary parts |
| `getAmplitude(i)` | Get single amplitude |
| `setAmplitude(i, re, im)` | Set single amplitude |
| `setAmplitudes(real, imag)` | Set all amplitudes |
| `norm()` | Calculate norm |
| `normSquared()` | Calculate norm² |
| `isNormalized(tol)` | Check normalization |
| `normalize()` | Normalize in place |
| `innerProduct(other)` | Calculate ⟨this\|other⟩ |
| `fidelity(other)` | Calculate \|⟨this\|other⟩\|² |
| `traceDistance(other)` | Calculate trace distance |
| `clone()` | Deep copy |
| `save(path)` | Save to file |
| `toBytes()` | Serialize to bytes |
| `toJson()` | Export to JSON |
| `setMetadata(k, v)` | Set metadata |
| `getMetadata(k)` | Get metadata |
| `getAllMetadata()` | Get all metadata |
| `getConfig()` | Get configuration |
| `setConfig(cfg)` | Update configuration |

### Static Factory Methods

| Method | Description |
|--------|-------------|
| `bellState()` | (|00⟩ + |11⟩) / √2 |
| `ghzState(n)` | (|0...0⟩ + |1...1⟩) / √2 |
| `uniformState(n)` | Equal superposition |
| `basisState(n, i)` | Computational basis |

### QftConfig

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `bondDimension` | int | 64 | MPS bond dimension |
| `golayEnabled` | boolean | true | Enable error correction |
| `truncationThreshold` | double | 1e-10 | SVD threshold |

## Requirements

- Java 17+
- Gson 2.10+ (for JSON support)

## Thread Safety

`QftFile` is **not** thread-safe. Use external synchronization if accessing from multiple threads.

## License

MIT OR Apache-2.0
