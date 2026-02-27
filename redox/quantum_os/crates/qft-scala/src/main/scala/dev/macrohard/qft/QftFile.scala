package dev.v-os.qft

import java.io.{ByteArrayInputStream, ByteArrayOutputStream, DataInputStream, DataOutputStream}
import java.nio.{ByteBuffer, ByteOrder}
import java.nio.file.{Files, Path}
import scala.collection.mutable
import scala.util.{Try, Success, Failure}

/**
 * Configuration for QFT file operations.
 *
 * @param bondDimension Bond dimension for MPS compression (1-1024)
 * @param golayEnabled Enable Golay error correction
 * @param truncationThreshold Truncation threshold for SVD
 */
case class QftConfig(
  bondDimension: Int = 64,
  golayEnabled: Boolean = true,
  truncationThreshold: Double = 1e-10
)

/**
 * Complex number representation.
 */
case class Complex(re: Double, im: Double = 0.0) {
  def conj: Complex = Complex(re, -im)
  def normSqr: Double = re * re + im * im
  def norm: Double = Math.sqrt(normSqr)
  
  def +(other: Complex): Complex = Complex(re + other.re, im + other.im)
  def -(other: Complex): Complex = Complex(re - other.re, im - other.im)
  def *(other: Complex): Complex = Complex(
    re * other.re - im * other.im,
    re * other.im + im * other.re
  )
  def *(scalar: Double): Complex = Complex(re * scalar, im * scalar)
  def /(scalar: Double): Complex = Complex(re / scalar, im / scalar)
  
  override def toString: String = {
    if (im == 0) s"$re"
    else if (re == 0) s"${im}i"
    else {
      val sign = if (im >= 0) "+" else "-"
      s"$re$sign${Math.abs(im)}i"
    }
  }
}

object Complex {
  val Zero: Complex = Complex(0, 0)
  val One: Complex = Complex(1, 0)
  val I: Complex = Complex(0, 1)
  
  def fromPolar(r: Double, theta: Double): Complex =
    Complex(r * Math.cos(theta), r * Math.sin(theta))
}

/**
 * Exception for QFT format errors.
 */
class QftFormatException(message: String) extends Exception(message)

/**
 * A quantum state file in .qft format.
 *
 * Production-grade Scala SDK for working with quantum states in the Quantum File Type format.
 *
 * ==Example Usage:==
 * {{{
 * import dev.v-os.qft._
 *
 * // Create a 4-qubit state
 * val state = QftFile(4)
 * println(s"Qubits: ${state.numQubits}")
 *
 * // Set Bell-like state
 * val sqrt2Inv = 1.0 / Math.sqrt(2.0)
 * state(0) = Complex(sqrt2Inv)
 * state(15) = Complex(sqrt2Inv)
 *
 * // Save and load
 * state.save("state.qft")
 * val loaded = QftFile.load("state.qft")
 * }}}
 *
 * @param numQubits Number of qubits (1-30)
 * @param config Configuration settings
 */
