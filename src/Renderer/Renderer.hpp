#pragma once

#include "RenderCommand.hpp"
#include "OrthographicCamera.hpp"
#include "Shader.hpp"

namespace Voltra {

    class Renderer {
    public:
        static void Init();
        
        static void BeginScene(OrthographicCamera& camera);
        static void EndScene();

        static void Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader, const glm::mat4& transform = glm::mat4(1.0f));

    private:
        struct SceneData {
            glm::mat4 ViewProjectionMatrix;
        };

        static std::unique_ptr<SceneData> s_SceneData;
    };

}