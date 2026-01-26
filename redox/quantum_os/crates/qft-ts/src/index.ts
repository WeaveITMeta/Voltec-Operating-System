/**
 * @qft/core - Production-Grade TypeScript SDK for Quantum File Type (.qft)
 *
 * Table of Contents:
 * 1. Types and Interfaces
 * 2. Complex Number Class
 * 3. QftFile Class
 * 4. QftBuilder Class
 * 5. Common States
 * 6. I/O Operations
 * 7. Validation Functions
 */

// =============================================================================
// Section 1: Types and Interfaces
// =============================================================================

/** Configuration for QFT file operations */
export interface QftConfig {
  /** Bond dimension for MPS compression (1-1024) */
  bondDimension: number;
  /** Enable Golay error correction */
  golayEnabled: boolean;
  /** Truncation threshold for SVD */
  truncationThreshold: number;
}

/** Default configuration */
export const DEFAULT_CONFIG: QftConfig = {
  bondDimension: 64,
  golayEnabled: true,
  truncationThreshold: 1e-10,
};

/** Error codes for QFT operations */
export enum QftErrorCode {
  InvalidQubits = 'INVALID_QUBITS',
  IndexOutOfRange = 'INDEX_OUT_OF_RANGE',
  DimensionMismatch = 'DIMENSION_MISMATCH',
  ZeroNorm = 'ZERO_NORM',
  InvalidFormat = 'INVALID_FORMAT',
  IoError = 'IO_ERROR',
  SerializationError = 'SERIALIZATION_ERROR',
}

/** Custom error class for QFT operations */
export class QftError extends Error {
  constructor(
    public readonly code: QftErrorCode,
    message: string
  ) {
    super(message);
    this.name = 'QftError';
  }
}

// =============================================================================
// Section 2: Complex Number Class
// =============================================================================

/** Complex number representation */
export class Complex {
  constructor(
    public readonly re: number,
    public readonly im: number = 0
  ) {}

  /** Create from polar coordinates */
  static fromPolar(r: number, theta: number): Complex {
    return new Complex(r * Math.cos(theta), r * Math.sin(theta));
  }

  /** Complex conjugate */
  conj(): Complex {
    return new Complex(this.re, -this.im);
  }

  /** Magnitude squared */
  normSqr(): number {
    return this.re * this.re + this.im * this.im;
  }

  /** Magnitude */
  norm(): number {
    return Math.sqrt(this.normSqr());
  }

  /** Addition */
  add(other: Complex): Complex {
    return new Complex(this.re + other.re, this.im + other.im);
  }

  /** Subtraction */
  sub(other: Complex): Complex {
    return new Complex(this.re - other.re, this.im - other.im);
  }

  /** Multiplication */
  mul(other: Complex | number): Complex {
    if (typeof other === 'number') {
      return new Complex(this.re * other, this.im * other);
    }
    return new Complex(
      this.re * other.re - this.im * other.im,
      this.re * other.im + this.im * other.re
    );
  }

  /** Division */
  div(other: Complex | number): Complex {
    if (typeof other === 'number') {
      return new Complex(this.re / other, this.im / other);
    }
    const denom = other.normSqr();
    return new Complex(
      (this.re * other.re + this.im * other.im) / denom,
      (this.im * other.re - this.re * other.im) / denom
    );
  }

  /** String representation */
  toString(): string {
    if (this.im === 0) return `${this.re}`;
    if (this.re === 0) return `${this.im}i`;
    const sign = this.im >= 0 ? '+' : '-';
    return `${this.re}${sign}${Math.abs(this.im)}i`;
  }

  /** Check equality */
  equals(other: Complex, tolerance = 1e-10): boolean {
    return (
      Math.abs(this.re - other.re) < tolerance &&
      Math.abs(this.im - other.im) < tolerance
    );
  }
}

/** Zero complex number */
export const ZERO = new Complex(0, 0);

/** One complex number */
export const ONE = new Complex(1, 0);

/** Imaginary unit */
export const I = new Complex(0, 1);

// =============================================================================
// Section 3: QftFile Class
// =============================================================================

/** A quantum state file in .qft format */
export class QftFile {
  private _numQubits: number;
  private _amplitudes: Complex[];
  private _metadata: Map<string, string>;
  private _config: QftConfig;

  /**
   * Create a new QFT file with the specified number of qubits.
   * Initializes to the |0...0⟩ state.
   *
   * @param numQubits - Number of qubits (1-30)
   * @param config - Optional configuration
   */
  constructor(numQubits: number, config: Partial<QftConfig> = {}) {
    if (numQubits < 1 || numQubits > 30) {
      throw new QftError(
        QftErrorCode.InvalidQubits,
        `num_qubits must be 1-30, got ${numQubits}`
      );
    }

    this._numQubits = numQubits;
    this._config = { ...DEFAULT_CONFIG, ...config };
    this._metadata = new Map();

    const dim = 1 << numQubits;
    this._amplitudes = new Array(dim).fill(null).map(() => ZERO);
    this._amplitudes[0] = ONE; // |0...0⟩
  }

