#include "Input.hpp"
#include "Application.hpp"
#include <GLFW/glfw3.h>

namespace Voltra {

    /**
     * @brief Checks if a specific key is currently pressed.
     * 
     * @param keycode The GLFW keycode to check.
     * @return true if the key is pressed or repeating, false otherwise.
     */
    bool Input::IsKeyPressed(int keycode) {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        auto state = glfwGetKey(window, keycode);
        return state == GLFW_PRESS || state == GLFW_REPEAT;
    }

    /**
     * @brief Checks if a specific mouse button is currently pressed.
     * 
     * @param button The GLFW mouse button code to check.
     * @return true if the button is pressed, false otherwise.
     */
    bool Input::IsMouseButtonPressed(int button) {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        auto state = glfwGetMouseButton(window, button);
        return state == GLFW_PRESS;
    }

    /**
     * @brief Retrieves the current mouse cursor position.
     * 
     * @return glm::vec2 containing the X and Y coordinates of the cursor relative to the window.
     */
    glm::vec2 Input::GetMousePosition() {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        double x_pos, y_pos;
        glfwGetCursorPos(window, &x_pos, &y_pos);
        
        return { (float)x_pos, (float)y_pos };
    }

    /**
     * @brief Retrieves the X-coordinate of the mouse cursor.
     * 
     * @return The current X position of the mouse cursor.
     */
    float Input::GetMouseX() {
        return GetMousePosition().x;
    }

    /**
     * @brief Retrieves the Y-coordinate of the mouse cursor.
     * 
     * @return The current Y position of the mouse cursor.
     */
    float Input::GetMouseY() {
        return GetMousePosition().y;
    }

}