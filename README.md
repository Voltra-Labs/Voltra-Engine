<div align="center">

# ⚡ Voltra Engine

**A high-performance 3D graphics engine built from scratch in C++20.**

<p align="center">
  <a href=".github/assets/editor_video.gif"><img src=".github/assets/editor_video.gif" alt="Voltra Engine Editor" width="800"/></a>
</p>

[![Voltra Engine CI](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml/badge.svg)](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Standard](https://img.shields.io/badge/C%2B%2B-20-blue.svg?logo=c%2B%2B)](CMakeLists.txt)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)]()
[![OpenGL](https://img.shields.io/badge/OpenGL-4.6-green.svg)](src/main.cpp)
[![Testing](https://img.shields.io/badge/Testing-Google%20Test-yellow.svg)](tests/)
<a href="https://deepwiki.com/Voltra-Labs/Voltra-Engine"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>

</div>

---

## 🎯 Why Voltra?

There are thousands of game engines, why another one? **Voltra Engine** is not intended to compete commercially with Unreal or Godot. Its mission is to demystify the architecture of modern game engines.

It's designed specifically for **engineering students, C++ developers, and graphics enthusiasts** who want to understand what's "under the hood": 
* **Realistic Architecture:** No "black boxes". All code (ECS, Renderer, Physics) is accessible and follows **C++20** standards.
* **Data-Oriented Design (DOD):** Structured to maximize performance and cache usage, moving away from traditional OOP on critical loops.
* **Test Zone:** The perfect place to experiment with OpenGL 4.6, write your own shaders or implement new physics without the bloatware of giant engines.

## ✨ Key Features

*   **Modern C++ Base:** Written strictly in **C++20** to utilize the latest language features.
*   **Cross-Platform:** Window abstraction layer powered by **GLFW 3.3.8**.
*   **Graphics Context:** Initialized with **OpenGL 4.6 Core Profile** via **GLAD** loader.
*   **Entity Component System (ECS):** Flexible architecture for game objects using Components (Tag, Transform, Sprite, Physics, etc.).
*   **2D Physics:** Integrated **Box2D** physics engine for realistic collisions and dynamics.
*   **Editor Interface:** Professional **ImGui**-based editor with Docking, Scene Hierarchy, and Properties Panel.
*   **Renderer:** Batching 2D Renderer (Quads, Textures, Rotated Quads) and Framebuffer support.
*   **Scene Serialization:** Save and Load scenes using **YAML**.
*   **Event System:** Blocking event system for windowing and input handling.
*   **Logging System:** Professional logging with **spdlog v1.12.0**.
*   **Testing Framework:** Integrated **Google Test v1.14.0**.
*   **Build System:** Modular **CMake** configuration with automatic dependency management.

## 🗺️ Roadmap & Status

| Module | Status | Description |
| :--- | :---: | :--- |
| **Core System** | ✅ | Window creation, Event handling, Main Loop. |
| **Logging** | ✅ | Multi-level logging system with spdlog. |
| **Maths** | ✅ | Integration of GLM. |
| **Testing** | ✅ | Google Test framework implemented. |
| **Renderer** | ✅ | Batch Renderer 2D, Framebuffers, Editor Camera. |
| **ECS** | ✅ | Entity-Component structure fully implemented. |
| **Physics** | ✅ | Box2D integration (Rigidbodies, Colliders). |
| **Editor** | ✅ | ImGui Docking, Scene Hierarchy, Inspector. |
| **Scene Serialization** | ✅ | Save and Load scenes using YAML. |
| **Gizmos** | ✅ | Draw gizmos for debugging and visualization. |
| **Scripting** | 🚧 | *In Progress* - Native C++ Scripting without to recompile the engine. |
| **Asset Management** | 🚧 | *In Progress* - Load and manage assets (Textures, Shaders, Fonts). |

## 🛠️ Requirements

To build Voltra Engine, you need:

*   **C++ Compiler:** MSVC (Visual Studio 2022 recommended), GCC 10+, or Clang 10+.
*   **CMake:** Version 3.15 or higher.
*   **Video Driver:** Support for OpenGL 4.6.
*   **Internet Connection:** Required for first build to fetch dependencies automatically.

> **Note:** All dependencies (GLFW, GLM, spdlog, Google Test, yaml-cpp, Box2D, ImGui) are automatically downloaded and configured by CMake using FetchContent.

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
```

## 📂 Project Structure

```
Voltra-Engine/
├── src/
│   ├── Core/
│   │   ├── Application.hpp/cpp  # Main application class
│   │   └── Log.hpp/cpp          # Logging system (spdlog)
│   ├── Events/
│   │   └── Event.hpp            # Event system (Key, Mouse, App)
│   ├── Renderer/
│   │   ├── Renderer2D.hpp/cpp   # Batch Renderer
│   │   ├── Framebuffer.hpp/cpp  # FBO handling
│   │   └── EditorCamera.hpp/cpp # Camera for Editor view
│   ├── Scene/
│   │   ├── Scene.hpp/cpp        # Scene management & ECS registry
│   │   ├── Entity.hpp/cpp       # Entity wrapper
│   │   └── Components.hpp       # ECS Components
│   ├── ImGui/
│   │   ├── ImGuiLayer.hpp/cpp       # ImGui integration
│   │   └── SceneHierarchyPanel.cpp  # Editor UI panels
│   ├── Sandbox/
│   │   └── EditorLayer.hpp/cpp  # Main Editor application layer
│   └── main.cpp                 # Entry point
├── tests/                       # Google Test unit tests
├── build/                       # Generated build files (git-ignored)
├── CMakeLists.txt               # Build configuration
├── README.md                    # This file
├── CONTRIBUTING.md              # Contribution guidelines
└── LICENSE                      # MIT License
```

> **Note:** Dependencies (Glad, ImGui, Box2D, etc.) are managed in `src/Vendor` or via CMake FetchContent.

## 🔧 Core Systems

### Entity Component System (ECS)
Voltra uses `entt` inspired registry system (internally implemented or wrapped) to manage game objects.
```cpp
auto entity = scene->CreateEntity("Player");
entity.AddComponent<TransformComponent>();
entity.AddComponent<SpriteRendererComponent>(glm::vec4(1.0f, 0.5f, 0.2f, 1.0f));
```

### Logging System
```cpp
VOLTRA_CORE_INFO("Engine Initialized");
VOLTRA_WARN("Player health low: {}", health);
```

### Event System
Blocking event system for handling inputs and window events.
```cpp
void OnEvent(Event& e) {
    EventDispatcher dispatcher(e);
    dispatcher.Dispatch<WindowResizeEvent>(OnWindowResize);
}
```

## 🧪 Testing

The project uses Google Test for unit testing. Tests are located in the `tests/` directory.

## 🤝 Contributing

Contributions are what make the open-source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

Please refer to the [CONTRIBUTING](CONTRIBUTING.md) guide for coding standards and pull request policies.

## 📄 License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.

## 🛡️ Code of Conduct

Please refer to the [CODE_OF_CONDUCT](CODE_OF_CONDUCT.md) for more information.

---

<div align="center">
<sub>Built with ❤️ by Voltra Labs.</sub>
</div>
