package dev.macrohard.qft;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;

import java.io.*;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;

/**
 * A quantum state file in .qft format.
 * 
 * <p>Production-grade Java SDK for working with quantum states in the Quantum File Type format.
 * Supports state creation, manipulation, serialization, and I/O operations.</p>
 * 
 * <h2>Example Usage:</h2>
 * <pre>{@code
 * // Create a 4-qubit state
 * QftFile state = new QftFile(4);
 * System.out.println("Qubits: " + state.getNumQubits());
 * 
 * // Set Bell-like state
 * double sqrt2Inv = 1.0 / Math.sqrt(2.0);
 * state.setAmplitude(0, sqrt2Inv, 0);
 * state.setAmplitude(15, sqrt2Inv, 0);
 * 
 * // Save and load
 * state.save("state.qft");
 * QftFile loaded = QftFile.load("state.qft");
 * }</pre>
 * 
 * @author MACROHARD Quantum OS Team
 * @version 1.0.0
 */
public class QftFile {
    
    /** Library version */
    public static final String VERSION = "1.0.0";
    
    /** Magic number for QFT files */
    private static final byte[] MAGIC = {'Q', 'F', 'T', 0x01};
    
    private final int numQubits;
    private final double[] amplitudesReal;
    private final double[] amplitudesImag;
    private final Map<String, String> metadata;
    private QftConfig config;
    
    /**
     * Creates a new QFT file with the specified number of qubits.
     * Initializes to the |0...0⟩ state.
     * 
     * @param numQubits Number of qubits (1-30)
     * @throws IllegalArgumentException if numQubits is not in range 1-30
     */
    public QftFile(int numQubits) {
        this(numQubits, QftConfig.defaultConfig());
    }
    
    /**
     * Creates a new QFT file with custom configuration.
     * 
     * @param numQubits Number of qubits (1-30)
     * @param config Configuration settings
     * @throws IllegalArgumentException if numQubits is not in range 1-30
     */
    public QftFile(int numQubits, QftConfig config) {
        if (numQubits < 1 || numQubits > 30) {
            throw new IllegalArgumentException("numQubits must be 1-30, got " + numQubits);
        }
        
        this.numQubits = numQubits;
        this.config = config;
        this.metadata = new HashMap<>();
        
        int dim = 1 << numQubits;
        this.amplitudesReal = new double[dim];
        this.amplitudesImag = new double[dim];
        this.amplitudesReal[0] = 1.0; // |0...0⟩
    }
    
    /**
     * Creates a QFT file from amplitude arrays.
     * 
     * @param real Real parts of amplitudes
     * @param imag Imaginary parts of amplitudes
     * @return New QftFile instance
     * @throws IllegalArgumentException if arrays have different lengths or length is not a power of 2
     */
    public static QftFile fromAmplitudes(double[] real, double[] imag) {
        if (real.length != imag.length) {
            throw new IllegalArgumentException("Real and imag arrays must have same length");
        }
        
        int dim = real.length;
        if (dim == 0 || (dim & (dim - 1)) != 0) {
            throw new IllegalArgumentException("Amplitude count must be a power of 2");
        }
        
        int numQubits = Integer.numberOfTrailingZeros(dim);
        if (numQubits > 30) {
            throw new IllegalArgumentException("Too many qubits: " + numQubits);
        }
        
        QftFile file = new QftFile(numQubits);
        System.arraycopy(real, 0, file.amplitudesReal, 0, dim);
        System.arraycopy(imag, 0, file.amplitudesImag, 0, dim);
        return file;
    }
    
    // =========================================================================
    // Properties
    // =========================================================================
    
    /** @return Number of qubits */
    public int getNumQubits() {
        return numQubits;
    }
    
    /** @return State vector dimension (2^numQubits) */
    public int getDimension() {
        return 1 << numQubits;
    }
    
    /** @return Current configuration */
    public QftConfig getConfig() {
        return config;
    }
    
    /** @param config New configuration */
    public void setConfig(QftConfig config) {
        this.config = config;
    }
    
    // =========================================================================
    // Amplitude Access
    // =========================================================================
    
    /** @return Copy of real parts of amplitudes */
    public double[] getAmplitudesReal() {
        return amplitudesReal.clone();
    }
    
