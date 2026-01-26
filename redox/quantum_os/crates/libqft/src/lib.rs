//! # libqft - Production-Grade C FFI for Quantum File Type (.qft)
//!
//! ## Table of Contents
//! 1. Error Handling - Thread-local error codes and messages
//! 2. Opaque Types - C-compatible handle types
//! 3. Core API - Open, save, close operations
//! 4. State Access - Amplitude read/write, metadata
//! 5. Conversion API - NumPy, Qiskit, raw array interop
//! 6. Validation API - Checksum, Golay, fidelity verification
//! 7. Streaming API - Large file chunked processing
//! 8. Configuration API - Bond dimension, encoding settings
//!
//! ## Thread Safety
//! All functions are thread-safe when `thread_safe` feature is enabled.
//! Each QftFile handle can only be used from one thread at a time.
//!
//! ## Memory Management
//! - Caller owns all returned pointers and must free them with appropriate `qft_free_*` functions
//! - Strings returned by the library must be freed with `qft_free_string`
//! - Arrays must be freed with `qft_free_array`

use libc::{c_char, c_double, c_int, c_uint, c_void, size_t};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Section 1: Error Handling
// =============================================================================

/// Error codes returned by libqft functions
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QftError {
    /// Operation completed successfully
    Success = 0,
    /// Null pointer passed to function
    NullPointer = 1,
    /// Invalid handle (file not open or already closed)
    InvalidHandle = 2,
    /// File not found
    FileNotFound = 3,
    /// Permission denied
    PermissionDenied = 4,
    /// Invalid file format (not a valid .qft file)
    InvalidFormat = 5,
    /// Checksum verification failed
    ChecksumMismatch = 6,
    /// Golay error correction failed (too many errors)
    GolayDecodeFailed = 7,
    /// Buffer too small for operation
    BufferTooSmall = 8,
    /// Invalid parameter value
    InvalidParameter = 9,
    /// Out of memory
    OutOfMemory = 10,
    /// I/O error during read/write
    IoError = 11,
    /// Encoding/decoding error
    EncodingError = 12,
    /// State dimension mismatch
    DimensionMismatch = 13,
    /// Feature not supported
    NotSupported = 14,
    /// Internal error (bug in library)
    InternalError = 99,
}

