#pragma once

#include "Window.hpp"
#include "Events/Event.hpp"
#include "Events/ApplicationEvent.hpp"
#include "Renderer/OrthographicCamera.hpp"
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
        
        // Helper to get the application instance
        static Application& Get() { return *s_Instance; }
        Window& GetWindow() { return *m_Window; }

    private:
        static Application* s_Instance;

        std::unique_ptr<Window> m_Window;
        bool m_Running = true;

        std::shared_ptr<Shader> m_Shader;
        std::shared_ptr<VertexArray> m_VertexArray;
        
        OrthographicCamera m_Camera;
    };

    // Defined in client (main.cpp)
    Application* CreateApplication();

}