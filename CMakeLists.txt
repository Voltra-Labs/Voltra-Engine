cmake_minimum_required(VERSION 3.15)
project(VoltraEngine)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# 1. Define external library paths
set(GLFW_DIR "${CMAKE_SOURCE_DIR}/external/glfw")
set(GLM_DIR "${CMAKE_SOURCE_DIR}/external/glm")

# 2. Include header directories
include_directories(include)
include_directories("${GLFW_DIR}/include")
include_directories("${GLM_DIR}")

# 3. GLFW library directory
link_directories("${GLFW_DIR}/lib-vc2022")

# 4. Find source files
file(GLOB_RECURSE SOURCES "src/*.cpp")

add_executable(${PROJECT_NAME} ${SOURCES})

# 5. Link libraries
target_link_libraries(${PROJECT_NAME} glfw3.lib opengl32.lib user32.lib gdi32.lib shell32.lib)