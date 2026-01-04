#pragma once

#include "RendererAPI.hpp"

namespace Voltra {

    /**
     * @brief Static interface for issuing rendering commands.
     * 
     * Hides the underlying RendererAPI usage.
     */
    class RenderCommand {
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
         */
        static void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray) {
            s_RendererAPI->DrawIndexed(vertexArray);
        }

    private:
        static std::unique_ptr<RendererAPI> s_RendererAPI;
    };

}