#pragma once

#include "Window.hpp"
#include "Events/Event.hpp"
#include "Events/ApplicationEvent.hpp"
#include <memory>

namespace Voltra {

    class Shader;
    class VertexArray;

    class Application {
    public:
        Application();
        virtual ~Application();

        void Run();
        void OnEvent(Event& e);

    private:
        std::unique_ptr<Window> m_Window;
        bool m_Running = true;

        std::shared_ptr<Shader> m_Shader;
        std::shared_ptr<VertexArray> m_VertexArray;
    };

    // Defined in client (main.cpp)
    Application* CreateApplication();

}