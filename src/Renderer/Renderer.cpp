#include "Renderer.hpp"

namespace Voltra {

    void Renderer::BeginScene() {
        // TODO: Take Camera and Environment data here
    }

    void Renderer::EndScene() {
    }

    void Renderer::Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader) {
        shader->Bind();
        vertexArray->Bind();
        RenderCommand::DrawIndexed(vertexArray);
    }

}