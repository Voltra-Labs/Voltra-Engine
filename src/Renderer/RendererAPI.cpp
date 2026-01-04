#include "RendererAPI.hpp"
#include <glad/glad.h>

namespace Voltra {

    RendererAPI::API RendererAPI::s_API = RendererAPI::API::OpenGL;

    /**
     * @brief OpenGL implementation of RendererAPI.
     */
    class OpenGLRendererAPI : public RendererAPI {
    public:
        /**
         * @brief Sets the clear color.
         * 
         * @param color RGBA color.
         */
        void SetClearColor(const glm::vec4& color) override {
            glClearColor(color.r, color.g, color.b, color.a);
        }

        /**
         * @brief Clears the buffers.
         */
        void Clear() override {
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        }

        /**
         * @brief Draws indexed geometry.
         * 
         * @param vertexArray VAO to use.
         */
        void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray) override {
            glDrawElements(GL_TRIANGLES, vertexArray->GetIndexBuffer()->GetCount(), GL_UNSIGNED_INT, nullptr);
        }
    };
}