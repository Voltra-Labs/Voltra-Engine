#include "Window.hpp"
#include "Log.hpp"
#include <glad/glad.h>
#include <GLFW/glfw3.h>

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
        
#ifdef __APPLE__
        glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GL_TRUE);
#endif

        m_Window = glfwCreateWindow((int)props.Width, (int)props.Height, m_Data.Title.c_str(), nullptr, nullptr);
        
        if (!m_Window) {
            VOLTRA_CORE_FATAL("Failed to create GLFW window!");
            glfwTerminate();
            return;
        }

        glfwMakeContextCurrent(m_Window);
        glfwSetWindowUserPointer(m_Window, &m_Data);
        
        // --- Glad initialization ---
        int status = gladLoadGLLoader((GLADloadproc)glfwGetProcAddress);
        if (!status) {
            VOLTRA_CORE_FATAL("Failed to initialize Glad!");
            return;
        }
        
        VOLTRA_CORE_INFO("OpenGL Info:");
        VOLTRA_CORE_INFO("  Vendor:   {0}", (const char*)glGetString(GL_VENDOR));
        VOLTRA_CORE_INFO("  Renderer: {0}", (const char*)glGetString(GL_RENDERER));
        VOLTRA_CORE_INFO("  Version:  {0}", (const char*)glGetString(GL_VERSION));

        // --- Basic OpenGL configuration ---
        glViewport(0, 0, props.Width, props.Height);
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

    bool Window::ShouldClose() const {
        return glfwWindowShouldClose(m_Window);
    }

}