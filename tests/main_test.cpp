#include <gtest/gtest.h>
#include <glad/glad.h>
#include <GLFW/glfw3.h>
#include <iostream>

// Callback to print GLFW internal errors
void GLFWErrorCallback(int error, const char* description) {
    std::cerr << "[GLFW ERROR] (" << error << "): " << description << std::endl;
}

int main(int argc, char** argv) {
    ::testing::InitGoogleTest(&argc, argv);

    // List tests
    if (::testing::GTEST_FLAG(list_tests)) {
        return RUN_ALL_TESTS();
    }

    glfwSetErrorCallback(GLFWErrorCallback);

    // Initialize GLFW
    if (!glfwInit()) {
        std::cerr << "[TESTS] CRITICAL: Failed to initialize GLFW" << std::endl;
        return -1;
    }

    // Headless Configuration (Window hidden)
    glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
    
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 5);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    // Use software rendering if hardware not available (useful for CI)
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_NATIVE_CONTEXT_API);

    std::cout << "[TESTS] Attempting to create window with OpenGL 4.5..." << std::endl;

    GLFWwindow* window = glfwCreateWindow(640, 480, "Voltra Test Context", nullptr, nullptr);
    if (!window) {
        std::cerr << "[TESTS] CRITICAL: Failed to create GLFW window/context." << std::endl;
        std::cerr << "        Possible causes: GPU driver too old or Mesa3D DLLs not found in .exe folder." << std::endl;
        glfwTerminate();
        return -1;
    }

    // Make the context current (Vital for GL calls to work)
    glfwMakeContextCurrent(window);

    // Load OpenGL pointers with GLAD
    if (!gladLoadGLLoader((GLADloadproc)glfwGetProcAddress)) {
        std::cerr << "[TESTS] CRITICAL: Failed to initialize GLAD" << std::endl;
        glfwDestroyWindow(window);
        glfwTerminate();
        return -1;
    }

    std::cout << "[TESTS] OpenGL Info:" << std::endl;
    std::cout << "  Vendor:   " << glGetString(GL_VENDOR) << std::endl;
    std::cout << "  Renderer: " << glGetString(GL_RENDERER) << std::endl;
    std::cout << "  Version:  " << glGetString(GL_VERSION) << std::endl;

    int result = RUN_ALL_TESTS();

    // Cleanup
    glfwDestroyWindow(window);
    glfwTerminate();

    return result;
}