  /**
   * Create from an array of complex amplitudes.
   */
  static fromAmplitudes(amplitudes: Complex[], config: Partial<QftConfig> = {}): QftFile {
    const dim = amplitudes.length;
    if (dim === 0 || (dim & (dim - 1)) !== 0) {
      throw new QftError(
        QftErrorCode.InvalidFormat,
        'Amplitude count must be a power of 2'
      );
    }

    const numQubits = Math.log2(dim);
    if (numQubits > 30) {
      throw new QftError(QftErrorCode.InvalidQubits, `Too many qubits: ${numQubits}`);
    }

    const file = new QftFile(numQubits, config);
    file._amplitudes = [...amplitudes];
    return file;
  }

  /**
   * Create from separate real and imaginary arrays.
   */
  static fromRealImag(
    real: number[],
    imag: number[],
    config: Partial<QftConfig> = {}
  ): QftFile {
    if (real.length !== imag.length) {
      throw new QftError(
        QftErrorCode.DimensionMismatch,
        `Real and imag arrays must have same length`
      );
    }

    const amplitudes = real.map((r, i) => new Complex(r, imag[i]));
    return QftFile.fromAmplitudes(amplitudes, config);
  }

  // =========================================================================
  // Properties
  // =========================================================================

  /** Number of qubits */
  get numQubits(): number {
    return this._numQubits;
  }

  /** State vector dimension (2^numQubits) */
  get dimension(): number {
    return 1 << this._numQubits;
  }

  /** Configuration */
  get config(): QftConfig {
    return { ...this._config };
  }

  /** Update configuration */
  setConfig(config: Partial<QftConfig>): void {
    this._config = { ...this._config, ...config };
  }

  // =========================================================================
  // Amplitude Access
  // =========================================================================

  /** Get all amplitudes */
  get amplitudes(): readonly Complex[] {
    return this._amplitudes;
  }

  /** Get real parts of amplitudes */
  get amplitudesReal(): number[] {
    return this._amplitudes.map((a) => a.re);
  }

  /** Get imaginary parts of amplitudes */
  get amplitudesImag(): number[] {
    return this._amplitudes.map((a) => a.im);
  }

  /** Get a single amplitude by basis state index */
  getAmplitude(index: number): Complex {
    if (index < 0 || index >= this.dimension) {
      throw new QftError(
        QftErrorCode.IndexOutOfRange,
        `Index ${index} out of range for ${this._numQubits} qubits`
      );
    }
    return this._amplitudes[index];
  }

  /** Set a single amplitude by basis state index */
  setAmplitude(index: number, value: Complex): void {
    if (index < 0 || index >= this.dimension) {
      throw new QftError(
        QftErrorCode.IndexOutOfRange,
        `Index ${index} out of range for ${this._numQubits} qubits`
      );
    }
    this._amplitudes[index] = value;
  }

  /** Set all amplitudes */
  setAmplitudes(amplitudes: Complex[]): void {
    if (amplitudes.length !== this.dimension) {
      throw new QftError(
        QftErrorCode.DimensionMismatch,
        `Expected ${this.dimension} amplitudes, got ${amplitudes.length}`
      );
    }
    this._amplitudes = [...amplitudes];
  }

  // =========================================================================
  // Metadata
  // =========================================================================

  /** Set a metadata key-value pair */
  setMetadata(key: string, value: string): void {
    this._metadata.set(key, value);
  }

  /** Get a metadata value by key */
  getMetadata(key: string): string | undefined {
    return this._metadata.get(key);
  }

  /** Get all metadata as an object */
  get metadata(): Record<string, string> {
    return Object.fromEntries(this._metadata);
  }

  // =========================================================================
  // State Operations
  // =========================================================================

  /** Calculate the norm squared (sum of |amplitude|²) */
  normSquared(): number {
    return this._amplitudes.reduce((sum, a) => sum + a.normSqr(), 0);
  }

  /** Calculate the norm */
  norm(): number {
    return Math.sqrt(this.normSquared());
  }

  /** Check if the state is normalized within tolerance */
  isNormalized(tolerance = 1e-10): boolean {
    return Math.abs(this.normSquared() - 1.0) < tolerance;
  }

  /** Normalize the state vector in place */
  normalize(): void {
    const n = this.norm();
    if (n < 1e-15) {
      throw new QftError(QftErrorCode.ZeroNorm, 'Cannot normalize zero state');
    }
    this._amplitudes = this._amplitudes.map((a) => a.div(n));
  }