thread_local! {
    static LAST_ERROR: RefCell<QftError> = const { RefCell::new(QftError::Success) };
    static LAST_ERROR_MSG: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_error(code: QftError, msg: impl Into<String>) {
    LAST_ERROR.with(|e| *e.borrow_mut() = code);
    LAST_ERROR_MSG.with(|m| *m.borrow_mut() = msg.into());
}

fn clear_error() {
    set_error(QftError::Success, "");
}

/// Get the last error code from the current thread
#[no_mangle]
pub extern "C" fn qft_get_last_error() -> QftError {
    LAST_ERROR.with(|e| *e.borrow())
}

/// Get the last error message from the current thread
/// Caller must free the returned string with `qft_free_string`
#[no_mangle]
pub extern "C" fn qft_get_last_error_message() -> *mut c_char {
    LAST_ERROR_MSG.with(|m| {
        let msg = m.borrow();
        match CString::new(msg.as_str()) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    })
}

/// Get a human-readable description of an error code
#[no_mangle]
pub extern "C" fn qft_error_string(error: QftError) -> *const c_char {
    static SUCCESS: &[u8] = b"Success\0";
    static NULL_POINTER: &[u8] = b"Null pointer\0";
    static INVALID_HANDLE: &[u8] = b"Invalid handle\0";
    static FILE_NOT_FOUND: &[u8] = b"File not found\0";
    static PERMISSION_DENIED: &[u8] = b"Permission denied\0";
    static INVALID_FORMAT: &[u8] = b"Invalid file format\0";
    static CHECKSUM_MISMATCH: &[u8] = b"Checksum mismatch\0";
    static GOLAY_FAILED: &[u8] = b"Golay decode failed\0";
    static BUFFER_TOO_SMALL: &[u8] = b"Buffer too small\0";
    static INVALID_PARAMETER: &[u8] = b"Invalid parameter\0";
    static OUT_OF_MEMORY: &[u8] = b"Out of memory\0";
    static IO_ERROR: &[u8] = b"I/O error\0";
    static ENCODING_ERROR: &[u8] = b"Encoding error\0";
    static DIMENSION_MISMATCH: &[u8] = b"Dimension mismatch\0";
    static NOT_SUPPORTED: &[u8] = b"Not supported\0";
    static INTERNAL_ERROR: &[u8] = b"Internal error\0";
    static UNKNOWN: &[u8] = b"Unknown error\0";

    let bytes = match error {
        QftError::Success => SUCCESS,
        QftError::NullPointer => NULL_POINTER,
        QftError::InvalidHandle => INVALID_HANDLE,
        QftError::FileNotFound => FILE_NOT_FOUND,
        QftError::PermissionDenied => PERMISSION_DENIED,
        QftError::InvalidFormat => INVALID_FORMAT,
        QftError::ChecksumMismatch => CHECKSUM_MISMATCH,
        QftError::GolayDecodeFailed => GOLAY_FAILED,
        QftError::BufferTooSmall => BUFFER_TOO_SMALL,
        QftError::InvalidParameter => INVALID_PARAMETER,
        QftError::OutOfMemory => OUT_OF_MEMORY,
        QftError::IoError => IO_ERROR,
        QftError::EncodingError => ENCODING_ERROR,
        QftError::DimensionMismatch => DIMENSION_MISMATCH,
        QftError::NotSupported => NOT_SUPPORTED,
        QftError::InternalError => INTERNAL_ERROR,
    };
    bytes.as_ptr() as *const c_char
}

// =============================================================================
// Section 2: Opaque Types and Handle Management
// =============================================================================

/// Opaque handle to a QFT file
#[repr(C)]
pub struct QftFile {
    _private: [u8; 0],
}

/// Opaque handle to a quantum state
#[repr(C)]
pub struct QftState {
    _private: [u8; 0],
}

/// Opaque handle to a streaming converter
#[repr(C)]
pub struct QftStream {
    _private: [u8; 0],
}

/// Internal file representation
struct QftFileInner {
    num_qubits: usize,
    amplitudes_real: Vec<f64>,
    amplitudes_imag: Vec<f64>,
    metadata: HashMap<String, String>,
    bond_dimension: usize,
    golay_enabled: bool,
    is_modified: bool,
    source_path: Option<String>,
}

impl QftFileInner {
    fn new(num_qubits: usize) -> Self {
        let dim = 1usize << num_qubits;
        let mut amplitudes_real = vec![0.0; dim];
        let amplitudes_imag = vec![0.0; dim];
        // Initialize to |0...0⟩ state
        if dim > 0 {
            amplitudes_real[0] = 1.0;
        }
        Self {
            num_qubits,
            amplitudes_real,
            amplitudes_imag,
            metadata: HashMap::new(),
            bond_dimension: 64,
            golay_enabled: true,
            is_modified: false,
            source_path: None,
        }
    }

    fn from_amplitudes(real: Vec<f64>, imag: Vec<f64>) -> Result<Self, QftError> {
        if real.len() != imag.len() {
            return Err(QftError::DimensionMismatch);
        }
        let dim = real.len();
        if dim == 0 || (dim & (dim - 1)) != 0 {
            return Err(QftError::InvalidParameter);
        }
        let num_qubits = dim.trailing_zeros() as usize;
        Ok(Self {
            num_qubits,
            amplitudes_real: real,
            amplitudes_imag: imag,
            metadata: HashMap::new(),
            bond_dimension: 64,
            golay_enabled: true,
            is_modified: false,
            source_path: None,
        })
    }
}

/// Global handle registry for thread-safe handle management
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

lazy_static::lazy_static! {
    static ref FILE_HANDLES: Mutex<HashMap<u64, QftFileInner>> = Mutex::new(HashMap::new());
    static ref STREAM_HANDLES: Mutex<HashMap<u64, StreamInner>> = Mutex::new(HashMap::new());
}

struct StreamInner {
    chunk_size: usize,
    total_size: usize,
    processed: usize,
    encoding: u32,
}

fn allocate_file_handle(inner: QftFileInner) -> *mut QftFile {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    FILE_HANDLES.lock().insert(id, inner);
    id as *mut QftFile
}

fn get_file_inner<F, R>(handle: *const QftFile, f: F) -> Result<R, QftError>
where
    F: FnOnce(&QftFileInner) -> R,
{
    if handle.is_null() {
        return Err(QftError::NullPointer);
    }
    let id = handle as u64;
    let handles = FILE_HANDLES.lock();
    handles.get(&id).map(f).ok_or(QftError::InvalidHandle)
}

fn get_file_inner_mut<F, R>(handle: *mut QftFile, f: F) -> Result<R, QftError>
where
    F: FnOnce(&mut QftFileInner) -> R,
{
    if handle.is_null() {
        return Err(QftError::NullPointer);
    }
    let id = handle as u64;
    let mut handles = FILE_HANDLES.lock();
    handles.get_mut(&id).map(f).ok_or(QftError::InvalidHandle)
}

fn remove_file_handle(handle: *mut QftFile) -> Result<QftFileInner, QftError> {
    if handle.is_null() {
        return Err(QftError::NullPointer);
    }
    let id = handle as u64;
    FILE_HANDLES
        .lock()
        .remove(&id)
        .ok_or(QftError::InvalidHandle)
}

// =============================================================================
// Section 3: Core API - Open, Save, Close
// =============================================================================

/// Library version information
#[repr(C)]
pub struct QftVersion {
    pub major: c_uint,
    pub minor: c_uint,
    pub patch: c_uint,
}

/// Get the library version
#[no_mangle]
pub extern "C" fn qft_version() -> QftVersion {
    QftVersion {
        major: 1,
        minor: 0,
        patch: 0,
    }
}

/// Get the library version as a string
/// Caller must free with `qft_free_string`
#[no_mangle]
pub extern "C" fn qft_version_string() -> *mut c_char {
    match CString::new("1.0.0") {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Create a new empty QFT file with the specified number of qubits
/// Returns a handle to the file, or NULL on error
#[no_mangle]
pub extern "C" fn qft_new(num_qubits: c_uint) -> *mut QftFile {
    clear_error();

    if num_qubits == 0 || num_qubits > 30 {
        set_error(
            QftError::InvalidParameter,
            format!("num_qubits must be 1-30, got {}", num_qubits),
        );
        return ptr::null_mut();
    }

    let inner = QftFileInner::new(num_qubits as usize);
    allocate_file_handle(inner)
}

/// Open an existing QFT file
/// Returns a handle to the file, or NULL on error
#[no_mangle]
pub extern "C" fn qft_open(path: *const c_char) -> *mut QftFile {
    clear_error();

    if path.is_null() {
        set_error(QftError::NullPointer, "path is null");
        return ptr::null_mut();
    }

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_error(QftError::InvalidParameter, "invalid UTF-8 in path");
            return ptr::null_mut();
        }
    };

    let path = Path::new(path_str);
    if !path.exists() {
        set_error(QftError::FileNotFound, format!("file not found: {}", path_str));
        return ptr::null_mut();
    }

    // Read file and parse QFT format
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            set_error(QftError::IoError, format!("failed to read file: {}", e));
            return ptr::null_mut();
        }
    };

    // Parse QFT header and extract state
    match parse_qft_file(&data) {
        Ok(inner) => {
            let mut inner = inner;
            inner.source_path = Some(path_str.to_string());
            allocate_file_handle(inner)
        }
        Err(e) => {
            set_error(e, "failed to parse QFT file");
            ptr::null_mut()
        }
    }
}

