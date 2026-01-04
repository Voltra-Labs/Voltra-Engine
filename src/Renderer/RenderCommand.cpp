#include "RenderCommand.hpp"

#include <glad/glad.h>

namespace Voltra {

    /**
     * @brief OpenGL specific implementation of the RendererAPI.
     */
    class OpenGLRendererAPI : public RendererAPI {
    public:
        /**
         * @brief Sets the OpenGL clear color.
         * 
         * @param color The clear color.
         */
        void SetClearColor(const glm::vec4& color) override {
            glClearColor(color.r, color.g, color.b, color.a);
        }
        /**
         * @brief Clears OpenGL buffers.
         */
        void Clear() override {
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        }
        /**
         * @brief Issues an OpenGL DrawElements call.
         * 
         * @param vertexArray The VAO to draw.
         */
        void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray) override {
            glDrawElements(GL_TRIANGLES, vertexArray->GetIndexBuffer()->GetCount(), GL_UNSIGNED_INT, nullptr);
        }
    };

    std::unique_ptr<RendererAPI> RenderCommand::s_RendererAPI = std::make_unique<OpenGLRendererAPI>();
}