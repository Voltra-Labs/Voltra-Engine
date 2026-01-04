#pragma once

#include "RenderCommand.hpp"
#include "OrthographicCamera.hpp"
#include "Shader.hpp"

namespace Voltra {

    /**
     * @brief High-level renderer interface.
     * 
     * Manages scene execution and primitive submission.
     */
    class Renderer {
    public:
        /**
         * @brief Initializes the renderer.
         */
        static void Init();

        /**
         * @brief Shuts down the renderer.
         */
        static void Shutdown();
        
        /**
         * @brief Begins a new scene context.
         * 
         * @param camera The scene camera.
         */
        static void BeginScene(OrthographicCamera& camera);

        /**
         * @brief Ends the current scene context.
         */
        static void EndScene();

        /**
         * @brief Submits a primitive to be rendered.
         * 
         * @param vertexArray Geometry to render.
         * @param shader Shader to use.
         * @param transform Transformation matrix.
         */
        static void Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader, const glm::mat4& transform = glm::mat4(1.0f));

    private:
        /**
         * @brief Internal data for the current scene.
         */
        struct SceneData {
            glm::mat4 ViewProjectionMatrix;
        };

        static std::unique_ptr<SceneData> s_SceneData;
    };

}