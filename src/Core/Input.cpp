#include "Input.hpp"
#include "Application.hpp"
#include <GLFW/glfw3.h>

namespace Voltra {

    bool Input::IsKeyPressed(int keycode) {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        auto state = glfwGetKey(window, keycode);
        return state == GLFW_PRESS || state == GLFW_REPEAT;
    }

    bool Input::IsMouseButtonPressed(int button) {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        auto state = glfwGetMouseButton(window, button);
        return state == GLFW_PRESS;
    }

    glm::vec2 Input::GetMousePosition() {
        auto* window = static_cast<GLFWwindow*>(Application::Get().GetWindow().GetNativeWindow());
        double x_pos, y_pos;
        glfwGetCursorPos(window, &x_pos, &y_pos);
        
        return { (float)x_pos, (float)y_pos };
    }

    float Input::GetMouseX() {
        return GetMousePosition().x;
    }

    float Input::GetMouseY() {
        return GetMousePosition().y;
    }

}