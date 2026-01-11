#pragma once

#include "Core/Core.hpp"
#include "RendererAPI.hpp"

namespace Voltra {

    /**
     * @brief Static interface for issuing rendering commands.
     * 
     * Hides the underlying RendererAPI usage.
     */
    class VOLTRA_API RenderCommand {
    public:
        /**
         * @brief Sets the clear color for the screen.
         * 
         * @param color The RGBA color to clear to.
         */
        static void SetClearColor(const glm::vec4& color) {
            s_RendererAPI->SetClearColor(color);
        }

        /**
         * @brief Clears the color and depth buffers.
         */
        static void Clear() {
            s_RendererAPI->Clear();
        }

        /**
         * @brief Issues an indexed draw call.
         * 
         * @param vertexArray The vertex array containing geometry.
         * @param indexCount Optional index count to draw.
         */
        static void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray, uint32_t indexCount = 0) {
            s_RendererAPI->DrawIndexed(vertexArray, indexCount);
        }

    private:
        static std::unique_ptr<RendererAPI> s_RendererAPI;
    };

}