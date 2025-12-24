#pragma once

#include "RendererAPI.hpp"

namespace Voltra {

    class RenderCommand {
    public:
        static void SetClearColor(const glm::vec4& color) {
            s_RendererAPI->SetClearColor(color);
        }

        static void Clear() {
            s_RendererAPI->Clear();
        }

        static void DrawIndexed(const std::shared_ptr<VertexArray>& vertexArray) {
            s_RendererAPI->DrawIndexed(vertexArray);
        }

    private:
        static std::unique_ptr<RendererAPI> s_RendererAPI;
    };

}