    /** @return Copy of imaginary parts of amplitudes */
    public double[] getAmplitudesImag() {
        return amplitudesImag.clone();
    }
    
    /**
     * Gets a single amplitude by basis state index.
     * 
     * @param index Basis state index (0 to 2^n - 1)
     * @return Array of [real, imag]
     * @throws IndexOutOfBoundsException if index is out of range
     */
    public double[] getAmplitude(int index) {
        if (index < 0 || index >= getDimension()) {
            throw new IndexOutOfBoundsException("Index " + index + " out of range for " + numQubits + " qubits");
        }
        return new double[]{amplitudesReal[index], amplitudesImag[index]};
    }
    
    /**
     * Sets a single amplitude by basis state index.
     * 
     * @param index Basis state index
     * @param real Real part
     * @param imag Imaginary part
     * @throws IndexOutOfBoundsException if index is out of range
     */
    public void setAmplitude(int index, double real, double imag) {
        if (index < 0 || index >= getDimension()) {
            throw new IndexOutOfBoundsException("Index " + index + " out of range for " + numQubits + " qubits");
        }
        amplitudesReal[index] = real;
        amplitudesImag[index] = imag;
    }
    
    /**
     * Sets all amplitudes.
     * 
     * @param real Real parts
     * @param imag Imaginary parts
     * @throws IllegalArgumentException if arrays have wrong length
     */
    public void setAmplitudes(double[] real, double[] imag) {
        int dim = getDimension();
        if (real.length != dim || imag.length != dim) {
            throw new IllegalArgumentException("Expected " + dim + " amplitudes");
        }
        System.arraycopy(real, 0, amplitudesReal, 0, dim);
        System.arraycopy(imag, 0, amplitudesImag, 0, dim);
    }
    
    // =========================================================================
    // Metadata
    // =========================================================================
    
    /**
     * Sets a metadata key-value pair.
     * 
     * @param key Metadata key
     * @param value Metadata value
     */
    public void setMetadata(String key, String value) {
        metadata.put(key, value);
    }
    
    /**
     * Gets a metadata value by key.
     * 
     * @param key Metadata key
     * @return Value or null if not found
     */
    public String getMetadata(String key) {
        return metadata.get(key);
    }
    
    /** @return Copy of all metadata */
    public Map<String, String> getAllMetadata() {
        return new HashMap<>(metadata);
    }
    
    // =========================================================================
    // State Operations
    // =========================================================================
    
    /** @return Norm squared (sum of |amplitude|²) */
    public double normSquared() {
        double sum = 0;
        for (int i = 0; i < getDimension(); i++) {
            sum += amplitudesReal[i] * amplitudesReal[i] + amplitudesImag[i] * amplitudesImag[i];
        }
        return sum;
    }
    
    /** @return Norm */
    public double norm() {
        return Math.sqrt(normSquared());
    }
    
    /**
     * Checks if the state is normalized within tolerance.
     * 
     * @param tolerance Tolerance for check
     * @return true if normalized
     */
    public boolean isNormalized(double tolerance) {
        return Math.abs(normSquared() - 1.0) < tolerance;
    }
    
    /**
     * Normalizes the state vector in place.
     * 
     * @throws IllegalStateException if state has zero norm
     */
    public void normalize() {
        double n = norm();
        if (n < 1e-15) {
            throw new IllegalStateException("Cannot normalize zero state");
        }
        for (int i = 0; i < getDimension(); i++) {
            amplitudesReal[i] /= n;
            amplitudesImag[i] /= n;
        }
    }
    
    /**
     * Calculates the inner product ⟨this|other⟩.
     * 
     * @param other Another QftFile
     * @return Array of [real, imag] representing the complex inner product
     * @throws IllegalArgumentException if dimensions don't match
     */
    public double[] innerProduct(QftFile other) {
        if (this.numQubits != other.numQubits) {
            throw new IllegalArgumentException("Dimension mismatch: " + this.numQubits + " vs " + other.numQubits);
        }
        
        double realPart = 0;
        double imagPart = 0;
        
        for (int i = 0; i < getDimension(); i++) {
            // conj(this) * other = (a - bi)(c + di) = ac + bd + i(ad - bc)
            double a = amplitudesReal[i];
            double b = amplitudesImag[i];
            double c = other.amplitudesReal[i];
            double d = other.amplitudesImag[i];
            
            realPart += a * c + b * d;
            imagPart += a * d - b * c;
        }
        
        return new double[]{realPart, imagPart};
    }
    
