#include "Renderer.hpp"
#include "TextureManager.hpp"
#include "Core/Log.hpp"

namespace Voltra {

    std::unique_ptr<Renderer::SceneData> Renderer::s_SceneData = std::make_unique<Renderer::SceneData>();

    /**
     * @brief Initializes the renderer and its subsystems.
     */
    void Renderer::Init() {
        TextureManager::Init();
    }

    /**
     * @brief Initializes the renderer's GLAD instance from an external loader.
     * 
     * @param loader The OpenGL loader function.
     */
    void Renderer::InitExternal(void* loader) {
        if (!gladLoadGLLoader((GLADloadproc)loader)) {
            VOLTRA_CORE_ERROR("Failed to initialize GLAD in Core DLL!");
        } else {
            VOLTRA_CORE_INFO("GLAD initialized in Core DLL.");
        }
        // Also initialize internal subsystems if needed
        Init(); 
    }

    /**
     * @brief Sets up the scene environment (VP matrix).
     * 
     * @param camera The camera for the scene.
     */
    void Renderer::BeginScene(OrthographicCamera& camera) {
        s_SceneData->ViewProjectionMatrix = camera.GetViewProjectionMatrix();
    }

    /**
     * @brief Finalizes the scene frame.
     */
    void Renderer::EndScene() {
    }

    /**
     * @brief Processes a submission request.
     * 
     * Binds the shader, updates uniforms, binds the VAO, and issues the draw call.
     * 
     * @param vertexArray Geometry.
     * @param shader Shader program.
     * @param transform Model matrix.
     */
    void Renderer::Submit(const std::shared_ptr<VertexArray>& vertexArray, const std::shared_ptr<Shader>& shader, const glm::mat4& transform) {
        shader->Bind();
        
        shader->UploadUniformMat4("u_ViewProjection", s_SceneData->ViewProjectionMatrix);
        shader->UploadUniformMat4("u_Transform", transform);

        vertexArray->Bind();
        RenderCommand::DrawIndexed(vertexArray);
    }

    /**
     * @brief Shuts down the renderer and frees resources.
     */
    void Renderer::Shutdown() {
        TextureManager::Clean();
    }
}