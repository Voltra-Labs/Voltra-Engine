<div align="center">

# ⚡ Voltra Engine

**A high-performance 3D graphics engine built from scratch in C++20.**

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Standard](https://img.shields.io/badge/C%2B%2B-20-blue.svg?logo=c%2B%2B)](CMakeLists.txt)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)]()
[![OpenGL](https://img.shields.io/badge/OpenGL-4.6-green.svg)](src/main.cpp)

</div>

---

## 📖 About The Project

**Voltra Engine** is an educational and experimental game engine designed with **Data-Oriented Design (DOD)** principles at its core. The goal is to create a modern architecture that maximizes CPU cache locality and leverages current hardware capabilities, moving away from traditional object-oriented overheads in critical loops.

Currently in the **Initial Core** phase, Voltra provides a robust foundation for windowing and context management, ready for the implementation of a modern rendering pipeline.

## ✨ Key Features

* **Modern C++ Base:** Written strictly in **C++20** to utilize the latest language features.
* **Cross-Platform:** Window abstraction layer powered by **GLFW**.
* **Graphics Context:** initialized with **OpenGL 4.6 Core Profile**.
* **Math Library:** Integrated **GLM** for SIMD-friendly vector mathematics.
* **Build System:** Modular **CMake** configuration for easy dependency management.

## 🗺️ Roadmap & Status

| Module | Status | Description |
| :--- | :---: | :--- |
| **Core System** | ✅ | Window creation, Input polling, Main Loop. |
| **Maths** | ✅ | Integration of GLM (Vectors, Matrices, Quaternions). |
| **Renderer** | 🚧 | *In Progress* - Basic pipeline setup. |
| **ECS** | ⏳ | *Planned* - Entity Component System architecture. |
| **Assets** | ⏳ | *Planned* - Asset manager and loaders. |

## 🛠️ Requirements

To build Voltra Engine, you need:

* **C++ Compiler:** MSVC (Visual Studio 2022 recommended), GCC 10+, or Clang 10+.
* **CMake:** Version 3.15 or higher.
* **Video Driver:** Support for OpenGL 4.6.

## 🚀 Quick Start

### 1. Clone the Repository
```bash
git clone https://github.com/VoltraLabs/VoltraEngine.git
cd VoltraEngine
```

### 2. Build with CMake
```bash
# Generate build files
cmake -S . -B build

# Compile the engine
cmake --build build --config Release
```

### 3. Run

The executable will be located in the `build/` directory (or `build/Release/` on Windows).

## 📂 Project Structure
```
VoltraEngine/
├── external/        # Third-party dependencies (GLFW, GLM)
├── include/         # Header files (.hpp)
├── src/             # Source files (.cpp)
├── CMakeLists.txt   # Build configuration
└── README.md        # Documentation
```

## 🤝 Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. Any contributions you make are greatly appreciated.

Please refer to the [CONTRIBUTING](CONTRIBUTING.md) guide for coding standards and pull request policies.

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.

## Code of Conduct

Please refer to the [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md) for more information.

## 📚 Documentation

See the [WIKI](https://github.com/Voltra-Labs/Voltra-Engine/wiki) for more information.

---

<div align="center">
<sub>Built with ❤️ by Voltra Labs.</sub>
</div>