  /** Calculate the inner product ⟨this|other⟩ */
  innerProduct(other: QftFile): Complex {
    if (this._numQubits !== other._numQubits) {
      throw new QftError(
        QftErrorCode.DimensionMismatch,
        `Dimension mismatch: ${this._numQubits} vs ${other._numQubits} qubits`
      );
    }

    let result = ZERO;
    for (let i = 0; i < this.dimension; i++) {
      result = result.add(this._amplitudes[i].conj().mul(other._amplitudes[i]));
    }
    return result;
  }

  /** Calculate the fidelity |⟨this|other⟩|² */
  fidelity(other: QftFile): number {
    return this.innerProduct(other).normSqr();
  }

  /** Calculate the trace distance */
  traceDistance(other: QftFile): number {
    return Math.sqrt(1 - this.fidelity(other));
  }

  /** Create a deep copy */
  clone(): QftFile {
    const copy = new QftFile(this._numQubits, this._config);
    copy._amplitudes = [...this._amplitudes];
    copy._metadata = new Map(this._metadata);
    return copy;
  }

  // =========================================================================
  // Serialization
  // =========================================================================

  /** Serialize to Uint8Array */
  toBytes(): Uint8Array {
    const dim = this.dimension;
    const buffer = new ArrayBuffer(16 + dim * 16);
    const view = new DataView(buffer);

    // Magic number "QFT\x01"
    view.setUint8(0, 0x51); // Q
    view.setUint8(1, 0x46); // F
    view.setUint8(2, 0x54); // T
    view.setUint8(3, 0x01); // version

    // Header
    view.setUint8(4, this._numQubits);
    view.setUint8(5, Math.min(this._config.bondDimension, 255));
    view.setUint8(6, this._config.golayEnabled ? 1 : 0);
    view.setUint8(7, 0); // reserved

    // Padding (8 bytes)
    for (let i = 8; i < 16; i++) {
      view.setUint8(i, 0);
    }

    // Amplitudes (little-endian f64 pairs)
    for (let i = 0; i < dim; i++) {
      const offset = 16 + i * 16;
      view.setFloat64(offset, this._amplitudes[i].re, true);
      view.setFloat64(offset + 8, this._amplitudes[i].im, true);
    }

    return new Uint8Array(buffer);
  }

  /** Deserialize from Uint8Array */
  static fromBytes(data: Uint8Array): QftFile {
    if (data.length < 16) {
      throw new QftError(QftErrorCode.InvalidFormat, 'File too small');
    }

    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

    // Verify magic number
    if (
      view.getUint8(0) !== 0x51 ||
      view.getUint8(1) !== 0x46 ||
      view.getUint8(2) !== 0x54 ||
      view.getUint8(3) !== 0x01
    ) {
      throw new QftError(QftErrorCode.InvalidFormat, 'Invalid magic number');
    }

    const numQubits = view.getUint8(4);
    if (numQubits < 1 || numQubits > 30) {
      throw new QftError(QftErrorCode.InvalidQubits, `Invalid num_qubits: ${numQubits}`);
    }

    const bondDimension = view.getUint8(5) || 64;
    const golayEnabled = view.getUint8(6) !== 0;

    const dim = 1 << numQubits;
    const expectedSize = 16 + dim * 16;

    if (data.length < expectedSize) {
      throw new QftError(
        QftErrorCode.InvalidFormat,
        `File truncated: expected ${expectedSize} bytes, got ${data.length}`
      );
    }

    const amplitudes: Complex[] = [];
    for (let i = 0; i < dim; i++) {
      const offset = 16 + i * 16;
      const re = view.getFloat64(offset, true);
      const im = view.getFloat64(offset + 8, true);
      amplitudes.push(new Complex(re, im));
    }

    return QftFile.fromAmplitudes(amplitudes, {
      bondDimension,
      golayEnabled,
    });
  }

  /** Export to JSON */
  toJSON(): string {
    return JSON.stringify(
      {
        numQubits: this._numQubits,
        config: this._config,
        amplitudesReal: this.amplitudesReal,
        amplitudesImag: this.amplitudesImag,
        metadata: this.metadata,
      },
      null,
      2
    );
  }

  /** Import from JSON */
  static fromJSON(json: string): QftFile {
    const data = JSON.parse(json);
    const file = QftFile.fromRealImag(data.amplitudesReal, data.amplitudesImag, data.config);
    if (data.metadata) {
      for (const [key, value] of Object.entries(data.metadata)) {
        file.setMetadata(key, value as string);
      }
    }
    return file;
  }

  /** String representation */
  toString(): string {
    return `QftFile(${this._numQubits} qubits, dim=${this.dimension}, norm=${this.norm().toFixed(6)})`;
  }
}

// =============================================================================
// Section 4: QftBuilder Class
// =============================================================================

