#pragma once

#include <glm/glm.hpp>

namespace Voltra {

    /**
     * @brief Static class for handling input polling (keyboard and mouse).
     */
    class Input {
    public:
        /**
         * @brief Checks if a specific keyboard key is pressed.
         * 
         * @param keycode The key code to check.
         * @return true if the key is currently pressed, false otherwise.
         */
        [[nodiscard]] static bool IsKeyPressed(int keycode);
        /**
         * @brief Checks if a specific mouse button is pressed.
         * 
         * @param button The mouse button code to check.
         * @return true if the button is currently pressed, false otherwise.
         */
        [[nodiscard]] static bool IsMouseButtonPressed(int button);
        
        /**
         * @brief Gets the current mouse cursor position.
         * 
         * @return The cursor position as a glm::vec2 (x, y).
         */
        [[nodiscard]] static glm::vec2 GetMousePosition();
        /**
         * @brief Gets the current X position of the mouse cursor.
         * 
         * @return The horizontal position of the cursor.
         */
        [[nodiscard]] static float GetMouseX();
        /**
         * @brief Gets the current Y position of the mouse cursor.
         * 
         * @return The vertical position of the cursor.
         */
        [[nodiscard]] static float GetMouseY();
    };

}