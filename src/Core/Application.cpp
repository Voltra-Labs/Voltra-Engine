#include "Application.hpp"
#include <GLFW/glfw3.h>

namespace Voltra {

    Application::Application() {
        m_Window = std::make_unique<Window>();
    }

    Application::~Application() {
    }

    void Application::Run() {
        while (m_Running && !m_Window->ShouldClose()) {
            // Temporary render loop
            glClearColor(0.2f, 0.2f, 0.2f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT);

            m_Window->OnUpdate();
        }
    }

}