#pragma once

#include "OrthographicCamera.hpp"
#include "Core/Timestep.hpp"
#include "Events/Event.hpp"
#include "Events/MouseEvent.hpp"
#include "Events/ApplicationEvent.hpp"


namespace Voltra {

    class EditorCamera : public OrthographicCamera {
    public:
        EditorCamera(float aspectRatio);

        void OnUpdate(Timestep ts);
        void OnEvent(Event& e);

        void SetViewportSize(float width, float height);

        float GetZoomLevel() const { return m_ZoomLevel; }
        void SetZoomLevel(float level) { m_ZoomLevel = level; CalculateView(); }
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
