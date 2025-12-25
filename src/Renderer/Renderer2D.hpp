#pragma once

#include "OrthographicCamera.hpp"
#include "Texture.hpp"
#include <glm/glm.hpp>
#include <memory>

namespace Voltra {

    class Renderer2D {
    public:
        static void Init();
        static void Shutdown();

        static void BeginScene(const OrthographicCamera& camera);
        static void EndScene();

        // Primitives drawing (Colors)
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const glm::vec4& color);
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const glm::vec4& color);
        static void DrawQuad(const glm::mat4& transform, const glm::vec4& color);

        // Primitives drawing (Textures)
        static void DrawQuad(const glm::vec2& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);
        static void DrawQuad(const glm::vec3& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture);
        static void DrawQuad(const glm::mat4& transform, const std::shared_ptr<Texture2D>& texture);
    };

}