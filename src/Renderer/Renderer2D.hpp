#pragma once

#include "OrthographicCamera.hpp"
#include "Texture.hpp"
#include <glm/glm.hpp>
#include <memory>

namespace Voltra {

    /**
     * @brief 2D Rendering System.
     * 
     * Handles batching (in future) and rendering of 2D primitives like quads.
     */
    class Renderer2D {
    public:
        /**
         * @brief Initializes the 2D renderer.
         */
        static void Init();

        /**
         * @brief Shuts down the 2D renderer.
         */
        static void Shutdown();

        /**
         * @brief Begins a new 2D scene.
         * 
         * @param camera The orthographic camera.
         */
        static void BeginScene(const OrthographicCamera& camera);

        /**
         * @brief Ends the current scene.
         */
        static void EndScene();

        // Primitives drawing (Colors)

        /**
         * @brief Draws a colored quad.
         * 
         * @param position 2D Position.
         * @param size Size (width, height).
         * @param color RGBA Color.
         */
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const glm::vec4& color);

        /**
         * @brief Draws a colored quad at a 3D position (Z-depth).
         * 
         * @param position 3D Position.
         * @param size Size (width, height).
         * @param color RGBA Color.
         */
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const glm::vec4& color);

        /**
         * @brief Draws a colored quad with a generic transform.
         * 
         * @param transform Transformation matrix.
         * @param color RGBA Color.
         */
        static void DrawQuad(const glm::mat4& transform, const glm::vec4& color);

        // Primitives drawing (Textures)

        /**
         * @brief Draws a textured quad.
         * 
         * @param position 2D Position.
         * @param size Size.
         * @param texture Texture to apply.
         */
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);

        /**
         * @brief Draws a textured quad at a 3D position.
         * 
         * @param position 3D Position.
         * @param size Size.
         * @param texture Texture to apply.
         */
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);

        /**
         * @brief Draws a textured quad with a generic transform.
         * 
         * @param transform Transformation matrix.
         * @param texture Texture to apply.
         */
        static void DrawQuad(const glm::mat4& transform, const std::shared_ptr<Texture2D>& texture, float tilingFactor = 1.0f, const glm::vec4& tintColor = glm::vec4(1.0f));
    };

}