    /**
     * Calculates the fidelity |⟨this|other⟩|².
     * 
     * @param other Another QftFile
     * @return Fidelity between 0 and 1
     */
    public double fidelity(QftFile other) {
        double[] overlap = innerProduct(other);
        return overlap[0] * overlap[0] + overlap[1] * overlap[1];
    }
    
    /**
     * Calculates the trace distance.
     * 
     * @param other Another QftFile
     * @return Trace distance
     */
    public double traceDistance(QftFile other) {
        return Math.sqrt(1.0 - fidelity(other));
    }
    
    /**
     * Creates a deep copy.
     * 
     * @return New QftFile with same state
     */
    public QftFile clone() {
        QftFile copy = new QftFile(numQubits, config);
        System.arraycopy(amplitudesReal, 0, copy.amplitudesReal, 0, getDimension());
        System.arraycopy(amplitudesImag, 0, copy.amplitudesImag, 0, getDimension());
        copy.metadata.putAll(metadata);
        return copy;
    }
    
    // =========================================================================
    // I/O Operations
    // =========================================================================
    
    /**
     * Loads a QFT file from disk.
     * 
     * @param path File path
     * @return Loaded QftFile
     * @throws IOException if file cannot be read
     * @throws QftFormatException if file format is invalid
     */
    public static QftFile load(String path) throws IOException, QftFormatException {
        return load(Path.of(path));
    }
    
    /**
     * Loads a QFT file from disk.
     * 
     * @param path File path
     * @return Loaded QftFile
     * @throws IOException if file cannot be read
     * @throws QftFormatException if file format is invalid
     */
    public static QftFile load(Path path) throws IOException, QftFormatException {
        byte[] data = Files.readAllBytes(path);
        return fromBytes(data);
    }
    
    /**
     * Saves the QFT file to disk.
     * 
     * @param path File path
     * @throws IOException if file cannot be written
     */
    public void save(String path) throws IOException {
        save(Path.of(path));
    }
    
    /**
     * Saves the QFT file to disk.
     * 
     * @param path File path
     * @throws IOException if file cannot be written
     */
    public void save(Path path) throws IOException {
        Files.write(path, toBytes());
    }
    
    /**
     * Serializes to a byte array.
     * 
     * @return Byte array representation
     */
    public byte[] toBytes() {
        int dim = getDimension();
        ByteBuffer buffer = ByteBuffer.allocate(16 + dim * 16);
        buffer.order(ByteOrder.LITTLE_ENDIAN);
        
        // Magic number
        buffer.put(MAGIC);
        
        // Header
        buffer.put((byte) numQubits);
        buffer.put((byte) Math.min(config.getBondDimension(), 255));
        buffer.put((byte) (config.isGolayEnabled() ? 1 : 0));
        buffer.put((byte) 0); // reserved
        
        // Padding to 16 bytes
        buffer.put(new byte[8]);
        
        // Amplitudes
        for (int i = 0; i < dim; i++) {
            buffer.putDouble(amplitudesReal[i]);
            buffer.putDouble(amplitudesImag[i]);
        }
        
        return buffer.array();
    }
    
    /**
     * Deserializes from a byte array.
     * 
     * @param data Byte array
     * @return QftFile instance
     * @throws QftFormatException if format is invalid
     */
    public static QftFile fromBytes(byte[] data) throws QftFormatException {
        if (data.length < 16) {
            throw new QftFormatException("File too small");
        }
        
        ByteBuffer buffer = ByteBuffer.wrap(data);
        buffer.order(ByteOrder.LITTLE_ENDIAN);
        
        // Verify magic
        byte[] magic = new byte[4];
        buffer.get(magic);
        if (magic[0] != 'Q' || magic[1] != 'F' || magic[2] != 'T' || magic[3] != 0x01) {
            throw new QftFormatException("Invalid magic number");
        }
        
        int numQubits = buffer.get() & 0xFF;
        if (numQubits < 1 || numQubits > 30) {
            throw new QftFormatException("Invalid numQubits: " + numQubits);
        }
        
        int bondDim = buffer.get() & 0xFF;
        if (bondDim == 0) bondDim = 64;
        boolean golayEnabled = buffer.get() != 0;
        buffer.get(); // reserved
        
        // Skip padding
        buffer.position(16);
        
        int dim = 1 << numQubits;
        int expectedSize = 16 + dim * 16;
        
        if (data.length < expectedSize) {
            throw new QftFormatException("File truncated: expected " + expectedSize + " bytes, got " + data.length);
        }
        
        QftConfig config = new QftConfig(bondDim, golayEnabled, 1e-10);
        QftFile file = new QftFile(numQubits, config);
        
        for (int i = 0; i < dim; i++) {
            file.amplitudesReal[i] = buffer.getDouble();
            file.amplitudesImag[i] = buffer.getDouble();
        }
        
        return file;
    }
    
