#include "EditorCamera.hpp"
#include "Core/Input.hpp"
#include "Core/Log.hpp"


#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>


namespace Voltra {

    /**
     * @brief Constructs an EditorCamera with a specific aspect ratio.
     * 
     * @param aspectRatio The aspect ratio (width / height).
     */
    EditorCamera::EditorCamera(float aspectRatio)
        : OrthographicCamera(-aspectRatio * 1.0f, aspectRatio * 1.0f, -1.0f, 1.0f), m_AspectRatio(aspectRatio) {}

    /**
     * @brief Updates camera position based on mouse input.
     * 
     * Handles panning with Right or Middle mouse button.
     * 
     * @param ts The timestep.
     */
    void EditorCamera::OnUpdate(Timestep ts) {
        glm::vec2 mouse = { Input::GetMouseX(), Input::GetMouseY() };
        glm::vec2 delta = (mouse - m_LastMousePosition);
        m_LastMousePosition = mouse;

        if (Input::IsMouseButtonPressed(GLFW_MOUSE_BUTTON_RIGHT) || Input::IsMouseButtonPressed(GLFW_MOUSE_BUTTON_MIDDLE)) {
            glm::vec3 pos = GetPosition();
            
            float heightScale = (m_ZoomLevel * 2.0f) / m_ViewportHeight;
            float widthScale = heightScale;

            pos.x -= delta.x * widthScale;
            pos.y += delta.y * heightScale; 

            SetPosition(pos);
        }
    }

    /**
     * @brief Dispatches events to specific handlers.
     * 
     * @param e The event to handle.
     */
    void EditorCamera::OnEvent(Event& e) {
        EventDispatcher dispatcher(e);
        dispatcher.Dispatch<MouseScrolledEvent>(VOLTRA_BIND_EVENT_FN(EditorCamera::OnMouseScrolled));
    }

    /**
     * @brief Updates the viewport size and recalculates projection.
     * 
     * @param width New width.
     * @param height New height.
     */
    void EditorCamera::SetViewportSize(float width, float height) {
        m_ViewportWidth = width;
        m_ViewportHeight = height;
        m_AspectRatio = width / height;
        CalculateView();
    }

    /**
     * @brief Recalculates the orthographic projection matrix.
     * 
     * Based on zoom level and aspect ratio.
     */
    void EditorCamera::CalculateView() {
        float left = -m_AspectRatio * m_ZoomLevel;
        float right = m_AspectRatio * m_ZoomLevel;
        float bottom = -m_ZoomLevel;
        float top = m_ZoomLevel;
        SetProjection(left, right, bottom, top);
    }

    /**
     * @brief Adjusts the zoom level.
     * 
     * @param delta The zoom amount factor.
     */
    void EditorCamera::Zoom(float delta) {
        m_ZoomLevel -= delta * 0.25f * m_ZoomLevel;
        m_ZoomLevel = std::max(m_ZoomLevel, 0.25f);
        CalculateView();
    }

    /**
     * @brief Handles mouse scroll events (used for zooming elsewhere usually).
     * 
     * @param e The event.
     * @return false (event not blocked).
     */
    bool EditorCamera::OnMouseScrolled(MouseScrolledEvent& e) {
        return false;
    }

    /**
     * @brief Handles window resize events.
     * 
     * @param e The event.
     * @return false.
     */
    bool EditorCamera::OnWindowResized(WindowResizeEvent& e) {
        SetViewportSize((float)e.GetWidth(), (float)e.GetHeight());
        return false;
    }
}
