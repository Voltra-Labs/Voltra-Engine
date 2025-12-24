<div align="center">

# ⚡ Voltra Engine

**A high-performance 3D graphics engine built from scratch in C++20.**

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Standard](https://img.shields.io/badge/C%2B%2B-20-blue.svg?logo=c%2B%2B)](CMakeLists.txt)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)]()
[![OpenGL](https://img.shields.io/badge/OpenGL-4.6-green.svg)](src/main.cpp)
[![Testing](https://img.shields.io/badge/Testing-Google%20Test-yellow.svg)](tests/)

</div>

---

## 📖 About The Project

**Voltra Engine** is an educational and experimental game engine designed with **Data-Oriented Design (DOD)** principles at its core. The goal is to create a modern architecture that maximizes CPU cache locality and leverages current hardware capabilities, moving away from traditional object-oriented overheads in critical loops.

Currently in the **Initial Core** phase, Voltra provides a robust foundation for windowing, context management, and logging, ready for the implementation of a modern rendering pipeline.

## ✨ Key Features

* **Modern C++ Base:** Written strictly in **C++20** to utilize the latest language features.
* **Cross-Platform:** Window abstraction layer powered by **GLFW 3.3.8**.
* **Graphics Context:** Initialized with **OpenGL 4.6 Core Profile** via **GLAD** loader.
* **Event System:** Blocking event system for windowing and input handling.
* **Renderer Abstraction:** Platform-agnostic buffer system (VertexBuffer, IndexBuffer) with OpenGL implementation.
* **Math Library:** Integrated **GLM 1.0.1** for SIMD-friendly vector mathematics.
* **Logging System:** Professional logging with **spdlog v1.12.0** for both engine and client code.
* **Testing Framework:** Integrated **Google Test v1.14.0** for unit testing.
* **Build System:** Modular **CMake** configuration with automatic dependency management via FetchContent.

## 🗺️ Roadmap & Status

| Module | Status | Description |
| :--- | :---: | :--- |
| **Core System** | ✅ | Window creation, Event handling, Main Loop. |
| **Logging** | ✅ | Multi-level logging system with spdlog. |
| **Maths** | ✅ | Integration of GLM (Vectors, Matrices, Quaternions). |
| **Testing** | ✅ | Google Test framework configured. |
| **Renderer** | 🚧 | *In Progress* - Buffer abstractions (VertexBuffer, IndexBuffer) implemented. |
| **ECS** | ⏳ | *Planned* - Entity Component System architecture. |
| **Assets** | ⏳ | *Planned* - Asset manager and loaders. |

## 🛠️ Requirements

To build Voltra Engine, you need:

* **C++ Compiler:** MSVC (Visual Studio 2022 recommended), GCC 10+, or Clang 10+.
* **CMake:** Version 3.15 or higher.
* **Video Driver:** Support for OpenGL 4.6.
* **Internet Connection:** Required for first build to fetch dependencies automatically.

> **Note:** All dependencies (GLFW, GLM, spdlog, Google Test) are automatically downloaded and configured by CMake using FetchContent. No manual installation required!

## 🚀 Quick Start

### 1. Clone the Repository
```bash
git clone https://github.com/Voltra-Labs/Voltra-Engine.git
cd Voltra-Engine
```

### 2. Build with CMake
```bash
# Generate build files
cmake -S . -B build

# Compile the engine
cmake --build build --config Release
```

### 3. Run the Engine

The executable will be located in the `build/` directory (or `build/Release/` on Windows):

```bash
# On Windows
.\build\Release\VoltraEngine.exe

# On Linux/macOS
./build/VoltraEngine
```

### 4. Run Tests (Optional)

```bash
# Run all tests
cd build
ctest --output-on-failure

# Or run the test executable directly
.\Release\VoltraTests.exe  # Windows
./VoltraTests              # Linux/macOS
```

## 📂 Project Structure

```
Voltra-Engine/
├── src/
│   ├── Core/
│   │   ├── Application.hpp/cpp  # Main application class
│   │   ├── Window.hpp/cpp       # Window abstraction (GLFW)
│   │   └── Log.hpp/cpp          # Logging system (spdlog)
│   ├── Events/
│   │   ├── Event.hpp            # Base event class and dispatcher
│   │   ├── ApplicationEvent.hpp # Window and App events
│   │   ├── KeyEvent.hpp         # Keyboard events
│   │   └── MouseEvent.hpp       # Mouse events
│   ├── Renderer/
│   │   ├── Buffer.hpp/cpp       # VertexBuffer & IndexBuffer abstractions
│   │   └── ...                  # Future renderer components
│   ├── Vendor/
│   │   └── Glad/                # OpenGL loader (GLAD)
│   └── main.cpp                 # Entry point
├── tests/
│   ├── main_test.cpp            # Basic tests
│   ├── EventTest.cpp            # Event system tests
│   └── BufferTest.cpp           # Renderer buffer tests
├── build/                       # Generated build files (git-ignored)
├── CMakeLists.txt               # Build configuration
├── README.md                    # This file
├── CONTRIBUTING.md              # Contribution guidelines
├── CODE_OF_CONDUCT.md           # Code of conduct
└── LICENSE                      # MIT License
```

> **Note:** Dependencies are managed automatically by CMake FetchContent and are downloaded to `build/_deps/` during the first build. GLAD is included as a vendor library in `src/Vendor/Glad/`.

## 🔧 Core Systems

### Logging System

Voltra Engine includes a professional logging system powered by spdlog with color-coded output:

```cpp
// Engine-side logging (use in engine code)
VOLTRA_CORE_TRACE("Detailed trace information");
VOLTRA_CORE_INFO("Informational message");
VOLTRA_CORE_WARN("Warning message");
VOLTRA_CORE_ERROR("Error message");
VOLTRA_CORE_FATAL("Critical error");

// Client-side logging (use in game/app code)
VOLTRA_TRACE("Game trace");
VOLTRA_INFO("Game info");
VOLTRA_WARN("Game warning");
VOLTRA_ERROR("Game error");
VOLTRA_FATAL("Game critical");
```

### Window Management

The `Window` class provides a clean abstraction over GLFW:

```cpp
// Window is automatically created by Application
// Access window properties:
uint32_t width = window->GetWidth();
uint32_t height = window->GetHeight();
bool shouldClose = window->ShouldClose();
```

### Event System

The Event System handles engine-wide events using a blocking dispatcher mechanism:

```cpp
void OnEvent(Event& e) {
    EventDispatcher dispatcher(e);
    
    // Dispatch window resize events
    dispatcher.Dispatch<WindowResizeEvent>([](WindowResizeEvent& e) {
        VOLTRA_CORE_INFO("Window resized to {0}x{1}", e.GetWidth(), e.GetHeight());
        return true; // Event handled
    });
}
```

Categories include:
* **Application:** Window close, resize, tick, update, render.
* **Keyboard:** Key pressed, released, typed.
* **Mouse:** Button pressed, released, moved, scrolled.

## 🧪 Testing

The project uses Google Test for unit testing. Tests are located in the `tests/` directory.

To add new tests, edit `tests/main_test.cpp` or create new test files and link them in `CMakeLists.txt`.

## 🤝 Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

Please refer to the [CONTRIBUTING](CONTRIBUTING.md) guide for coding standards and pull request policies.

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.

## 🛡️ Code of Conduct

Please refer to the [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md) for more information.

## 📚 Documentation

For detailed documentation on architecture, systems, and development guides, visit the [Wiki](https://github.com/Voltra-Labs/Voltra-Engine/wiki).

### Quick Links:
- [🏗️ Architecture & Core Systems](https://github.com/Voltra-Labs/Voltra-Engine/wiki/Architecture)
- [🎨 Renderer System](https://github.com/Voltra-Labs/Voltra-Engine/wiki/Renderer-System)
- [⚡ Event System](https://github.com/Voltra-Labs/Voltra-Engine/wiki/Event-System)
- [🚀 Getting Started](https://github.com/Voltra-Labs/Voltra-Engine/wiki/Getting-Started)
- [🤝 Contribution Guide](https://github.com/Voltra-Labs/Voltra-Engine/wiki/Contribution-Guide)

---

<div align="center">
<sub>Built with ❤️ by Voltra Labs.</sub>
</div>
