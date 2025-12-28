#include "EditorCamera.hpp"
#include "Core/Input.hpp"
#include "Core/Log.hpp"


#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>


namespace Voltra {

    EditorCamera::EditorCamera(float aspectRatio)
        : OrthographicCamera(-aspectRatio * 1.0f, aspectRatio * 1.0f, -1.0f, 1.0f), m_AspectRatio(aspectRatio)
    {
    }

    void EditorCamera::OnUpdate(Timestep ts)
    {
        glm::vec2 mouse = { Input::GetMouseX(), Input::GetMouseY() };
        glm::vec2 delta = (mouse - m_LastMousePosition);
        m_LastMousePosition = mouse;

        // Panning (RMB or MMB)
        if (Input::IsMouseButtonPressed(GLFW_MOUSE_BUTTON_RIGHT) || Input::IsMouseButtonPressed(GLFW_MOUSE_BUTTON_MIDDLE))
        {
            glm::vec3 pos = GetPosition();
            
            // Calculate scale based on viewport size to map pixels to world units
            // Ortho Size (Vertical) = 2 * ZoomLevel
            float heightScale = (m_ZoomLevel * 2.0f) / m_ViewportHeight;
            float widthScale = heightScale; // Because pixels are square, and AspectRatio handles the width of viewport

            // X Axis: Moving mouse Right (+X) -> Camera Left (-X) -> World Right
            pos.x -= delta.x * widthScale;
            // Y Axis: Moving mouse Down (+Y) -> Camera Up (+Y) -> World Down
            // (GLFW Y is inverted relative to World Y)
            pos.y += delta.y * heightScale; 

            SetPosition(pos);
        }
    }

    void EditorCamera::OnEvent(Event& e)
    {
        EventDispatcher dispatcher(e);
        dispatcher.Dispatch<MouseScrolledEvent>(VOLTRA_BIND_EVENT_FN(EditorCamera::OnMouseScrolled));
    }

    void EditorCamera::SetViewportSize(float width, float height)
    {
        m_ViewportWidth = width;
        m_ViewportHeight = height;
        m_AspectRatio = width / height;
        CalculateView();
    }

    void EditorCamera::CalculateView()
    {
        float left = -m_AspectRatio * m_ZoomLevel;
        float right = m_AspectRatio * m_ZoomLevel;
        float bottom = -m_ZoomLevel;
        float top = m_ZoomLevel;
        SetProjection(left, right, bottom, top);
    }

    void EditorCamera::Zoom(float delta)
    {
        m_ZoomLevel -= delta * 0.25f * m_ZoomLevel;
        m_ZoomLevel = std::max(m_ZoomLevel, 0.25f);
        CalculateView();
    }

    bool EditorCamera::OnMouseScrolled(MouseScrolledEvent& e)
    {
        // Handled in EditorLayer via ImGui
        return false;
    }

    bool EditorCamera::OnWindowResized(WindowResizeEvent& e)
    {
        SetViewportSize((float)e.GetWidth(), (float)e.GetHeight());
        return false;
    }
}