    /**
     * Exports to JSON string.
     * 
     * @return JSON representation
     */
    public String toJson() {
        Gson gson = new GsonBuilder().setPrettyPrinting().create();
        JsonExport export = new JsonExport();
        export.numQubits = numQubits;
        export.config = config;
        export.amplitudesReal = amplitudesReal.clone();
        export.amplitudesImag = amplitudesImag.clone();
        export.metadata = new HashMap<>(metadata);
        return gson.toJson(export);
    }
    
    /**
     * Imports from JSON string.
     * 
     * @param json JSON string
     * @return QftFile instance
     */
    public static QftFile fromJson(String json) {
        Gson gson = new Gson();
        JsonExport export = gson.fromJson(json, JsonExport.class);
        
        QftFile file = new QftFile(export.numQubits, export.config != null ? export.config : QftConfig.defaultConfig());
        System.arraycopy(export.amplitudesReal, 0, file.amplitudesReal, 0, file.getDimension());
        System.arraycopy(export.amplitudesImag, 0, file.amplitudesImag, 0, file.getDimension());
        if (export.metadata != null) {
            file.metadata.putAll(export.metadata);
        }
        return file;
    }
    
    @Override
    public String toString() {
        return String.format("QftFile(%d qubits, dim=%d, norm=%.6f)", numQubits, getDimension(), norm());
    }
    
    // =========================================================================
    // Common States (Static Factory Methods)
    // =========================================================================
    
    /**
     * Creates a Bell state (|00⟩ + |11⟩) / √2.
     * 
     * @return Bell state
     */
    public static QftFile bellState() {
        double sqrt2Inv = 1.0 / Math.sqrt(2.0);
        double[] real = {sqrt2Inv, 0, 0, sqrt2Inv};
        double[] imag = {0, 0, 0, 0};
        return fromAmplitudes(real, imag);
    }
    
    /**
     * Creates a GHZ state (|0...0⟩ + |1...1⟩) / √2.
     * 
     * @param numQubits Number of qubits
     * @return GHZ state
     */
    public static QftFile ghzState(int numQubits) {
        QftFile file = new QftFile(numQubits);
        double sqrt2Inv = 1.0 / Math.sqrt(2.0);
        file.amplitudesReal[0] = sqrt2Inv;
        file.amplitudesReal[file.getDimension() - 1] = sqrt2Inv;
        return file;
    }
    
    /**
     * Creates a uniform superposition state.
     * 
     * @param numQubits Number of qubits
     * @return Uniform state
     */
    public static QftFile uniformState(int numQubits) {
        QftFile file = new QftFile(numQubits);
        double amp = 1.0 / Math.sqrt(file.getDimension());
        for (int i = 0; i < file.getDimension(); i++) {
            file.amplitudesReal[i] = amp;
        }
        return file;
    }
    
    /**
     * Creates a computational basis state |i⟩.
     * 
     * @param numQubits Number of qubits
     * @param index Basis state index
     * @return Basis state
     */
    public static QftFile basisState(int numQubits, int index) {
        QftFile file = new QftFile(numQubits);
        if (index < 0 || index >= file.getDimension()) {
            throw new IndexOutOfBoundsException("Index " + index + " out of range");
        }
        file.amplitudesReal[0] = 0;
        file.amplitudesReal[index] = 1.0;
        return file;
    }
    
    // =========================================================================
    // Internal Classes
    // =========================================================================
    
    private static class JsonExport {
        int numQubits;
        QftConfig config;
        double[] amplitudesReal;
        double[] amplitudesImag;
        Map<String, String> metadata;
    }
}
