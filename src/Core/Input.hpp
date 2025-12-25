#pragma once

#include <glm/glm.hpp>

namespace Voltra {

    // Static input polling class
    class Input {
    public:
        [[nodiscard]] static bool IsKeyPressed(int keycode);
        [[nodiscard]] static bool IsMouseButtonPressed(int button);
        
        [[nodiscard]] static glm::vec2 GetMousePosition();
        [[nodiscard]] static float GetMouseX();
        [[nodiscard]] static float GetMouseY();
    };

}