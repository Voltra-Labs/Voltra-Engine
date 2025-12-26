#pragma once

#include "Window.hpp"
#include "Events/Event.hpp"
#include "Events/ApplicationEvent.hpp"
#include "Timestep.hpp"
#include "Renderer/OrthographicCamera.hpp"
#include <memory>
#include "LayerStack.hpp"
#include "ImGui/ImGuiLayer.hpp"

namespace Voltra {
    class Application {
    public:
        Application();
        virtual ~Application();

        void Run();
        void OnEvent(Event& e);
        
        void PushLayer(Layer* layer);
        void PushOverlay(Layer* overlay);

        static Application& Get() { return *s_Instance; }
        Window& GetWindow() { return *m_Window; }

    private:
        static Application* s_Instance;
        std::unique_ptr<Window> m_Window;
        ImGuiLayer* m_ImGuiLayer;
        bool m_Running = true;
        LayerStack m_LayerStack;
        float m_LastFrameTime = 0.0f;
    };

    // Defined in client (main.cpp)
    Application* CreateApplication();

}