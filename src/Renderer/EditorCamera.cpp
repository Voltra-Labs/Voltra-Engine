#include "EditorCamera.hpp"
#include "Core/Input.hpp"


#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>


namespace Voltra {

    EditorCamera::EditorCamera(float aspectRatio)
        : OrthographicCamera(-aspectRatio * 1.0f, aspectRatio * 1.0f, -1.0f, 1.0f), m_AspectRatio(aspectRatio)
    {
    }

    void EditorCamera::OnUpdate(Timestep ts)
    {
        // Pan logic
        if (Input::IsMouseButtonPressed(GLFW_MOUSE_BUTTON_RIGHT))
        {
            glm::vec2 mousePos = { Input::GetMouseX(), Input::GetMouseY() };

            // Simple panning logic could be implemented here relative to delta mouse
            // For now, let's implement WASD movement for the editor camera as requested in some contexts,
            // or stick to 2D standard Right-Click Drag.
            
            // NOTE: Delta time based movement matches 2D Engine usage usually
        }

        // Keyboard movement for Editor
        if (Input::IsKeyPressed(GLFW_KEY_A))
        {
            glm::vec3 pos = GetPosition();
            pos.x -= m_ZoomLevel * ts; // Speed scales with zoom
            SetPosition(pos);
        }
        if (Input::IsKeyPressed(GLFW_KEY_D))
        {
            glm::vec3 pos = GetPosition();
            pos.x += m_ZoomLevel * ts;
            SetPosition(pos);
        }
        if (Input::IsKeyPressed(GLFW_KEY_W))
        {
            glm::vec3 pos = GetPosition();
            pos.y += m_ZoomLevel * ts;
            SetPosition(pos);
        }
        if (Input::IsKeyPressed(GLFW_KEY_S))
        {
            glm::vec3 pos = GetPosition();
            pos.y -= m_ZoomLevel * ts;
            SetPosition(pos);
        }


        // Update Projection based on Aspect Ratio and Zoom
        CalculateView();
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

    bool EditorCamera::OnMouseScrolled(MouseScrolledEvent& e)
    {
        m_ZoomLevel -= e.GetYOffset() * 0.25f;
        m_ZoomLevel = std::max(m_ZoomLevel, 0.25f);
        CalculateView();
        return false;
    }

    bool EditorCamera::OnWindowResized(WindowResizeEvent& e)
    {
        SetViewportSize((float)e.GetWidth(), (float)e.GetHeight());
        return false;
    }
}
