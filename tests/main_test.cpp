#include <gtest/gtest.h>
#include <glad/glad.h>
#include <GLFW/glfw3.h>
#include <iostream>

//For LOGS
#include "Core/Log.hpp"
#include "Renderer/AssetManager.hpp"

// Callback to print GLFW internal errors
void GLFWErrorCallback(int error, const char* description) {
    VOLTRA_CORE_ERROR("GLFW Error ({0}): {1}", error, description);
}

int main(int argc, char** argv) {
    ::testing::InitGoogleTest(&argc, argv);

    Voltra::Log::Init();

    // List tests
    if (::testing::GTEST_FLAG(list_tests)) {
        return RUN_ALL_TESTS();
    }

    glfwSetErrorCallback(GLFWErrorCallback);

    // Initialize GLFW
    if (!glfwInit()) {
        VOLTRA_CORE_FATAL("Failed to initialize GLFW");
        return -1;
    }

    // Headless Configuration (Window hidden)
    glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
    
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 5);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    // Use software rendering if hardware not available (useful for CI)
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_NATIVE_CONTEXT_API);

    VOLTRA_CORE_INFO("Attempting to create headless window (OpenGL 4.5)...");

    GLFWwindow* window = glfwCreateWindow(640, 480, "Voltra Test Context", nullptr, nullptr);
    if (!window) {
        VOLTRA_CORE_FATAL("Failed to create GLFW window/context.");
        VOLTRA_CORE_ERROR("Possible causes: GPU driver too old or Mesa3D DLLs not found.");
        glfwTerminate();
        return -1;
    }

    // Make the context current (Vital for GL calls to work)
    glfwMakeContextCurrent(window);

    // Load OpenGL pointers with GLAD
    if (!gladLoadGLLoader((GLADloadproc)glfwGetProcAddress)) {
        VOLTRA_CORE_FATAL("Failed to initialize GLAD");
        glfwDestroyWindow(window);
        glfwTerminate();
        return -1;
    }

    VOLTRA_CORE_INFO("OpenGL Info:");
    VOLTRA_CORE_TRACE("  Vendor:   {0}", (const char*)glGetString(GL_VENDOR));
    VOLTRA_CORE_TRACE("  Renderer: {0}", (const char*)glGetString(GL_RENDERER));
    VOLTRA_CORE_TRACE("  Version:  {0}", (const char*)glGetString(GL_VERSION));

    int result = RUN_ALL_TESTS();

    // Cleanup
    Voltra::AssetManager::Clear();
    glfwDestroyWindow(window);
    glfwTerminate();

    return result;
}