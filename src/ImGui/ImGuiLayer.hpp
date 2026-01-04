#pragma once

#include "Core/Layer.hpp"
#include "Events/ApplicationEvent.hpp"
#include "Events/KeyEvent.hpp"
#include "Events/MouseEvent.hpp"

namespace Voltra {

    /**
     * @brief Layer that handles ImGui rendering and events.
     * 
     * Sets up the ImGui context, style, and platform backends.
     */
    class ImGuiLayer : public Layer {
    public:
        /**
         * @brief Constructs the ImGuiLayer.
         */
        ImGuiLayer();

        /**
         * @brief Destroys the ImGuiLayer.
         */
        ~ImGuiLayer();

        /**
         * @brief Initializes ImGui and installs backends.
         */
        virtual void OnAttach() override;

        /**
         * @brief Shuts down ImGui and uninstalls backends.
         */
        virtual void OnDetach() override;

        /**
         * @brief Renders ImGui draw data.
         * 
         * @note Usually empty as we use Begin/End for the dockspace.
         */
        virtual void OnImGuiRender() override;

        /**
         * @brief Handles events to block them if ImGui captures mouse/keyboard.
         * 
         * @param event The event to handle.
         */
        virtual void OnEvent(Event& event) override;

        /**
         * @brief Begins a new ImGui frame.
         */
        void Begin();

        /**
         * @brief Ends the ImGui frame and renders it.
         */
        void End();

    private:
        float m_Time = 0.0f;
    };

}
