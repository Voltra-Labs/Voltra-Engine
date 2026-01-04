#include "Window.hpp"
#include "Log.hpp"
#include "../Events/ApplicationEvent.hpp"
#include "../Events/KeyEvent.hpp"
#include "../Events/MouseEvent.hpp"
#include <glad/glad.h>
#include <GLFW/glfw3.h>

namespace Voltra {

    /**
     * @brief Constructs a new Window object.
     * 
     * @param props The window configuration properties.
     */
    Window::Window(const Properties& props) {
        Init(props);
    }

    /**
     * @brief Destroys the Window object.
     */
    Window::~Window() {
        Shutdown();
    }

    /**
     * @brief Initializes the GLFW window and OpenGL context.
     * 
     * @param props The window properties.
     * @note Creates the context, loads GLAD, and sets up GLFW callbacks.
     */
    void Window::Init(const Properties& props) {
        m_Data.Title = props.Title;
        m_Data.Width = props.Width;
        m_Data.Height = props.Height;

        VOLTRA_CORE_INFO("Creating window {0} ({1}, {2})", props.Title, props.Width, props.Height);

        if (!glfwInit()) {
            VOLTRA_CORE_FATAL("Failed to initialize GLFW!");
            return;
        }

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
        
        int status = gladLoadGLLoader((GLADloadproc)glfwGetProcAddress);
        if (!status) {
            VOLTRA_CORE_FATAL("Failed to initialize Glad!");
            return;
        }
        
        VOLTRA_CORE_INFO("OpenGL Info:");
        VOLTRA_CORE_INFO("  Vendor:   {0}", (const char*)glGetString(GL_VENDOR));
        VOLTRA_CORE_INFO("  Renderer: {0}", (const char*)glGetString(GL_RENDERER));
        VOLTRA_CORE_INFO("  Version:  {0}", (const char*)glGetString(GL_VERSION));

        glViewport(0, 0, props.Width, props.Height);

        glfwSetWindowSizeCallback(m_Window, [](GLFWwindow* window, int width, int height) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);
            data.Width = width;
            data.Height = height;

            WindowResizeEvent event(width, height);
            if (data.EventCallback) 
                data.EventCallback(event);
        });

        glfwSetWindowCloseCallback(m_Window, [](GLFWwindow* window) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);
            WindowCloseEvent event;
            if (data.EventCallback) data.EventCallback(event);
        });

        glfwSetKeyCallback(m_Window, [](GLFWwindow* window, int key, int scancode, int action, int mods) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);
            
            switch (action) {
                case GLFW_PRESS: {
                    KeyPressedEvent event(key, 0);
                    if (data.EventCallback) data.EventCallback(event);
                    break;
                }
                case GLFW_RELEASE: {
                    KeyReleasedEvent event(key);
                    if (data.EventCallback) data.EventCallback(event);
                    break;
                }
                case GLFW_REPEAT: {
                    KeyPressedEvent event(key, 1);
                    if (data.EventCallback) data.EventCallback(event);
                    break;
                }
            }
        });

        glfwSetMouseButtonCallback(m_Window, [](GLFWwindow* window, int button, int action, int mods) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);

            switch (action) {
                case GLFW_PRESS: {
                    MouseButtonPressedEvent event(button);
                    if (data.EventCallback) data.EventCallback(event);
                    break;
                }
                case GLFW_RELEASE: {
                    MouseButtonReleasedEvent event(button);
                    if (data.EventCallback) data.EventCallback(event);
                    break;
                }
            }
        });

        glfwSetScrollCallback(m_Window, [](GLFWwindow* window, double xOffset, double yOffset) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);
            MouseScrolledEvent event((float)xOffset, (float)yOffset);
            if (data.EventCallback) data.EventCallback(event);
        });

        glfwSetCursorPosCallback(m_Window, [](GLFWwindow* window, double xPos, double yPos) {
            WindowData& data = *(WindowData*)glfwGetWindowUserPointer(window);
            MouseMovedEvent event((float)xPos, (float)yPos);
            if (data.EventCallback) data.EventCallback(event);
        });
    }

    /**
     * @brief Shuts down the window and terminates GLFW.
     */
    void Window::Shutdown() {
        if (m_Window) {
            glfwDestroyWindow(m_Window);
        }
        glfwTerminate();
    }

    /**
     * @brief Updates the window frame.
     * 
     * Polls events and swaps buffers.
     */
    void Window::OnUpdate() {
        glfwPollEvents();
        glfwSwapBuffers(m_Window);
    }

    /**
     * @brief Checks if the window close flag is set.
     * 
     * @return true if the window should close, false otherwise.
     */
    bool Window::ShouldClose() const {
        return glfwWindowShouldClose(m_Window);
    }

}