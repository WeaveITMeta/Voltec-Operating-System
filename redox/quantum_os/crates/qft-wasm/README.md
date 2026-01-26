# qft-wasm - Quantum File Type WebAssembly SDK

Production-grade WebAssembly bindings for the Quantum File Type (.qft) format.

## Installation

### npm

```bash
npm install qft-wasm
```

### From Source

```bash
# Requires wasm-pack
cargo install wasm-pack
cd crates/qft-wasm
wasm-pack build --target web --release
```

## Quick Start

### Browser (ES Modules)

```javascript
import init, { QftFile, createBellState, version } from 'qft-wasm';

async function main() {
  await init();
  
  console.log(`QFT WASM v${version()}`);
  
  // Create a 4-qubit state
  const file = new QftFile(4);
  console.log(`Qubits: ${file.numQubits}`);
  console.log(`Dimension: ${file.dimension}`);
  
  // Set Bell-like state |0000⟩ + |1111⟩
  const real = new Float64Array(16);
  const imag = new Float64Array(16);
  real[0] = 1 / Math.sqrt(2);
  real[15] = 1 / Math.sqrt(2);
  file.setAmplitudes(real, imag);
  
  // Serialize to bytes
  const bytes = file.toBytes();
  console.log(`Serialized: ${bytes.length} bytes`);
  
  // Download as file
  downloadQft(bytes, 'state.qft');
}

function downloadQft(bytes, filename) {
  const blob = new Blob([bytes], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

main();
```

### Node.js

```javascript
const { QftFile, version } = require('qft-wasm');

// Create state
const file = new QftFile(4);
console.log(`Version: ${version()}`);

// Export to JSON
const json = file.toJson();
console.log(json);

// Save to file
const fs = require('fs');
fs.writeFileSync('state.qft', Buffer.from(file.toBytes()));
```

### TypeScript

```typescript
import init, { QftFile, Encoding, StreamingConverter } from 'qft-wasm';

async function processLargeFile(file: File): Promise<void> {
  await init();
  
  const converter = new StreamingConverter(
    file.size,
    1024 * 1024,  // 1MB chunks
    Encoding.Amplitude
  );
  
  const reader = file.stream().getReader();
  
  while (!converter.isComplete) {
    const { done, value } = await reader.read();
    if (done) break;
    
    const chunk = new Uint8Array(value);
    const state = converter.processChunk(chunk);
    console.log(`Progress: ${(converter.progress * 100).toFixed(1)}%`);
    
    // Process state...
  }
}
```

## API Reference

### QftFile Class

```typescript
class QftFile {
  constructor(numQubits: number);
  
  // Properties
  readonly numQubits: number;
  readonly dimension: number;
  bondDimension: number;
  golayEnabled: boolean;
  
  // Amplitude access
  getAmplitudesReal(): Float64Array;
  getAmplitudesImag(): Float64Array;
  setAmplitudes(real: Float64Array, imag: Float64Array): void;
  getAmplitude(index: number): { real: number; imag: number };
  setAmplitude(index: number, real: number, imag: number): void;
  
  // Validation
  isNormalized(tolerance?: number): boolean;
  normalize(): void;
  normSquared(): number;
  fidelity(other: QftFile): number;
  
  // Metadata
  setMetadata(key: string, value: string): void;
  getMetadata(key: string): string | undefined;
  getMetadataObject(): Record<string, string>;
  
  // Serialization
  toBytes(): Uint8Array;
  static fromBytes(data: Uint8Array): QftFile;
  toJson(): string;
  static fromJson(json: string): QftFile;
}
```

### StreamingConverter Class

```typescript
class StreamingConverter {
  constructor(
    totalSize: number,
    chunkSize?: number,
    encoding?: Encoding
  );
  
  processChunk(data: Uint8Array): QftFile;
  
  readonly progress: number;
  readonly isComplete: boolean;
  readonly chunkSize: number;
}
```

### Encoding Enum

```typescript
enum Encoding {
  Amplitude = 0,
  BasisState = 1,
  Qram = 2,
  Angle = 3,
  Block = 4,
}
```

### Utility Functions

```typescript
function version(): string;
function verify(data: Uint8Array): boolean;
function fidelityFromBytes(data1: Uint8Array, data2: Uint8Array): number;
function createBellState(): QftFile;
function createGhzState(numQubits: number): QftFile;
function createUniformState(numQubits: number): QftFile;
```

## Examples

### Create Bell State

```javascript
import { createBellState } from 'qft-wasm';

const bell = createBellState();
console.log(bell.getAmplitudesReal());
// Float64Array [0.707..., 0, 0, 0.707...]
```

### Calculate Fidelity

```javascript
import { QftFile } from 'qft-wasm';

const state1 = new QftFile(2);
const state2 = new QftFile(2);

// Both initialized to |00⟩
const fidelity = state1.fidelity(state2);
console.log(`Fidelity: ${fidelity}`);  // 1.0
```

### File Upload Handler

```javascript
async function handleFileUpload(event) {
  const file = event.target.files[0];
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  
  try {
    const qft = QftFile.fromBytes(bytes);
    console.log(`Loaded ${qft.numQubits}-qubit state`);
    console.log(`Normalized: ${qft.isNormalized()}`);
  } catch (e) {
    console.error('Invalid QFT file:', e.message);
  }
}
```

### React Component

```tsx
import React, { useState, useEffect } from 'react';
import init, { QftFile, createBellState } from 'qft-wasm';

export function QuantumStateViewer() {
  const [state, setState] = useState<QftFile | null>(null);
  const [ready, setReady] = useState(false);
  
  useEffect(() => {
    init().then(() => setReady(true));
  }, []);
  
  const createBell = () => {
    const bell = createBellState();
    setState(bell);
  };
  
  if (!ready) return <div>Loading WASM...</div>;
  
  return (
    <div>
      <button onClick={createBell}>Create Bell State</button>
      {state && (
        <div>
          <p>Qubits: {state.numQubits}</p>
          <p>Dimension: {state.dimension}</p>
          <p>Normalized: {state.isNormalized() ? 'Yes' : 'No'}</p>
        </div>
      )}
    </div>
  );
}
```

## Bundle Size

| Build | Size (gzip) |
|-------|-------------|
| Web | ~45 KB |
| Node.js | ~50 KB |

## Browser Support

- Chrome 89+
- Firefox 89+
- Safari 15+
- Edge 89+

## Performance

| Operation | 10 qubits | 20 qubits |
|-----------|-----------|-----------|
| Create | 0.1 ms | 2 ms |
| Serialize | 0.2 ms | 20 ms |
| Deserialize | 0.1 ms | 15 ms |
| Fidelity | 0.1 ms | 10 ms |

## License

MIT OR Apache-2.0
