#pragma once

#include "Core/Core.hpp"
#include <glm/glm.hpp>
#include <memory>
#include "VertexArray.hpp"

namespace Voltra {

    /**
     * @brief Abstract base class for the Renderer API.
     * 
     * Defines platform-agnostic rendering commands.
     */
    class VOLTRA_API RendererAPI {
    public:
        /**
         * @brief Supported graphics APIs.
         */
        enum class API {
            None = 0, OpenGL = 1
        };

    public:
        /**
         * @brief Virtual destructor.
         */
        virtual ~RendererAPI() = default;

        /**
         * @brief Sets the clear color.
         * 
         * @param color RGBA color.
         */
        virtual void SetClearColor(const glm::vec4& color) = 0;

        /**
         * @brief Clears the framebuffer.
         */
        virtual void Clear() = 0;

        /**
         * @brief Performs an indexed draw call.
         * 
         * @param vertexArray The vertex array to render.
         * @param indexCount Optional index count (0 to use entire IB).
         */
        virtual void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray, uint32_t indexCount = 0) = 0;

        /**
         * @brief Gets the current API.
         * 
         * @return The API enum value.
         */
        static API GetAPI() { return s_API; }

    private:
        static API s_API;
    };

}