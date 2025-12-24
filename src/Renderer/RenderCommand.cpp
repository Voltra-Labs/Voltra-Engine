#include "RenderCommand.hpp"

// Forward declaration of the OpenGL implementation defined in another translation unit
// Or simpler: We can move the OpenGLRendererAPI definition to its own header later.
// For now, to make it compile with the structure provided:

#include <glad/glad.h>

namespace Voltra {

    // Quick OpenGL implementation inside here for simplicity until we create src/Platform/OpenGL
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

    std::unique_ptr<RendererAPI> RenderCommand::s_RendererAPI = std::make_unique<OpenGLRendererAPI>();
}