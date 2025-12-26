#include "Application.hpp"
#include "Core/Log.hpp"
#include "Renderer/Renderer.hpp"
#include <GLFW/glfw3.h>
#include "../Sandbox/ExampleLayer.hpp"

namespace Voltra {

    Application* Application::s_Instance = nullptr;

    Application::Application() {
        if (s_Instance) VOLTRA_CORE_ERROR("Application already exists!");
        s_Instance = this;

        m_Window = std::make_unique<Window>(Window::Properties("Voltra Engine", 1280, 720));
        m_Window->SetEventCallback(std::bind(&Application::OnEvent, this, std::placeholders::_1));

        Renderer::Init();

        // Example layer
        PushLayer(new ExampleLayer());
    }

    Application::~Application() {}

    void Application::PushLayer(Layer* layer) {
        m_LayerStack.PushLayer(layer);
    }

    void Application::PushOverlay(Layer* overlay) {
        m_LayerStack.PushOverlay(overlay);
    }

    void Application::OnEvent(Event& e) {
        if (e.GetEventType() == EventType::WindowClose) {
            m_Running = false;
        }

        // Propagate events to layers (from top to bottom)
        for (auto it = m_LayerStack.end(); it != m_LayerStack.begin(); ) {
            (*--it)->OnEvent(e);
            if (e.Handled) break;
        }
    }

    void Application::Run() {
        while (m_Running) {
            float time = (float)glfwGetTime(); 
            Timestep timestep = time - m_LastFrameTime;
            m_LastFrameTime = time;

            RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1.0f });
            RenderCommand::Clear();

            // Update all layers
            for (Layer* layer : m_LayerStack)
                layer->OnUpdate(timestep);

            m_Window->OnUpdate();
        }
    }
}