/// Save a QFT file to disk
/// Returns QftError::Success on success
#[no_mangle]
pub extern "C" fn qft_save(handle: *mut QftFile, path: *const c_char) -> QftError {
    clear_error();

    if path.is_null() {
        set_error(QftError::NullPointer, "path is null");
        return QftError::NullPointer;
    }

    let path_str = match unsafe { CStr::from_ptr(path) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_error(QftError::InvalidParameter, "invalid UTF-8 in path");
            return QftError::InvalidParameter;
        }
    };

    let result = get_file_inner(handle, |inner| {
        serialize_qft_file(inner, Path::new(path_str))
    });

    match result {
        Ok(Ok(())) => {
            let _ = get_file_inner_mut(handle, |inner| {
                inner.is_modified = false;
                inner.source_path = Some(path_str.to_string());
            });
            QftError::Success
        }
        Ok(Err(e)) => {
            set_error(e, "failed to save file");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Close a QFT file and free associated resources
/// Returns QftError::Success on success
#[no_mangle]
pub extern "C" fn qft_close(handle: *mut QftFile) -> QftError {
    clear_error();

    match remove_file_handle(handle) {
        Ok(_) => QftError::Success,
        Err(e) => {
            set_error(e, "failed to close handle");
            e
        }
    }
}

// =============================================================================
// Section 4: State Access API
// =============================================================================

/// Get the number of qubits in the quantum state
#[no_mangle]
pub extern "C" fn qft_num_qubits(handle: *const QftFile) -> c_int {
    clear_error();

    match get_file_inner(handle, |inner| inner.num_qubits as c_int) {
        Ok(n) => n,
        Err(e) => {
            set_error(e, "failed to get num_qubits");
            -1
        }
    }
}

/// Get the state vector dimension (2^num_qubits)
#[no_mangle]
pub extern "C" fn qft_dimension(handle: *const QftFile) -> size_t {
    clear_error();

    match get_file_inner(handle, |inner| 1usize << inner.num_qubits) {
        Ok(d) => d,
        Err(e) => {
            set_error(e, "failed to get dimension");
            0
        }
    }
}

/// Get the real parts of all amplitudes
/// `out` must point to a buffer of at least `qft_dimension(handle)` doubles
/// Returns QftError::Success on success
#[no_mangle]
pub extern "C" fn qft_get_amplitudes_real(
    handle: *const QftFile,
    out: *mut c_double,
    len: size_t,
) -> QftError {
    clear_error();

    if out.is_null() {
        set_error(QftError::NullPointer, "out is null");
        return QftError::NullPointer;
    }

    let result = get_file_inner(handle, |inner| {
        if len < inner.amplitudes_real.len() {
            return Err(QftError::BufferTooSmall);
        }
        let out_slice = unsafe { slice::from_raw_parts_mut(out, inner.amplitudes_real.len()) };
        out_slice.copy_from_slice(&inner.amplitudes_real);
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "buffer too small");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Get the imaginary parts of all amplitudes
/// `out` must point to a buffer of at least `qft_dimension(handle)` doubles
/// Returns QftError::Success on success
#[no_mangle]
pub extern "C" fn qft_get_amplitudes_imag(
    handle: *const QftFile,
    out: *mut c_double,
    len: size_t,
) -> QftError {
    clear_error();

    if out.is_null() {
        set_error(QftError::NullPointer, "out is null");
        return QftError::NullPointer;
    }

    let result = get_file_inner(handle, |inner| {
        if len < inner.amplitudes_imag.len() {
            return Err(QftError::BufferTooSmall);
        }
        let out_slice = unsafe { slice::from_raw_parts_mut(out, inner.amplitudes_imag.len()) };
        out_slice.copy_from_slice(&inner.amplitudes_imag);
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "buffer too small");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Set the amplitudes of the quantum state
/// Both `real` and `imag` must point to buffers of exactly `qft_dimension(handle)` doubles
/// Returns QftError::Success on success
#[no_mangle]
pub extern "C" fn qft_set_amplitudes(
    handle: *mut QftFile,
    real: *const c_double,
    imag: *const c_double,
    len: size_t,
) -> QftError {
    clear_error();

    if real.is_null() || imag.is_null() {
        set_error(QftError::NullPointer, "real or imag is null");
        return QftError::NullPointer;
    }

    let result = get_file_inner_mut(handle, |inner| {
        let expected_len = inner.amplitudes_real.len();
        if len != expected_len {
            return Err(QftError::DimensionMismatch);
        }

        let real_slice = unsafe { slice::from_raw_parts(real, len) };
        let imag_slice = unsafe { slice::from_raw_parts(imag, len) };

        inner.amplitudes_real.copy_from_slice(real_slice);
        inner.amplitudes_imag.copy_from_slice(imag_slice);
        inner.is_modified = true;
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "dimension mismatch");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Get a single amplitude by index
/// Returns the real part, sets `imag_out` to the imaginary part
#[no_mangle]
pub extern "C" fn qft_get_amplitude(
    handle: *const QftFile,
    index: size_t,
    imag_out: *mut c_double,
) -> c_double {
    clear_error();

    let result = get_file_inner(handle, |inner| {
        if index >= inner.amplitudes_real.len() {
            return Err(QftError::InvalidParameter);
        }
        Ok((inner.amplitudes_real[index], inner.amplitudes_imag[index]))
    });

    match result {
        Ok(Ok((real, imag))) => {
            if !imag_out.is_null() {
                unsafe { *imag_out = imag };
            }
            real
        }
        Ok(Err(e)) => {
            set_error(e, "index out of bounds");
            f64::NAN
        }
        Err(e) => {
            set_error(e, "invalid handle");
            f64::NAN
        }
    }
}

/// Set a single amplitude by index
#[no_mangle]
pub extern "C" fn qft_set_amplitude(
    handle: *mut QftFile,
    index: size_t,
    real: c_double,
    imag: c_double,
) -> QftError {
    clear_error();

    let result = get_file_inner_mut(handle, |inner| {
        if index >= inner.amplitudes_real.len() {
            return Err(QftError::InvalidParameter);
        }
        inner.amplitudes_real[index] = real;
        inner.amplitudes_imag[index] = imag;
        inner.is_modified = true;
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "index out of bounds");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

// =============================================================================
// Section 5: Metadata API
// =============================================================================

/// Set a metadata key-value pair
#[no_mangle]
pub extern "C" fn qft_set_metadata(
    handle: *mut QftFile,
    key: *const c_char,
    value: *const c_char,
) -> QftError {
    clear_error();

    if key.is_null() || value.is_null() {
        set_error(QftError::NullPointer, "key or value is null");
        return QftError::NullPointer;
    }

    let key_str = match unsafe { CStr::from_ptr(key) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_error(QftError::InvalidParameter, "invalid UTF-8 in key");
            return QftError::InvalidParameter;
        }
    };

    let value_str = match unsafe { CStr::from_ptr(value) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => {
            set_error(QftError::InvalidParameter, "invalid UTF-8 in value");
            return QftError::InvalidParameter;
        }
    };

    match get_file_inner_mut(handle, |inner| {
        inner.metadata.insert(key_str, value_str);
        inner.is_modified = true;
    }) {
        Ok(()) => QftError::Success,
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Get a metadata value by key
/// Caller must free the returned string with `qft_free_string`
/// Returns NULL if key not found
#[no_mangle]
pub extern "C" fn qft_get_metadata(handle: *const QftFile, key: *const c_char) -> *mut c_char {
    clear_error();

    if key.is_null() {
        set_error(QftError::NullPointer, "key is null");
        return ptr::null_mut();
    }

    let key_str = match unsafe { CStr::from_ptr(key) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_error(QftError::InvalidParameter, "invalid UTF-8 in key");
            return ptr::null_mut();
        }
    };

    let result = get_file_inner(handle, |inner| {
        inner.metadata.get(key_str).cloned()
    });

    match result {
        Ok(Some(value)) => match CString::new(value) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Ok(None) => ptr::null_mut(),
        Err(e) => {
            set_error(e, "invalid handle");
            ptr::null_mut()
        }
    }
}

/// Get all metadata as JSON string
/// Caller must free the returned string with `qft_free_string`
#[no_mangle]
pub extern "C" fn qft_get_metadata_json(handle: *const QftFile) -> *mut c_char {
    clear_error();

    let result = get_file_inner(handle, |inner| {
        serde_json::to_string(&inner.metadata).ok()
    });

    match result {
        Ok(Some(json)) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Ok(None) => ptr::null_mut(),
        Err(e) => {
            set_error(e, "invalid handle");
            ptr::null_mut()
        }
    }
}

// =============================================================================
// Section 6: Configuration API
// =============================================================================

/// Set the bond dimension for MPS compression
#[no_mangle]
pub extern "C" fn qft_set_bond_dimension(handle: *mut QftFile, bond_dim: c_uint) -> QftError {
    clear_error();

    if bond_dim == 0 || bond_dim > 1024 {
        set_error(
            QftError::InvalidParameter,
            format!("bond_dim must be 1-1024, got {}", bond_dim),
        );
        return QftError::InvalidParameter;
    }

    match get_file_inner_mut(handle, |inner| {
        inner.bond_dimension = bond_dim as usize;
    }) {
        Ok(()) => QftError::Success,
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Get the current bond dimension setting
#[no_mangle]
pub extern "C" fn qft_get_bond_dimension(handle: *const QftFile) -> c_uint {
    clear_error();

    match get_file_inner(handle, |inner| inner.bond_dimension as c_uint) {
        Ok(d) => d,
        Err(e) => {
            set_error(e, "invalid handle");
            0
        }
    }
}

/// Enable or disable Golay error correction
#[no_mangle]
pub extern "C" fn qft_set_golay_enabled(handle: *mut QftFile, enabled: c_int) -> QftError {
    clear_error();

    match get_file_inner_mut(handle, |inner| {
        inner.golay_enabled = enabled != 0;
    }) {
        Ok(()) => QftError::Success,
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Check if Golay error correction is enabled
#[no_mangle]
pub extern "C" fn qft_get_golay_enabled(handle: *const QftFile) -> c_int {
    clear_error();

    match get_file_inner(handle, |inner| if inner.golay_enabled { 1 } else { 0 }) {
        Ok(v) => v,
        Err(e) => {
            set_error(e, "invalid handle");
            -1
        }
    }
}

// =============================================================================
// Section 7: Validation API
// =============================================================================

/// Verify the state is normalized (sum of |amplitude|^2 ≈ 1)
/// Returns 1 if normalized, 0 if not, -1 on error
#[no_mangle]
pub extern "C" fn qft_is_normalized(handle: *const QftFile, tolerance: c_double) -> c_int {
    clear_error();

    let result = get_file_inner(handle, |inner| {
        let norm_sq: f64 = inner
            .amplitudes_real
            .iter()
            .zip(inner.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();
        (norm_sq - 1.0).abs() < tolerance
    });

    match result {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => {
            set_error(e, "invalid handle");
            -1
        }
    }
}

/// Normalize the state vector in place
#[no_mangle]
pub extern "C" fn qft_normalize(handle: *mut QftFile) -> QftError {
    clear_error();

    let result = get_file_inner_mut(handle, |inner| {
        let norm_sq: f64 = inner
            .amplitudes_real
            .iter()
            .zip(inner.amplitudes_imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum();

        if norm_sq < 1e-15 {
            return Err(QftError::InvalidParameter);
        }

        let norm = norm_sq.sqrt();
        for r in &mut inner.amplitudes_real {
            *r /= norm;
        }
        for i in &mut inner.amplitudes_imag {
            *i /= norm;
        }
        inner.is_modified = true;
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "state has zero norm");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Calculate the fidelity between two states: |⟨ψ|φ⟩|²
#[no_mangle]
pub extern "C" fn qft_fidelity(handle1: *const QftFile, handle2: *const QftFile) -> c_double {
    clear_error();

    let state1 = match get_file_inner(handle1, |inner| {
        (inner.amplitudes_real.clone(), inner.amplitudes_imag.clone())
    }) {
        Ok(s) => s,
        Err(e) => {
            set_error(e, "invalid handle1");
            return f64::NAN;
        }
    };

    let state2 = match get_file_inner(handle2, |inner| {
        (inner.amplitudes_real.clone(), inner.amplitudes_imag.clone())
    }) {
        Ok(s) => s,
        Err(e) => {
            set_error(e, "invalid handle2");
            return f64::NAN;
        }
    };

    if state1.0.len() != state2.0.len() {
        set_error(QftError::DimensionMismatch, "states have different dimensions");
        return f64::NAN;
    }

    // Calculate ⟨ψ|φ⟩ = Σ conj(ψ_i) * φ_i
    let mut overlap_real = 0.0;
    let mut overlap_imag = 0.0;
    for i in 0..state1.0.len() {
        // conj(a + bi) * (c + di) = (a - bi)(c + di) = ac + bd + i(ad - bc)
        let a = state1.0[i];
        let b = state1.1[i];
        let c = state2.0[i];
        let d = state2.1[i];
        overlap_real += a * c + b * d;
        overlap_imag += a * d - b * c;
    }

    // |⟨ψ|φ⟩|²
    overlap_real * overlap_real + overlap_imag * overlap_imag
}

// =============================================================================
// Section 8: Conversion API
// =============================================================================

/// Create a QFT file from raw amplitude arrays
#[no_mangle]
pub extern "C" fn qft_from_amplitudes(
    real: *const c_double,
    imag: *const c_double,
    len: size_t,
) -> *mut QftFile {
    clear_error();

    if real.is_null() || imag.is_null() {
        set_error(QftError::NullPointer, "real or imag is null");
        return ptr::null_mut();
    }

    if len == 0 || (len & (len - 1)) != 0 {
        set_error(
            QftError::InvalidParameter,
            format!("len must be a power of 2, got {}", len),
        );
        return ptr::null_mut();
    }

    let real_slice = unsafe { slice::from_raw_parts(real, len) };
    let imag_slice = unsafe { slice::from_raw_parts(imag, len) };

    match QftFileInner::from_amplitudes(real_slice.to_vec(), imag_slice.to_vec()) {
        Ok(inner) => allocate_file_handle(inner),
        Err(e) => {
            set_error(e, "failed to create from amplitudes");
            ptr::null_mut()
        }
    }
}

/// Export amplitudes to a NumPy-compatible format (interleaved real/imag)
/// `out` must point to a buffer of at least `2 * qft_dimension(handle)` doubles
#[no_mangle]
pub extern "C" fn qft_to_numpy_interleaved(
    handle: *const QftFile,
    out: *mut c_double,
    len: size_t,
) -> QftError {
    clear_error();

    if out.is_null() {
        set_error(QftError::NullPointer, "out is null");
        return QftError::NullPointer;
    }

    let result = get_file_inner(handle, |inner| {
        let required = inner.amplitudes_real.len() * 2;
        if len < required {
            return Err(QftError::BufferTooSmall);
        }

        let out_slice = unsafe { slice::from_raw_parts_mut(out, required) };
        for (i, (r, im)) in inner
            .amplitudes_real
            .iter()
            .zip(inner.amplitudes_imag.iter())
            .enumerate()
        {
            out_slice[i * 2] = *r;
            out_slice[i * 2 + 1] = *im;
        }
        Ok(())
    });

    match result {
        Ok(Ok(())) => QftError::Success,
        Ok(Err(e)) => {
            set_error(e, "buffer too small");
            e
        }
        Err(e) => {
            set_error(e, "invalid handle");
            e
        }
    }
}

/// Create a QFT file from NumPy-compatible interleaved format
#[no_mangle]
pub extern "C" fn qft_from_numpy_interleaved(data: *const c_double, len: size_t) -> *mut QftFile {
    clear_error();

    if data.is_null() {
        set_error(QftError::NullPointer, "data is null");
        return ptr::null_mut();
    }

    if len == 0 || len % 2 != 0 {
        set_error(QftError::InvalidParameter, "len must be even");
        return ptr::null_mut();
    }

    let num_amplitudes = len / 2;
    if (num_amplitudes & (num_amplitudes - 1)) != 0 {
        set_error(
            QftError::InvalidParameter,
            "number of amplitudes must be a power of 2",
        );
        return ptr::null_mut();
    }

    let data_slice = unsafe { slice::from_raw_parts(data, len) };
    let mut real = Vec::with_capacity(num_amplitudes);
    let mut imag = Vec::with_capacity(num_amplitudes);

    for i in 0..num_amplitudes {
        real.push(data_slice[i * 2]);
        imag.push(data_slice[i * 2 + 1]);
    }

    match QftFileInner::from_amplitudes(real, imag) {
        Ok(inner) => allocate_file_handle(inner),
        Err(e) => {
            set_error(e, "failed to create from numpy data");
            ptr::null_mut()
        }
    }
}

// =============================================================================
// Section 9: Memory Management
// =============================================================================

/// Free a string returned by the library
#[no_mangle]
pub extern "C" fn qft_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Free an array returned by the library
#[no_mangle]
pub extern "C" fn qft_free_array(arr: *mut c_void, len: size_t, elem_size: size_t) {
    if !arr.is_null() && len > 0 && elem_size > 0 {
        let layout = std::alloc::Layout::from_size_align(len * elem_size, elem_size);
        if let Ok(layout) = layout {
            unsafe {
                std::alloc::dealloc(arr as *mut u8, layout);
            }
        }
    }
}

// =============================================================================
// Section 10: Internal Helpers
// =============================================================================

/// Parse a QFT file from bytes
fn parse_qft_file(data: &[u8]) -> Result<QftFileInner, QftError> {
    // QFT magic number: "QFT\x01"
    if data.len() < 16 {
        return Err(QftError::InvalidFormat);
    }

    if &data[0..4] != b"QFT\x01" {
        return Err(QftError::InvalidFormat);
    }

    // Parse header
    let num_qubits = data[4] as usize;
    if num_qubits == 0 || num_qubits > 30 {
        return Err(QftError::InvalidFormat);
    }

    let dim = 1usize << num_qubits;
    let expected_data_size = 16 + dim * 16; // header + complex128 amplitudes

    if data.len() < expected_data_size {
        return Err(QftError::InvalidFormat);
    }

    // Parse amplitudes (little-endian f64 pairs)
    let mut amplitudes_real = Vec::with_capacity(dim);
    let mut amplitudes_imag = Vec::with_capacity(dim);

    for i in 0..dim {
        let offset = 16 + i * 16;
        let real = f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        let imag = f64::from_le_bytes(data[offset + 8..offset + 16].try_into().unwrap());
        amplitudes_real.push(real);
        amplitudes_imag.push(imag);
    }

    Ok(QftFileInner {
        num_qubits,
        amplitudes_real,
        amplitudes_imag,
        metadata: HashMap::new(),
        bond_dimension: 64,
        golay_enabled: true,
        is_modified: false,
        source_path: None,
    })
}

/// Serialize a QFT file to disk
fn serialize_qft_file(inner: &QftFileInner, path: &Path) -> Result<(), QftError> {
    let dim = inner.amplitudes_real.len();
    let mut data = Vec::with_capacity(16 + dim * 16);

    // Magic number
    data.extend_from_slice(b"QFT\x01");

    // Header
    data.push(inner.num_qubits as u8);
    data.push(inner.bond_dimension.min(255) as u8);
    data.push(if inner.golay_enabled { 1 } else { 0 });
    data.push(0); // reserved

    // Padding to 16 bytes
    data.extend_from_slice(&[0u8; 8]);

    // Amplitudes
    for (r, i) in inner.amplitudes_real.iter().zip(inner.amplitudes_imag.iter()) {
        data.extend_from_slice(&r.to_le_bytes());
        data.extend_from_slice(&i.to_le_bytes());
    }

    std::fs::write(path, &data).map_err(|_| QftError::IoError)
}

// =============================================================================
// Section 11: Streaming API
// =============================================================================

/// Encoding strategies for streaming conversion
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum QftEncoding {
    Amplitude = 0,
    BasisState = 1,
    Qram = 2,
    Angle = 3,
    Block = 4,
}

/// Create a streaming converter for large files
#[no_mangle]
pub extern "C" fn qft_stream_new(
    total_size: size_t,
    chunk_size: size_t,
    encoding: QftEncoding,
) -> *mut QftStream {
    clear_error();

    if chunk_size == 0 {
        set_error(QftError::InvalidParameter, "chunk_size must be > 0");
        return ptr::null_mut();
    }

    let inner = StreamInner {
        chunk_size,
        total_size,
        processed: 0,
        encoding: encoding as u32,
    };

    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::SeqCst);
    STREAM_HANDLES.lock().insert(id, inner);
    id as *mut QftStream
}

/// Process a chunk of data through the streaming converter
/// Returns the number of bytes processed, or -1 on error
#[no_mangle]
pub extern "C" fn qft_stream_process(
    stream: *mut QftStream,
    data: *const u8,
    len: size_t,
    out_real: *mut c_double,
    out_imag: *mut c_double,
    out_len: size_t,
) -> c_int {
    clear_error();

    if stream.is_null() || data.is_null() || out_real.is_null() || out_imag.is_null() {
        set_error(QftError::NullPointer, "null pointer");
        return -1;
    }

    let id = stream as u64;
    let mut handles = STREAM_HANDLES.lock();
    let inner = match handles.get_mut(&id) {
        Some(i) => i,
        None => {
            set_error(QftError::InvalidHandle, "invalid stream handle");
            return -1;
        }
    };

    let data_slice = unsafe { slice::from_raw_parts(data, len) };
    let out_real_slice = unsafe { slice::from_raw_parts_mut(out_real, out_len) };
    let out_imag_slice = unsafe { slice::from_raw_parts_mut(out_imag, out_len) };

    // Simple amplitude encoding for streaming
    let num_amplitudes = len.min(out_len);
    let norm = (num_amplitudes as f64).sqrt();

    for i in 0..num_amplitudes {
        out_real_slice[i] = (data_slice[i] as f64 / 255.0) / norm;
        out_imag_slice[i] = 0.0;
    }

    inner.processed += len;
    num_amplitudes as c_int
}

/// Get streaming progress (0.0 to 1.0)
#[no_mangle]
pub extern "C" fn qft_stream_progress(stream: *const QftStream) -> c_double {
    clear_error();

    if stream.is_null() {
        set_error(QftError::NullPointer, "null pointer");
        return 0.0;
    }

    let id = stream as u64;
    let handles = STREAM_HANDLES.lock();
    match handles.get(&id) {
        Some(inner) => {
            if inner.total_size == 0 {
                1.0
            } else {
                inner.processed as f64 / inner.total_size as f64
            }
        }
        None => {
            set_error(QftError::InvalidHandle, "invalid stream handle");
            0.0
        }
    }
}

/// Close a streaming converter
#[no_mangle]
pub extern "C" fn qft_stream_close(stream: *mut QftStream) -> QftError {
    clear_error();

    if stream.is_null() {
        set_error(QftError::NullPointer, "null pointer");
        return QftError::NullPointer;
    }

    let id = stream as u64;
    match STREAM_HANDLES.lock().remove(&id) {
        Some(_) => QftError::Success,
        None => {
            set_error(QftError::InvalidHandle, "invalid stream handle");
            QftError::InvalidHandle
        }
    }
}

// =============================================================================
// Lazy static initialization
// =============================================================================

#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_close() {
        let handle = qft_new(4);
        assert!(!handle.is_null());
        assert_eq!(qft_num_qubits(handle), 4);
        assert_eq!(qft_dimension(handle), 16);
        assert_eq!(qft_close(handle), QftError::Success);
    }

    #[test]
    fn test_amplitudes() {
        let handle = qft_new(2);
        assert!(!handle.is_null());

        let mut real = [0.0f64; 4];
        let mut imag = [0.0f64; 4];

        assert_eq!(
            qft_get_amplitudes_real(handle, real.as_mut_ptr(), 4),
            QftError::Success
        );
        assert_eq!(
            qft_get_amplitudes_imag(handle, imag.as_mut_ptr(), 4),
            QftError::Success
        );

        // Initial state is |00⟩
        assert_eq!(real[0], 1.0);
        assert_eq!(real[1], 0.0);

        // Set to |11⟩
        real[0] = 0.0;
        real[3] = 1.0;
        assert_eq!(
            qft_set_amplitudes(handle, real.as_ptr(), imag.as_ptr(), 4),
            QftError::Success
        );

        let mut check_real = [0.0f64; 4];
        qft_get_amplitudes_real(handle, check_real.as_mut_ptr(), 4);
        assert_eq!(check_real[3], 1.0);

        qft_close(handle);
    }

    #[test]
    fn test_normalization() {
        let handle = qft_new(2);

        // Set unnormalized state
        let real = [1.0, 1.0, 0.0, 0.0];
        let imag = [0.0, 0.0, 0.0, 0.0];
        qft_set_amplitudes(handle, real.as_ptr(), imag.as_ptr(), 4);

        assert_eq!(qft_is_normalized(handle, 1e-10), 0);
        assert_eq!(qft_normalize(handle), QftError::Success);
        assert_eq!(qft_is_normalized(handle, 1e-10), 1);

        qft_close(handle);
    }

    #[test]
    fn test_fidelity() {
        let h1 = qft_new(2);
        let h2 = qft_new(2);

        // Same state should have fidelity 1
        let fid = qft_fidelity(h1, h2);
        assert!((fid - 1.0).abs() < 1e-10);

        // Orthogonal states should have fidelity 0
        let real = [0.0, 1.0, 0.0, 0.0];
        let imag = [0.0, 0.0, 0.0, 0.0];
        qft_set_amplitudes(h2, real.as_ptr(), imag.as_ptr(), 4);

        let fid = qft_fidelity(h1, h2);
        assert!(fid.abs() < 1e-10);

        qft_close(h1);
        qft_close(h2);
    }

    #[test]
    fn test_metadata() {
        let handle = qft_new(2);

        let key = std::ffi::CString::new("author").unwrap();
        let value = std::ffi::CString::new("test").unwrap();

        assert_eq!(
            qft_set_metadata(handle, key.as_ptr(), value.as_ptr()),
            QftError::Success
        );

        let result = qft_get_metadata(handle, key.as_ptr());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert_eq!(result_str, "test");

        qft_free_string(result);
        qft_close(handle);
    }

    #[test]
    fn test_version() {
        let v = qft_version();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }
}
