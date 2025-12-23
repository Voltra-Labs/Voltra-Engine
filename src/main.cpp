#include <GLFW/glfw3.h>
#include <iostream>

int main() {
    // Initialize Voltra Engine
    if (!glfwInit()) {
        return -1;
    }

    // Configure GLFW to use OpenGL 4.6
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 6);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

    // Create the engine window
    GLFWwindow* window = glfwCreateWindow(1280, 720, "Voltra Engine v0.0.1", NULL, NULL);
    if (!window) {
        glfwTerminate();
        return -1;
    }

    glfwMakeContextCurrent(window);

    std::cout << "Voltra Engine: Successfully initialized." << std::endl;

    // Game Loop
    while (!glfwWindowShouldClose(window)) {
        // Rendering
        glClearColor(0.8f, 0.1f, 0.1f, 1.0f); // Clear screen with a reddish color
        glClear(GL_COLOR_BUFFER_BIT);

        // Future Voltra rendering system implementation goes here

        glfwSwapBuffers(window);
        glfwPollEvents();
    }

    glfwTerminate();
    return 0;
}