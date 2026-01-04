#pragma once

#include "OrthographicCamera.hpp"
#include "Core/Timestep.hpp"
#include "Events/Event.hpp"
#include "Events/MouseEvent.hpp"
#include "Events/ApplicationEvent.hpp"


namespace Voltra {

    /**
     * @brief Camera controlled by the editor (pan, zoom).
     * 
     * Provides orthographic projection and mouse interaction.
     */
    class EditorCamera : public OrthographicCamera {
    public:
        /**
         * @brief Constructs an EditorCamera.
         * 
         * @param aspectRatio The initial aspect ratio.
         */
        EditorCamera(float aspectRatio);

        /**
         * @brief Updates the camera state (input handling).
         * 
         * @param ts Time step.
         */
        void OnUpdate(Timestep ts);

        /**
         * @brief Handles events (scrolling, resizing).
         * 
         * @param e The event.
         */
        void OnEvent(Event& e);

        /**
         * @brief Sets the viewport size for aspect ratio calculation.
         * 
         * @param width Viewport width.
         * @param height Viewport height.
         */
        void SetViewportSize(float width, float height);

        /**
         * @brief Gets the current zoom level.
         * 
         * @return Zoom level.
         */
        float GetZoomLevel() const { return m_ZoomLevel; }

        /**
         * @brief Sets the zoom level directly.
         * 
         * @param level New zoom level.
         */
        void SetZoomLevel(float level) { m_ZoomLevel = level; CalculateView(); }

        /**
         * @brief Zooms by a delta amount.
         * 
         * @param delta Amount to zoom.
         */
        void Zoom(float delta);

    private:
        bool OnMouseScrolled(MouseScrolledEvent& e);
        bool OnWindowResized(WindowResizeEvent& e);
        
        void CalculateView();

    private:
        float m_AspectRatio;
        float m_ZoomLevel = 1.0f;
        float m_ViewportWidth = 1280.0f;
        float m_ViewportHeight = 720.0f;
        glm::vec2 m_LastMousePosition = { 0.0f, 0.0f };
    };

}
