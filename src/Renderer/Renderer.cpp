#include "Renderer.hpp"

namespace Voltra {

    std::unique_ptr<Renderer::SceneData> Renderer::s_SceneData = std::make_unique<Renderer::SceneData>();

    void Renderer::Init() {
    }

    void Renderer::BeginScene(OrthographicCamera& camera) {
        s_SceneData->ViewProjectionMatrix = camera.GetViewProjectionMatrix();
    }

    void Renderer::EndScene() {
    }

    void Renderer::Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader, const glm::mat4& transform) {
        shader->Bind();
        
        shader->UploadUniformMat4("u_ViewProjection", s_SceneData->ViewProjectionMatrix);
        shader->UploadUniformMat4("u_Transform", transform);

        vertexArray->Bind();
        RenderCommand::DrawIndexed(vertexArray);
    }

}