#pragma once

#include "RenderCommand.hpp"
#include "Shader.hpp"

namespace Voltra {

    class Renderer {
    public:
        static void BeginScene();
        static void EndScene();

        static void Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader);
    };

}