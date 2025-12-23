#include "Window.hpp"
#include <iostream>

namespace Voltra {

    Window::Window(const Properties& props) {
        Init(props);
    }

    Window::~Window() {
        Shutdown();
    }

    void Window::Init(const Properties& props) {
        m_Data = props;

        if (!glfwInit()) {
            // TODO: Use a proper logging system instead of stderr
            std::cerr << "[Window] Failed to initialize GLFW!" << std::endl;
            return;
        }

        // Configure for OpenGL 4.6 Core Profile
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 6);
        glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

        m_Window = glfwCreateWindow((int)props.Width, (int)props.Height, m_Data.Title.c_str(), nullptr, nullptr);
        
        if (!m_Window) {
            std::cerr << "[Window] Failed to create GLFW window!" << std::endl;
            glfwTerminate();
            return;
        }

        glfwMakeContextCurrent(m_Window);
        
        // TODO: Initialize GLAD/OpenGL loader here
        
        std::cout << "[Window] Initialized: " << props.Width << "x" << props.Height << std::endl;
    }

    void Window::Shutdown() {
        if (m_Window) {
            glfwDestroyWindow(m_Window);
        }
        glfwTerminate();
    }

    void Window::OnUpdate() {
        glfwPollEvents();
        glfwSwapBuffers(m_Window);
    }

}