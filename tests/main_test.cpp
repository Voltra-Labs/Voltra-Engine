#include <gtest/gtest.h>
#include <glad/glad.h>
#include <GLFW/glfw3.h>
#include <iostream>

int main(int argc, char** argv) {
    ::testing::InitGoogleTest(&argc, argv);

    // Initialize GLFW
    if (!glfwInit()) {
        std::cerr << "[TESTS] CRITICAL: Failed to initialize GLFW" << std::endl;
        return -1;
    }

    // Configure hidden window (Headless)
    glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 5);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    GLFWwindow* window = glfwCreateWindow(640, 480, "Voltra Test Context", nullptr, nullptr);
    if (!window) {
        std::cerr << "[TESTS] CRITICAL: Failed to create GLFW window/context" << std::endl;
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

    std::cout << "[TESTS] OpenGL Context Initialized: " << glGetString(GL_VERSION) << std::endl;

    // Run all tests
    int result = RUN_ALL_TESTS();

    // Cleanup
    glfwDestroyWindow(window);
    glfwTerminate();

    return result;
}