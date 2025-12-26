# 🤝 Contribution Guide

First off, thank you for considering contributing to **Voltra Engine**! It's people like you that make open source tools great.

To ensure the project remains stable and maintainable, we follow a specific set of rules for development. Please read this document before opening a Pull Request.

---

## 🌿 Git Workflow

We use a simplified **Git Flow** strategy.

### Branches
* **`main`**: The stable version of the engine. **Do not commit directly here.**
* **`develop`**: The integration branch for the next release (optional, depending on project stage).
* **Feature Branches**: Where you work. Create a new branch for every feature or fix.

### Naming Convention
Please use the following prefixes for your branches:
* `feature/` - New capabilities (e.g., `feature/ecs-implementation`)
* `fix/` - Bug fixes (e.g., `fix/memory-leak-texture`)
* `docs/` - Documentation changes (e.g., `docs/update-wiki`)
* `refactor/` - Code restructuring without behavioral changes

---

## 📝 Commit Guidelines

We enforce **Conventional Commits** to keep our history readable and enable automated versioning in the future.

**Format:**
```text
<type>(<scope>): <short description>

[Optional body]
```

**Types:**
* `feat`: A new feature.
* `fix`: A bug fix.
* `docs`: Documentation only changes.
* `style`: Changes that do not affect the meaning of the code (white-space, formatting, etc).
* `refactor`: A code change that neither fixes a bug nor adds a feature.
* `perf`: A code change that improves performance.
* `chore`: Changes to the build process or auxiliary tools.

**Examples:**
* `feat(renderer): add support for basic phong lighting`
* `fix(window): resolve crash when resizing on Linux`
* `chore(cmake): update minimum required version`

---

## 💻 Coding Standards (C++20)

We aim for modern, clean, and performant C++.

1. **Standard**: Use C++20 features where appropriate (Concepts, Modules, etc.).
2. **Memory**: Avoid raw pointers (`new`/`delete`). Use smart pointers (`std::unique_ptr`, `std::shared_ptr`) or stack allocation.
3. **Style**:
   * Variables: `snake_case` (e.g., `window_width`, `is_running`).
   * Functions: `snake_case` (e.g., `init_window()`, `render_frame()`).
   * Types/Classes: `PascalCase` (e.g., `RenderSystem`, `Texture2D`).
   * Private Members: Prefix with `m_` (e.g., `m_width`).
4. **Headers**:
   * Use `#pragma once`.
   * Separate declarations (`.hpp`) from implementations (`.cpp`).


---

## 📝 Logging Guidelines

**CRITICAL:** Do NOT use `std::cout`, `std::cerr`, or `printf` for logging.

Voltra Engine uses a professional logging system (**spdlog**) that provides:
- Color-coded output
- Timestamps and Layouts
- Separation between Engine (Core) and Client (App) logs
- Different log levels (Trace, Info, Warn, Error, Fatal)

### Usage Rules:

1. **Engine Code** (`src/Core`, `src/Renderer`, etc.):
   - ALways use `VOLTRA_CORE_*` macros.
   - Example: `VOLTRA_CORE_INFO("System initialized");`

2. **Client/App Code** (Game logic, Sandbox):
   - Always use `VOLTRA_*` macros.
   - Example: `VOLTRA_ERROR("Falied to load level: {}", levelName);`

3. **Forbidden**:
   - ❌ `std::cout << "Debug" << std::endl;`
   - ❌ `printf("Debug\n");`

Any Process Request containing raw `std::cout` usage will be **rejected**.


## 🚀 Pull Request Process

1. Fork the repository and clone it locally.
2. Create a branch from `main`: `git checkout -b feature/amazing-feature`.
3. Make your changes and ensure the code compiles without warnings.
4. Push to your fork and open a Pull Request to the `main` branch of the official repo.
5. Provide a clear description of your changes and reference any related Issues (e.g., "Closes #12").
6. Wait for code review and address any feedback.

---

## 🧪 Testing

* Ensure the engine builds on your platform.
* If you add a new feature, provide a small example or test case demonstrating it works.

---

**Thank you for your help!**