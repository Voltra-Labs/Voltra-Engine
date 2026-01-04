#include "Application.hpp"
#include "Core/Log.hpp"
#include "Renderer/Renderer.hpp"
#include <GLFW/glfw3.h>
#include "../Sandbox/ExampleLayer.hpp"
#include "Renderer/Renderer2D.hpp"

namespace Voltra {

    Application* Application::s_Instance = nullptr;

    /**
     * @brief Initializes the Application, creates the window, and initializes the renderer.
     * 
     * @note This constructor initializes the singleton instance and sets up the event callbacks.
     */
    Application::Application() {
        if (s_Instance) VOLTRA_CORE_ERROR("Application already exists!");
        s_Instance = this;

        m_Window = std::make_unique<Window>(Window::Properties("Voltra Engine", 1920, 1080));
        m_Window->SetEventCallback(std::bind(&Application::OnEvent, this, std::placeholders::_1));

        Renderer::Init();
        Renderer2D::Init();

        m_ImGuiLayer = new ImGuiLayer();
        PushOverlay(m_ImGuiLayer);

        PushLayer(new ExampleLayer());
    }

    /**
     * @brief Destroys the Application object.
     */
    Application::~Application() {}

    /**
     * @brief Pushes a new Layer to the LayerStack.
     * 
     * @param layer Pointer to the layer to be added.
     */
    void Application::PushLayer(Layer* layer) {
        m_LayerStack.PushLayer(layer);
    }

    /**
     * @brief Pushes a new Overlay to the LayerStack.
     * 
     * @param overlay Pointer to the overlay to be added.
     */
    void Application::PushOverlay(Layer* overlay) {
        m_LayerStack.PushOverlay(overlay);
    }

    /**
     * @brief Handles events dispatched to the Application.
     * 
     * @param e The event to be handled.
     */
    void Application::OnEvent(Event& e) {
        if (e.GetEventType() == EventType::WindowClose) {
            m_Running = false;
        }

        for (auto it = m_LayerStack.end(); it != m_LayerStack.begin(); ) {
            (*--it)->OnEvent(e);
            if (e.Handled) break;
        }
    }

    /**
     * @brief Runs the main application loop.
     * 
     * @note Covers the update loop, layer updates, ImGui rendering, and window updates.
     */
    void Application::Run() {
        while (m_Running) {
            float time = (float)glfwGetTime(); 
            Timestep timestep = time - m_LastFrameTime;
            m_LastFrameTime = time;

            RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1.0f });
            RenderCommand::Clear();

            for (Layer* layer : m_LayerStack)
                layer->OnUpdate(timestep);

            m_ImGuiLayer->Begin();
            for (Layer* layer : m_LayerStack)
                layer->OnImGuiRender();
            m_ImGuiLayer->End();

            m_Window->OnUpdate();
        }
    }
}