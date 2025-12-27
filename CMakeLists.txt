cmake_minimum_required(VERSION 3.15)
project(VoltraEngine VERSION 0.0.1 LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Module to automatically download libraries
include(FetchContent)

# --- Dependency: stb (Image Loading) ---
message(STATUS "Fetching stb...")
FetchContent_Declare(
    stb
    GIT_REPOSITORY https://github.com/nothings/stb.git
    GIT_TAG        master
)
FetchContent_MakeAvailable(stb)

# --- Dependency: GLFW ---
message(STATUS "Fetching GLFW...")
FetchContent_Declare(
    glfw
    GIT_REPOSITORY https://github.com/glfw/glfw.git
    GIT_TAG        3.3.8
)
set(GLFW_BUILD_DOCS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(GLFW_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(glfw)

# --- Dependency: GLM ---
message(STATUS "Fetching GLM...")
FetchContent_Declare(
    glm
    GIT_REPOSITORY https://github.com/g-truc/glm.git
    GIT_TAG        1.0.1
)
FetchContent_MakeAvailable(glm)

# --- Dependency: spdlog (Logging) ---
message(STATUS "Fetching spdlog...")
FetchContent_Declare(
    spdlog
    GIT_REPOSITORY https://github.com/gabime/spdlog.git
    GIT_TAG        v1.12.0
)
FetchContent_MakeAvailable(spdlog)

# --- Dependency: EnTT (ECS) ---
message(STATUS "Fetching EnTT...")
FetchContent_Declare(
    entt
    GIT_REPOSITORY https://github.com/skypjack/entt.git
    GIT_TAG        v3.13.0
)
FetchContent_MakeAvailable(entt)

# --- Dependency: ImGui ---
message(STATUS "Fetching ImGui...")
FetchContent_Declare(
    imgui
    GIT_REPOSITORY https://github.com/ocornut/imgui.git
    GIT_TAG        docking
)
FetchContent_MakeAvailable(imgui)

# --- Dependency: ImGuizmo ---
message(STATUS "Fetching ImGuizmo...")
FetchContent_Declare(
    imguizmo
    GIT_REPOSITORY https://github.com/CedricGuillemet/ImGuizmo.git
    GIT_TAG        master
)
FetchContent_MakeAvailable(imguizmo)

# Create an ImGui library to handle includes/sources cleanly
set(IMGUI_DIR ${imgui_SOURCE_DIR})
add_library(ImGuiLib STATIC
    ${IMGUI_DIR}/imgui.cpp
    ${IMGUI_DIR}/imgui_demo.cpp
    ${IMGUI_DIR}/imgui_draw.cpp
    ${IMGUI_DIR}/imgui_tables.cpp
    ${IMGUI_DIR}/imgui_widgets.cpp
    ${IMGUI_DIR}/backends/imgui_impl_glfw.cpp
    ${IMGUI_DIR}/backends/imgui_impl_opengl3.cpp
    ${imguizmo_SOURCE_DIR}/ImGuizmo.cpp
)
target_include_directories(ImGuiLib PUBLIC 
    ${IMGUI_DIR} 
    ${IMGUI_DIR}/backends
    ${glfw_SOURCE_DIR}/include 
    include # to find Voltra headers if backends need them? No, usually not.
    ${imguizmo_SOURCE_DIR}
)
target_link_libraries(ImGuiLib PRIVATE glfw OpenGL::GL) # Backends need these

# --- The Engine ---
file(GLOB_RECURSE ENGINE_SOURCES "src/*.cpp" "src/*.c" "src/*.h" "src/*.hpp")

list(REMOVE_ITEM ENGINE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/main.cpp")

list(APPEND ENGINE_SOURCES
    # ImGui sources are now in ImGuiLib
)

add_executable(${PROJECT_NAME} ${ENGINE_SOURCES} src/main.cpp)

# Include Configuration
target_include_directories(${PROJECT_NAME} PUBLIC 
    include 
    src
    src/Vendor/Glad/include
    ${glfw_SOURCE_DIR}/include
    ${glm_SOURCE_DIR}
    ${stb_SOURCE_DIR}
    ${stb_SOURCE_DIR}
    # ImGui includes handled by linking ImGuiLib
)

target_compile_definitions(${PROJECT_NAME} PRIVATE GLFW_INCLUDE_NONE)

# Library Linking
find_package(OpenGL REQUIRED)

target_link_libraries(${PROJECT_NAME} 
    PRIVATE 
    ImGuiLib 
    glfw 
    glm 
    OpenGL::GL
    spdlog::spdlog
    EnTT::EnTT
    box2d
    ${CMAKE_DL_LIBS}
)

# --- Testing Framework (Google Test) ---
# Enable CMake's testing functionality
enable_testing()

message(STATUS "Fetching Google Test...")
FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG        v1.14.0
)
# Prevent GTest from overriding parent project compiler settings on Windows
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)

FetchContent_MakeAvailable(googletest)

# --- Dependency: Box2D (Physics) ---
message(STATUS "Fetching Box2D...")
FetchContent_Declare(
    box2d
    GIT_REPOSITORY https://github.com/erincatto/box2d.git
    GIT_TAG        v2.4.1
)
set(BOX2D_BUILD_UNIT_TESTS OFF CACHE BOOL "" FORCE)
set(BOX2D_BUILD_TESTBED OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(box2d)

# --- Test Executable ---
# We create a separate executable just for running tests
add_executable(VoltraTests 
    tests/main_test.cpp 
    tests/EventTest.cpp
    tests/BufferTest.cpp
    tests/LayerStackTest.cpp
    tests/TimestepTest.cpp
    tests/ECSTest.cpp
    tests/TextureTest.cpp
    tests/TextureManagerTest.cpp
    ${ENGINE_SOURCES}
)


# Link against GTest and the main functionality required
# Note: GTest::gtest_main provides a standard main() entry point for tests
target_link_libraries(VoltraTests 
    PRIVATE 
    ImGuiLib
    GTest::gtest
    glfw 
    glm 
    OpenGL::GL
    spdlog::spdlog
    EnTT::EnTT
    box2d
    ${CMAKE_DL_LIBS}
)

target_compile_definitions(VoltraTests PRIVATE GLFW_INCLUDE_NONE)

target_include_directories(VoltraTests PRIVATE 
    include
    src
    src/Vendor/Glad/include
    ${glfw_SOURCE_DIR}/include
    ${glm_SOURCE_DIR}
    ${stb_SOURCE_DIR}
)

# Integrate with CTest (allows running "ctest" command)
include(GoogleTest)
gtest_discover_tests(VoltraTests DISCOVERY_TIMEOUT 60)