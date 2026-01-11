#pragma once

#include "Core.hpp"

#include "Window.hpp"
#include "Events/Event.hpp"
#include "Events/ApplicationEvent.hpp"
#include "Timestep.hpp"
#include "Renderer/OrthographicCamera.hpp"
#include <memory>
#include "LayerStack.hpp"
#include "ImGui/ImGuiLayer.hpp"

namespace Voltra {
    /**
     * @brief The main Application class.
     * 
     * Manages the application lifecycle, window, events, and layer stack.
     * This class is a singleton.
     */
    class VOLTRA_API Application {
    public:
        /**
         * @brief Constructs the Application object.
         */
        Application();

        /**
         * @brief Destroys the Application object.
         */
        virtual ~Application();

        /**
         * @brief Runs the application loop.
         */
        void Run();

        /**
         * @brief Handles application events.
         * 
         * @param e The event to handle.
         */
        void OnEvent(Event& e);
        
        /**
         * @brief Pushes a layer to the layer stack.
         * 
         * @param layer Pointer to the layer to push.
         */
        void PushLayer(Layer* layer);

        /**
         * @brief Pushes an overlay to the layer stack.
         * 
         * @param overlay Pointer to the overlay to push.
         */
        void PushOverlay(Layer* overlay);

        /**
         * @brief Gets the singleton instance of the Application.
         * 
         * @return Reference to the Application instance.
         */
        static Application& Get() { return *s_Instance; }

        /**
         * @brief Gets the application window.
         * 
         * @return Reference to the Window object.
         */
        Window& GetWindow() { return *m_Window; }

    private:
        static Application* s_Instance;
        std::unique_ptr<Window> m_Window;
        ImGuiLayer* m_ImGuiLayer;
        bool m_Running = true;
        LayerStack m_LayerStack;
        float m_LastFrameTime = 0.0f;

    public:
        ImGuiLayer* GetImGuiLayer() { return m_ImGuiLayer; }
    };

    /**
     * @brief Creates the Application instance.
     * 
     * This function must be implemented by the client.
     * 
     * @return Pointer to the created Application instance.
     */
}

// Client App Define
#if defined(VOLTRA_CLIENT_EXPORT) || defined(Sandbox_EXPORTS)
    #define VOLTRA_CLIENT_API __declspec(dllexport)
#else
    #define VOLTRA_CLIENT_API __declspec(dllimport)
#endif

extern "C" VOLTRA_CLIENT_API Voltra::Application* CreateApplication();