class QftFile private (
  val numQubits: Int,
  private val _amplitudes: Array[Complex],
  private val _metadata: mutable.Map[String, String],
  var config: QftConfig
) {
  
  require(numQubits >= 1 && numQubits <= 30, s"numQubits must be 1-30, got $numQubits")
  
  /** State vector dimension (2^numQubits) */
  def dimension: Int = 1 << numQubits
  
  // =========================================================================
  // Amplitude Access
  // =========================================================================
  
  /** Get all amplitudes */
  def amplitudes: IndexedSeq[Complex] = _amplitudes.toIndexedSeq
  
  /** Get real parts of amplitudes */
  def amplitudesReal: IndexedSeq[Double] = _amplitudes.map(_.re).toIndexedSeq
  
  /** Get imaginary parts of amplitudes */
  def amplitudesImag: IndexedSeq[Double] = _amplitudes.map(_.im).toIndexedSeq
  
  /** Get amplitude by index */
  def apply(index: Int): Complex = {
    require(index >= 0 && index < dimension, s"Index $index out of range for $numQubits qubits")
    _amplitudes(index)
  }
  
  /** Set amplitude by index */
  def update(index: Int, value: Complex): Unit = {
    require(index >= 0 && index < dimension, s"Index $index out of range for $numQubits qubits")
    _amplitudes(index) = value
  }
  
  /** Set all amplitudes */
  def setAmplitudes(amplitudes: Seq[Complex]): Unit = {
    require(amplitudes.length == dimension, s"Expected $dimension amplitudes, got ${amplitudes.length}")
    amplitudes.copyToArray(_amplitudes)
  }
  
  // =========================================================================
  // Metadata
  // =========================================================================
  
  /** Set metadata key-value pair */
  def setMetadata(key: String, value: String): Unit = _metadata(key) = value
  
  /** Get metadata value */
  def getMetadata(key: String): Option[String] = _metadata.get(key)
  
  /** Get all metadata */
  def metadata: Map[String, String] = _metadata.toMap
  
  // =========================================================================
  // State Operations
  // =========================================================================
  
  /** Norm squared (sum of |amplitude|²) */
  def normSquared: Double = _amplitudes.map(_.normSqr).sum
  
  /** Norm */
  def norm: Double = Math.sqrt(normSquared)
  
  /** Check if normalized within tolerance */
  def isNormalized(tolerance: Double = 1e-10): Boolean =
    Math.abs(normSquared - 1.0) < tolerance
  
  /** Normalize in place */
  def normalize(): Unit = {
    val n = norm
    require(n >= 1e-15, "Cannot normalize zero state")
    for (i <- _amplitudes.indices) {
      _amplitudes(i) = _amplitudes(i) / n
    }
  }
  
  /** Inner product ⟨this|other⟩ */
  def innerProduct(other: QftFile): Complex = {
    require(numQubits == other.numQubits, s"Dimension mismatch: $numQubits vs ${other.numQubits}")
    _amplitudes.zip(other._amplitudes)
      .map { case (a, b) => a.conj * b }
      .reduce(_ + _)
  }
  
  /** Fidelity |⟨this|other⟩|² */
  def fidelity(other: QftFile): Double = innerProduct(other).normSqr
  
  /** Trace distance */
  def traceDistance(other: QftFile): Double = Math.sqrt(1.0 - fidelity(other))
  
  /** Deep copy */
  def copy: QftFile = new QftFile(
    numQubits,
    _amplitudes.clone(),
    mutable.Map.from(_metadata),
    config
  )
  
  // =========================================================================
  // I/O Operations
  // =========================================================================
  
  /** Save to file */
  def save(path: String): Unit = save(Path.of(path))
  
  /** Save to file */
  def save(path: Path): Unit = Files.write(path, toBytes)
  
  /** Serialize to bytes */
  def toBytes: Array[Byte] = {
    val buffer = ByteBuffer.allocate(16 + dimension * 16)
    buffer.order(ByteOrder.LITTLE_ENDIAN)
    
    // Magic number
    buffer.put(Array[Byte]('Q', 'F', 'T', 0x01))
    
    // Header
    buffer.put(numQubits.toByte)
    buffer.put(Math.min(config.bondDimension, 255).toByte)
    buffer.put((if (config.golayEnabled) 1 else 0).toByte)
    buffer.put(0.toByte) // reserved
    
    // Padding
    buffer.put(new Array[Byte](8))
    
    // Amplitudes
    for (a <- _amplitudes) {
      buffer.putDouble(a.re)
      buffer.putDouble(a.im)
    }
    
    buffer.array()
  }
  
  /** Export to JSON */
  def toJson: String = {
    import com.google.gson.{Gson, GsonBuilder}
    val gson = new GsonBuilder().setPrettyPrinting().create()
    
    val map = new java.util.HashMap[String, Any]()
    map.put("numQubits", numQubits)
    map.put("bondDimension", config.bondDimension)
    map.put("golayEnabled", config.golayEnabled)
    map.put("truncationThreshold", config.truncationThreshold)
    map.put("amplitudesReal", amplitudesReal.toArray)
    map.put("amplitudesImag", amplitudesImag.toArray)
    map.put("metadata", mapToJava(_metadata))
    
    gson.toJson(map)
  }
  
  override def toString: String =
    f"QftFile($numQubits qubits, dim=$dimension, norm=${norm}%.6f)"
  
  private implicit def mapToJava[K, V](m: mutable.Map[K, V]): java.util.Map[K, V] = {
    val jmap = new java.util.HashMap[K, V]()
    m.foreach { case (k, v) => jmap.put(k, v) }
    jmap
  }
}

object QftFile {
  
  /** Library version */
  val Version: String = "1.0.0"
  
  /** Magic number */
  private val Magic: Array[Byte] = Array('Q', 'F', 'T', 0x01)
  
  /**
   * Create a new QFT file initialized to |0...0⟩.
   *
   * @param numQubits Number of qubits (1-30)
   * @param config Configuration settings
   */
  def apply(numQubits: Int, config: QftConfig = QftConfig()): QftFile = {
    require(numQubits >= 1 && numQubits <= 30, s"numQubits must be 1-30, got $numQubits")
    
    val dim = 1 << numQubits
    val amplitudes = Array.fill(dim)(Complex.Zero)
    amplitudes(0) = Complex.One
    
    new QftFile(numQubits, amplitudes, mutable.Map.empty, config)
  }
  
