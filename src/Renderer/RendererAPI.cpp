#include "RendererAPI.hpp"
#include <glad/glad.h>

namespace Voltra {

    RendererAPI::API RendererAPI::s_API = RendererAPI::API::OpenGL;

    // --- OpenGL Implementation ---
    class OpenGLRendererAPI : public RendererAPI {
    public:
        void SetClearColor(const glm::vec4& color) override {
            glClearColor(color.r, color.g, color.b, color.a);
        }

        void Clear() override {
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        }

        void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray) override {
            glDrawElements(GL_TRIANGLES, vertexArray->GetIndexBuffer()->GetCount(), GL_UNSIGNED_INT, nullptr);
        }
    };
}