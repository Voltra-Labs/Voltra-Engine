#include "Window.hpp"
#include "Log.hpp"

namespace Voltra {

    Window::Window(const Properties& props) {
        Init(props);
    }

    Window::~Window() {
        Shutdown();
    }

    void Window::Init(const Properties& props) {
        m_Data = props;

        VOLTRA_CORE_INFO("Creating window {0} ({1}, {2})", props.Title, props.Width, props.Height);

        if (!glfwInit()) {
            VOLTRA_CORE_FATAL("Failed to initialize GLFW!");
            return;
        }

        // Configure for OpenGL 4.6 Core Profile
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 4);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 6);
        glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

        m_Window = glfwCreateWindow((int)props.Width, (int)props.Height, m_Data.Title.c_str(), nullptr, nullptr);
        
        if (!m_Window) {
            VOLTRA_CORE_FATAL("Failed to create GLFW window!");
            glfwTerminate();
            return;
        }

        glfwMakeContextCurrent(m_Window);
        
        // TODO: Initialize GLAD/OpenGL loader here
        
        VOLTRA_CORE_INFO("Initialized: {0}x{1}", props.Width, props.Height);
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