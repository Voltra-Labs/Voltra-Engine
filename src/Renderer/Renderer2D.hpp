#pragma once

#include "Core/Core.hpp"
#include "OrthographicCamera.hpp"
#include "Texture.hpp"

#include <glm/glm.hpp>
#include <memory>

namespace Voltra {

    /**
     * @brief 2D Rendering System.
     * 
     * Handles efficient batch rendering of 2D primitives (quads, sprites, etc.).
     * All geometry is aggregated into a single dynamic vertex buffer and flushed
     * in minimal draw calls.
     */
    class VOLTRA_API Renderer2D {
    public:
        /**
         * @brief Statistics structure for profiling rendering performance.
         */
        struct Statistics {
            uint32_t DrawCalls = 0;
            uint32_t QuadCount = 0;

            /**
             * @brief Gets the total vertex count.
             * @return Total vertices.
             */
            uint32_t GetTotalVertexCount() const { return QuadCount * 4; }

            /**
             * @brief Gets the total index count.
             * @return Total indices.
             */
            uint32_t GetTotalIndexCount() const { return QuadCount * 6; }
        };

    public:
        /**
         * @brief Initializes the 2D renderer.
         * 
         * Sets up the vertex array, vertex buffers, shaders, and textures.
         * allocates memory for the batch renderer.
         */
        static void Init();

        /**
         * @brief Shuts down the 2D renderer.
         * 
         * Frees all allocated resources and buffers.
         */
        static void Shutdown();

        /**
         * @brief Begins a new 2D scene.
         * 
         * Resets all internal statistics and batch data, preparing for a new frame.
         * 
         * @param camera The orthographic camera used for rendering.
         */
        static void BeginScene(const OrthographicCamera& camera);

        /**
         * @brief Ends the current scene.
         * 
         * Flushes any remaining geometry to the GPU.
         */
        static void EndScene();

        /**
         * @brief Flushes the current batch.
         * 
         * Sends all accumulated geometry to the GPU and resets the batch.
         * This is automatically called when the buffer is full or at EndScene.
         */
        static void Flush();

        // Primitives drawing (Colors)

        /**
         * @brief Draws a colored quad.
         * 
         * @param position The 2D position (x, y) of the quad center.
         * @param size The size (width, height) of the quad.
         * @param color The RGBA color of the quad.
         */
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const glm::vec4& color);

        /**
         * @brief Draws a colored quad at a specific depth.
         * 
         * @param position The 3D position (x, y, z) of the quad center.
         * @param size The size (width, height) of the quad.
         * @param color The RGBA color of the quad.
         */
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const glm::vec4& color);

        /**
         * @brief Draws a colored quad with a generic transform matrix.
         * 
         * @param transform The transformation matrix (translation * rotation * scale).
         * @param color The RGBA color of the quad.
         */
        static void DrawQuad(const glm::mat4& transform, const glm::vec4& color);

        // Primitives drawing (Textures)

        /**
         * @brief Draws a textured quad.
         * 
         * @param position The 2D position (x, y) of the quad center.
         * @param size The size (width, height) of the quad.
         * @param texture The texture to apply to the quad.
         */
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);

        /**
         * @brief Draws a textured quad at a specific depth.
         * 
         * @param position The 3D position (x, y, z) of the quad center.
         * @param size The size (width, height) of the quad.
         * @param texture The texture to apply to the quad.
         */
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);

        /**
         * @brief Draws a textured quad with a generic transform matrix.
         * 
         * @param transform The transformation matrix.
         * @param texture The texture to apply.
         * @param tilingFactor The tiling factor for the texture coordinates (default 1.0).
         * @param tintColor The color to tint the texture with (default white).
         */
        static void DrawQuad(const glm::mat4& transform, const std::shared_ptr<Texture2D>& texture, float tilingFactor = 1.0f, const glm::vec4& tintColor = glm::vec4(1.0f));

        /**
         * @brief Resets the internal rendering statistics.
         */
        static void ResetStats();

        /**
         * @brief Gets the current rendering statistics.
         * 
         * @return A copy of the statistics structure.
         */
        static Statistics GetStats();

    private:
        /**
         * @brief Starts a new batch.
         * 
         * Resets pointers and indices for the next batch.
         */
        static void StartBatch();

        /**
         * @brief Updates the queue for the next flush.
         * 
         * Usually called when texture slots are full or buffer is filled.
         */
        static void NextBatch();
    };

}