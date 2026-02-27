//! V-OS Shell - Slint-based Desktop Environment
//!
//! A modern desktop shell for V-OS featuring:
//! - QDB-backed filesystem browsing
//! - Quantum state (.qft) preview and management
//! - Eustress 3D integration for scene visualization
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         Slint Desktop Shell             │
//! │    (File browser, settings, panels)     │
//! ├─────────────────────────────────────────┤
//! │  Eustress 3D Viewport (IPC connection)  │
//! │    (Scene editing, visualization)       │
//! ├─────────────────────────────────────────┤
//! │         QDB Storage Backend             │
//! └─────────────────────────────────────────┘
//! ```

slint::include_modules!();

use qdb_rs::prelude::*;
use qdb_rs::storage::MemoryBackend;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{info, warn, error};

// =============================================================================
// Application State
// =============================================================================

/// Global application state shared across UI callbacks
pub struct AppState {
    /// QDB database instance
    pub db: QDB<MemoryBackend>,
    /// Current directory path
    pub current_path: String,
    /// Eustress IPC connection (if available)
    pub eustress_connected: bool,
}

impl AppState {
    pub fn new() -> Self {
        let db = QDB::new(MemoryBackend::new());
        
        // Initialize with some demo data
        Self::populate_demo_data(&db);
        
        Self {
            db,
            current_path: "/home/user".to_string(),
            eustress_connected: false,
        }
    }
    
    fn populate_demo_data(db: &QDB<MemoryBackend>) {
        // Add some demo quantum states
        if let Ok(bell) = bell_state() {
            let _ = db.put_quantum("qfs:/home/user/bell_state.qft", bell);
        }
        
        if let Ok(ghz) = ghz_state(5) {
            let _ = db.put_quantum("qfs:/home/user/ghz_state.qft", ghz);
        }
        
        // Add classical files
        let _ = db.put("qfs:/home/user/notes.txt", "Hello V-OS!");
        let _ = db.put("qfs:/home/user/Documents", Value::Json(serde_json::json!({
            "type": "directory",
            "items": 12
        })));
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Starting V-OS Shell...");
    
    // Create application state
    let state = Arc::new(RwLock::new(AppState::new()));
    
    // Create main window
    let main_window = MainWindow::new().expect("Failed to create main window");
    
    // Setup callbacks
    setup_callbacks(&main_window, state.clone());
    
    info!("QDB initialized with demo data");
    info!("Shell ready");
    
    // Run the event loop
    main_window.run().expect("Failed to run main window");
}

// =============================================================================
// Callback Setup
// =============================================================================

fn setup_callbacks(window: &MainWindow, state: Arc<RwLock<AppState>>) {
    // File open callback
    let state_clone = state.clone();
    window.on_open_file(move |file| {
        let state = state_clone.read();
        info!("Opening file: {}", file.path);
        
        if file.is_quantum {
            // Load quantum state from QDB
            let key = format!("qfs:{}", file.path);
            match state.db.get_quantum(key.clone()) {
                Ok(Some(qft)) => {
                    info!("Loaded quantum state: {} qubits", qft.num_qubits());
                }
                Ok(None) => {
                    warn!("Quantum state not found: {}", file.path);
                }
                Err(e) => {
                    error!("Failed to load quantum state: {}", e);
                }
            }
        } else if file.is_directory {
            info!("Navigating to directory: {}", file.path);
        } else {
            info!("Opening classical file: {}", file.path);
        }
    });
    
    // Eustress launch callback
    let state_clone = state.clone();
    window.on_launch_eustress(move |path| {
        let state = state_clone.read();
        info!("Launching Eustress 3D for: {}", path);
        
        if !state.eustress_connected {
            // Try to connect to Eustress via IPC
            info!("Attempting to connect to Eustress...");
            // TODO: Implement IPC connection
        }
        
        // Send file path to Eustress
        info!("Sending path to Eustress: {}", path);
    });
}

// =============================================================================
// Modules
// =============================================================================

mod filesystem {
    //! QDB-backed filesystem operations
    
    use qdb_rs::prelude::*;
    use qdb_rs::storage::MemoryBackend;
    
    /// List files in a directory
    pub fn list_directory(db: &QDB<MemoryBackend>, path: &str) -> Vec<FileInfo> {
        let prefix = format!("qfs:{}/", path.trim_end_matches('/'));
        let mut files = Vec::new();
        
        if let Ok(keys) = db.keys() {
            for key in keys {
                let key_str = key.to_string();
                if key_str.starts_with(&prefix) {
                    let relative = &key_str[prefix.len()..];
                    // Only include direct children (no nested paths)
                    if !relative.contains('/') {
                        if let Ok(Some(entry)) = db.get_entry(key.to_string()) {
                            files.push(FileInfo {
                                name: relative.to_string(),
                                path: key_str.replace("qfs:", ""),
                                is_quantum: matches!(entry.entry_type, EntryType::QuantumState),
                                is_directory: matches!(entry.value, Value::Json(_)),
                                size: entry.size_bytes(),
                            });
                        }
                    }
                }
            }
        }
        
        files
    }
    
    #[derive(Debug, Clone)]
    pub struct FileInfo {
        pub name: String,
        pub path: String,
        pub is_quantum: bool,
        pub is_directory: bool,
        pub size: usize,
    }
}

mod eustress_bridge {
    //! IPC bridge to Eustress 3D engine
    
    use std::path::Path;
    
    /// Message types for Eustress IPC
    #[derive(Debug, Clone)]
    pub enum EustressMessage {
        /// Open a file in Eustress
        OpenFile(String),
        /// Load a quantum state for visualization
        LoadQuantumState { path: String, num_qubits: usize },
        /// Request current scene info
        GetSceneInfo,
        /// Shutdown Eustress
        Shutdown,
    }
    
    /// Eustress IPC client
    pub struct EustressBridge {
        connected: bool,
    }
    
    impl EustressBridge {
        pub fn new() -> Self {
            Self { connected: false }
        }
        
        pub fn connect(&mut self) -> Result<(), String> {
            // TODO: Implement actual IPC connection
            // Using interprocess crate for cross-platform IPC
            self.connected = true;
            Ok(())
        }
        
        pub fn send(&self, msg: EustressMessage) -> Result<(), String> {
            if !self.connected {
                return Err("Not connected to Eustress".to_string());
            }
            
            // TODO: Serialize and send message
            tracing::info!("Sending to Eustress: {:?}", msg);
            Ok(())
        }
        
        pub fn is_connected(&self) -> bool {
            self.connected
        }
    }
    
    impl Default for EustressBridge {
        fn default() -> Self {
            Self::new()
        }
    }
}

mod quantum_preview {
    //! Quantum state preview utilities
    
    use qft::QftFile;
    
    /// Preview data for a quantum state
    #[derive(Debug, Clone)]
    pub struct QuantumPreviewData {
        pub name: String,
        pub num_qubits: usize,
        pub dimension: usize,
        pub norm: f64,
        pub is_normalized: bool,
        pub bond_dimension: usize,
        pub golay_enabled: bool,
    }
    
    impl QuantumPreviewData {
        pub fn from_qft(name: &str, qft: &QftFile) -> Self {
            Self {
                name: name.to_string(),
                num_qubits: qft.num_qubits(),
                dimension: qft.dimension(),
                norm: qft.norm(),
                is_normalized: (qft.norm() - 1.0).abs() < 1e-10,
                bond_dimension: qft.config().bond_dimension,
                golay_enabled: qft.config().golay_enabled,
            }
        }
    }
}