/** Builder for constructing QFT files with a fluent interface */
export class QftBuilder {
  private _numQubits: number;
  private _config: Partial<QftConfig> = {};
  private _metadata: Map<string, string> = new Map();
  private _amplitudes?: Complex[];

  constructor(numQubits: number) {
    this._numQubits = numQubits;
  }

  /** Set bond dimension */
  bondDimension(dim: number): this {
    this._config.bondDimension = dim;
    return this;
  }

  /** Enable/disable Golay error correction */
  golay(enabled: boolean): this {
    this._config.golayEnabled = enabled;
    return this;
  }

  /** Set truncation threshold */
  truncationThreshold(threshold: number): this {
    this._config.truncationThreshold = threshold;
    return this;
  }

  /** Add metadata entry */
  metadata(key: string, value: string): this {
    this._metadata.set(key, value);
    return this;
  }

  /** Set initial amplitudes */
  amplitudes(amplitudes: Complex[]): this {
    this._amplitudes = amplitudes;
    return this;
  }

  /** Build the QftFile */
  build(): QftFile {
    const file = new QftFile(this._numQubits, this._config);
    if (this._amplitudes) {
      file.setAmplitudes(this._amplitudes);
    }
    for (const [key, value] of this._metadata) {
      file.setMetadata(key, value);
    }
    return file;
  }
}

// =============================================================================
// Section 5: Common States
// =============================================================================

/** Create a Bell state (|00⟩ + |11⟩) / √2 */
export function bellState(): QftFile {
  const sqrt2Inv = 1 / Math.sqrt(2);
  return QftFile.fromAmplitudes([
    new Complex(sqrt2Inv),
    ZERO,
    ZERO,
    new Complex(sqrt2Inv),
  ]);
}

/** Create a GHZ state (|0...0⟩ + |1...1⟩) / √2 */
export function ghzState(numQubits: number): QftFile {
  const file = new QftFile(numQubits);
  const sqrt2Inv = 1 / Math.sqrt(2);
  file.setAmplitude(0, new Complex(sqrt2Inv));
  file.setAmplitude(file.dimension - 1, new Complex(sqrt2Inv));
  return file;
}

/** Create a uniform superposition state */
export function uniformState(numQubits: number): QftFile {
  const dim = 1 << numQubits;
  const amp = 1 / Math.sqrt(dim);
  const amplitudes = new Array(dim).fill(null).map(() => new Complex(amp));
  return QftFile.fromAmplitudes(amplitudes);
}

/** Create a computational basis state |i⟩ */
export function basisState(numQubits: number, index: number): QftFile {
  const file = new QftFile(numQubits);
  if (index < 0 || index >= file.dimension) {
    throw new QftError(
      QftErrorCode.IndexOutOfRange,
      `Index ${index} out of range for ${numQubits} qubits`
    );
  }
  file.setAmplitude(0, ZERO);
  file.setAmplitude(index, ONE);
  return file;
}

// =============================================================================
// Section 6: I/O Operations (Node.js)
// =============================================================================

/** Load a QFT file from disk (Node.js only) */
export async function load(path: string): Promise<QftFile> {
  if (typeof globalThis.process === 'undefined') {
    throw new QftError(QftErrorCode.IoError, 'File I/O only available in Node.js');
  }
  const fs = await import('fs/promises');
  const data = await fs.readFile(path);
  return QftFile.fromBytes(new Uint8Array(data));
}

/** Save a QFT file to disk (Node.js only) */
export async function save(file: QftFile, path: string): Promise<void> {
  if (typeof globalThis.process === 'undefined') {
    throw new QftError(QftErrorCode.IoError, 'File I/O only available in Node.js');
  }
  const fs = await import('fs/promises');
  await fs.writeFile(path, file.toBytes());
}

// =============================================================================
// Section 7: Validation Functions
// =============================================================================

/** Verify a QFT file's integrity */
export function verify(data: Uint8Array): boolean {
  try {
    const file = QftFile.fromBytes(data);
    return file.isNormalized(1e-6);
  } catch {
    return false;
  }
}

/** Calculate fidelity between two byte arrays */
export function fidelityFromBytes(data1: Uint8Array, data2: Uint8Array): number {
  const file1 = QftFile.fromBytes(data1);
  const file2 = QftFile.fromBytes(data2);
  return file1.fidelity(file2);
}

/** Library version */
export const VERSION = '1.0.0';

// =============================================================================
// Section 8: Exports
// =============================================================================

export default {
  QftFile,
  QftBuilder,
  Complex,
  QftError,
  QftErrorCode,
  bellState,
  ghzState,
  uniformState,
  basisState,
  load,
  save,
  verify,
  fidelityFromBytes,
  VERSION,
  ZERO,
  ONE,
  I,
  DEFAULT_CONFIG,
};