  /**
   * Create from amplitude sequence.
   */
  def fromAmplitudes(amplitudes: Seq[Complex], config: QftConfig = QftConfig()): QftFile = {
    val dim = amplitudes.length
    require(dim > 0 && (dim & (dim - 1)) == 0, "Amplitude count must be a power of 2")
    
    val numQubits = Integer.numberOfTrailingZeros(dim)
    require(numQubits <= 30, s"Too many qubits: $numQubits")
    
    new QftFile(numQubits, amplitudes.toArray, mutable.Map.empty, config)
  }
  
  /**
   * Create from separate real and imaginary arrays.
   */
  def fromRealImag(real: Seq[Double], imag: Seq[Double], config: QftConfig = QftConfig()): QftFile = {
    require(real.length == imag.length, "Real and imag must have same length")
    fromAmplitudes(real.zip(imag).map { case (r, i) => Complex(r, i) }, config)
  }
  
  /**
   * Load from file.
   */
  def load(path: String): Try[QftFile] = load(Path.of(path))
  
  /**
   * Load from file.
   */
  def load(path: Path): Try[QftFile] = Try {
    fromBytes(Files.readAllBytes(path))
  }.flatten
  
  /**
   * Deserialize from bytes.
   */
  def fromBytes(data: Array[Byte]): Try[QftFile] = Try {
    if (data.length < 16) throw new QftFormatException("File too small")
    
    val buffer = ByteBuffer.wrap(data)
    buffer.order(ByteOrder.LITTLE_ENDIAN)
    
    // Verify magic
    val magic = new Array[Byte](4)
    buffer.get(magic)
    if (!magic.sameElements(Magic)) throw new QftFormatException("Invalid magic number")
    
    val numQubits = buffer.get() & 0xFF
    if (numQubits < 1 || numQubits > 30) throw new QftFormatException(s"Invalid numQubits: $numQubits")
    
    val bondDim = {
      val b = buffer.get() & 0xFF
      if (b == 0) 64 else b
    }
    val golayEnabled = buffer.get() != 0
    buffer.get() // reserved
    
    // Skip padding
    buffer.position(16)
    
    val dim = 1 << numQubits
    val expectedSize = 16 + dim * 16
    if (data.length < expectedSize) throw new QftFormatException(s"File truncated")
    
    val amplitudes = Array.tabulate(dim) { _ =>
      Complex(buffer.getDouble(), buffer.getDouble())
    }
    
    new QftFile(
      numQubits,
      amplitudes,
      mutable.Map.empty,
      QftConfig(bondDim, golayEnabled)
    )
  }
  
  // =========================================================================
  // Common States
  // =========================================================================
  
  /** Bell state (|00⟩ + |11⟩) / √2 */
  def bellState: QftFile = {
    val sqrt2Inv = 1.0 / Math.sqrt(2.0)
    fromAmplitudes(Seq(
      Complex(sqrt2Inv), Complex.Zero, Complex.Zero, Complex(sqrt2Inv)
    ))
  }
  
  /** GHZ state (|0...0⟩ + |1...1⟩) / √2 */
  def ghzState(numQubits: Int): QftFile = {
    val file = QftFile(numQubits)
    val sqrt2Inv = 1.0 / Math.sqrt(2.0)
    file(0) = Complex(sqrt2Inv)
    file(file.dimension - 1) = Complex(sqrt2Inv)
    file
  }
  
  /** Uniform superposition state */
  def uniformState(numQubits: Int): QftFile = {
    val dim = 1 << numQubits
    val amp = 1.0 / Math.sqrt(dim.toDouble)
    fromAmplitudes(Seq.fill(dim)(Complex(amp)))
  }
  
  /** Computational basis state |i⟩ */
  def basisState(numQubits: Int, index: Int): QftFile = {
    val file = QftFile(numQubits)
    require(index >= 0 && index < file.dimension, s"Index $index out of range")
    file(0) = Complex.Zero
    file(index) = Complex.One
    file
  }
  
  // =========================================================================
  // Validation
  // =========================================================================
  
  /** Verify if bytes represent a valid QFT file */
  def verify(data: Array[Byte]): Boolean = fromBytes(data) match {
    case Success(file) => file.isNormalized(1e-6)
    case Failure(_) => false
  }
  
  /** Calculate fidelity between two byte arrays */
  def fidelityFromBytes(data1: Array[Byte], data2: Array[Byte]): Try[Double] = for {
    file1 <- fromBytes(data1)
    file2 <- fromBytes(data2)
  } yield file1.fidelity(file2)
}
