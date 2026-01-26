"""
Conan recipe for libqft - Quantum File Type C/C++ Library

This recipe enables publishing to Conan Center for C/C++ package management.
"""

from conan import ConanFile
from conan.tools.cmake import CMake, cmake_layout
from conan.tools.files import copy
import os


class LibQftConan(ConanFile):
    name = "libqft"
    version = "1.0.0"
    license = "MIT", "Apache-2.0"
    author = "MACROHARD Quantum OS Team <quantum@macrohard.dev>"
    url = "https://github.com/WeaveITMeta/Quantum-Operating-System"
    description = "Production-grade C/C++ library for Quantum File Type (.qft) format"
    topics = ("quantum", "quantum-computing", "mps", "tensor-network", "qft")
    
    settings = "os", "compiler", "build_type", "arch"
    options = {
        "shared": [True, False],
        "fPIC": [True, False],
        "thread_safe": [True, False],
    }
    default_options = {
        "shared": False,
        "fPIC": True,
        "thread_safe": True,
    }
    
    exports_sources = "src/*", "include/*", "Cargo.toml", "build.rs", "cbindgen.toml"
    
    def config_options(self):
        if self.settings.os == "Windows":
            del self.options.fPIC
    
    def layout(self):
        cmake_layout(self)
    
    def build_requirements(self):
        self.tool_requires("cargo/1.75.0")
    
    def build(self):
        # Build using cargo for the Rust core
        self.run("cargo build --release")
        
        # Copy built library to expected location
        if self.settings.os == "Windows":
            lib_name = "qft.dll"
            static_name = "qft.lib"
        elif self.settings.os == "Macos":
            lib_name = "libqft.dylib"
            static_name = "libqft.a"
        else:
            lib_name = "libqft.so"
            static_name = "libqft.a"
        
        # Libraries are in target/release
        src_dir = os.path.join(self.source_folder, "target", "release")
        
        if self.options.shared:
            copy(self, lib_name, src_dir, os.path.join(self.build_folder, "lib"))
        else:
            copy(self, static_name, src_dir, os.path.join(self.build_folder, "lib"))
    
    def package(self):
        copy(self, "*.h", os.path.join(self.source_folder, "include"),
             os.path.join(self.package_folder, "include"))
        copy(self, "*.a", os.path.join(self.build_folder, "lib"),
             os.path.join(self.package_folder, "lib"))
        copy(self, "*.so*", os.path.join(self.build_folder, "lib"),
             os.path.join(self.package_folder, "lib"))
        copy(self, "*.dylib", os.path.join(self.build_folder, "lib"),
             os.path.join(self.package_folder, "lib"))
        copy(self, "*.dll", os.path.join(self.build_folder, "lib"),
             os.path.join(self.package_folder, "bin"))
        copy(self, "*.lib", os.path.join(self.build_folder, "lib"),
             os.path.join(self.package_folder, "lib"))
        
        # Copy license
        copy(self, "LICENSE*", self.source_folder, 
             os.path.join(self.package_folder, "licenses"))
    
    def package_info(self):
        self.cpp_info.libs = ["qft"]
        
        if self.settings.os in ["Linux", "FreeBSD"]:
            self.cpp_info.system_libs = ["pthread", "dl", "m"]
        
        if self.options.thread_safe:
            self.cpp_info.defines = ["QFT_THREAD_SAFE"]
