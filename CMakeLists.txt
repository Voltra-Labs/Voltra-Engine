cmake_minimum_required(VERSION 3.15)
project(VoltraEngine VERSION 0.0.1 LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Module to automatically download libraries
include(FetchContent)

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

# --- The Engine ---
file(GLOB_RECURSE ENGINE_SOURCES "src/*.cpp" "src/*.c" "src/*.h" "src/*.hpp")

list(REMOVE_ITEM ENGINE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/src/main.cpp")

add_executable(${PROJECT_NAME} ${ENGINE_SOURCES} src/main.cpp)

# Include Configuration
target_include_directories(${PROJECT_NAME} PUBLIC 
    include 
    src
    src/Vendor/Glad/include
    ${glfw_SOURCE_DIR}/include
    ${glm_SOURCE_DIR}
)

# Library Linking
find_package(OpenGL REQUIRED)

target_link_libraries(${PROJECT_NAME} 
    PRIVATE 
    glfw 
    glm 
    OpenGL::GL
    spdlog::spdlog
    ${CMAKE_DL_LIBS}
)

# --- 4. Testing Framework (Google Test) ---
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

# --- 5. Test Executable ---
# We create a separate executable just for running tests
add_executable(VoltraTests 
    tests/main_test.cpp 
    tests/EventTest.cpp
    tests/BufferTest.cpp
    ${ENGINE_SOURCES}
)


# Link against GTest and the main functionality required
# Note: GTest::gtest_main provides a standard main() entry point for tests
target_link_libraries(VoltraTests 
    PRIVATE 
    GTest::gtest_main
    glfw 
    glm 
    OpenGL::GL
    spdlog::spdlog
    ${CMAKE_DL_LIBS}
)

target_include_directories(VoltraTests PRIVATE 
    include
    src
    src/Vendor/Glad/include
    ${glfw_SOURCE_DIR}/include
    ${glm_SOURCE_DIR}
)

# Integrate with CTest (allows running "ctest" command)
include(GoogleTest)
gtest_discover_tests(VoltraTests)