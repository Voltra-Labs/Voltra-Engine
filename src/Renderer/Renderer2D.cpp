#include "Renderer2D.hpp"

#include "VertexArray.hpp"
#include "Shader.hpp"
#include "RenderCommand.hpp"

#include <glm/gtc/matrix_transform.hpp>
#include <memory>

namespace Voltra {

    /**
     * @brief Internal storage for Renderer2D data.
     */
    struct Renderer2DStorage {
        std::shared_ptr<VertexArray> QuadVertexArray;
        std::shared_ptr<Shader> TextureShader;
        std::shared_ptr<Texture2D> WhiteTexture;
    };

    static Renderer2DStorage* s_Data;

    /**
     * @brief Initializes the 2D renderer.
     * 
     * Creates Vertex Arrays, Buffers, default Textures, and Shaders.
     */
    void Renderer2D::Init() {
        s_Data = new Renderer2DStorage();
        s_Data->QuadVertexArray = VertexArray::Create();

        float squareVertices[5 * 4] = {
            -0.5f, -0.5f, 0.0f, 0.0f, 0.0f,
             0.5f, -0.5f, 0.0f, 1.0f, 0.0f,
             0.5f,  0.5f, 0.0f, 1.0f, 1.0f,
            -0.5f,  0.5f, 0.0f, 0.0f, 1.0f
        };

        std::shared_ptr<VertexBuffer> squareVB = VertexBuffer::Create(squareVertices, sizeof(squareVertices));

        squareVB->SetLayout({
            { ShaderDataType::Float3, "a_Position" },
            { ShaderDataType::Float2, "a_TexCoord" }
        });
        s_Data->QuadVertexArray->AddVertexBuffer(squareVB);

        uint32_t squareIndices[6] = { 0, 1, 2, 2, 3, 0 };
        std::shared_ptr<IndexBuffer> squareIB = IndexBuffer::Create(squareIndices, sizeof(squareIndices) / sizeof(uint32_t));
        s_Data->QuadVertexArray->SetIndexBuffer(squareIB);

        s_Data->WhiteTexture = std::make_shared<Texture2D>(1, 1, TextureFilter::Nearest);
        uint32_t whiteTextureData = 0xffffffff;
        s_Data->WhiteTexture->SetData(&whiteTextureData, sizeof(uint32_t));
        std::string vertexSrc = R"(
            #version 330 core
            
            layout(location = 0) in vec3 a_Position;
            layout(location = 1) in vec2 a_TexCoord;
            
            uniform mat4 u_ViewProjection;
            uniform mat4 u_Transform;

            out vec2 v_TexCoord;
            
            void main() {
                v_TexCoord = a_TexCoord;
                gl_Position = u_ViewProjection * u_Transform * vec4(a_Position, 1.0);
            }
        )";

        std::string fragmentSrc = R"(
            #version 330 core
            
            layout(location = 0) out vec4 color;
            
            in vec2 v_TexCoord;
            
            uniform vec4 u_Color;
            uniform sampler2D u_Texture;
            
            void main() {
                color = texture(u_Texture, v_TexCoord) * u_Color;
            }
        )";

        s_Data->TextureShader = std::make_shared<Shader>(vertexSrc, fragmentSrc);
        s_Data->TextureShader->Bind();
        s_Data->TextureShader->UploadUniformInt("u_Texture", 0);
    }

    /**
     * @brief Shuts down the 2D renderer and frees memory.
     */
    void Renderer2D::Shutdown() {
        delete s_Data;
    }

    /**
     * @brief Prepares the scene for rendering.
     * 
     * Uploads the camera view-projection matrix to the shader.
     * 
     * @param camera The scene camera.
     */
    void Renderer2D::BeginScene(const OrthographicCamera& camera) {
        s_Data->TextureShader->Bind();
        s_Data->TextureShader->UploadUniformMat4("u_ViewProjection", camera.GetViewProjectionMatrix());
    }

    /**
     * @brief Finalizes the scene rendering (flushes batches).
     */
    void Renderer2D::EndScene() {
    }

    /**
     * @brief Draws a colored quad at 2D position.
     * 
     * @param position Position X, Y.
     * @param size Width and Height.
     * @param color Color.
     */
    void Renderer2D::DrawQuad(const glm::vec2& position, const glm::vec2& size, const glm::vec4& color) {
        DrawQuad({ position.x, position.y, 0.0f }, size, color);
    }

    /**
     * @brief Draws a colored quad at 3D position.
     * 
     * @param position Position X, Y, Z.
     * @param size Width and Height.
     * @param color Color.
     */
    void Renderer2D::DrawQuad(const glm::vec3& position, const glm::vec2& size, const glm::vec4& color) {
        glm::mat4 transform = glm::translate(glm::mat4(1.0f), position) * glm::scale(glm::mat4(1.0f), { size.x, size.y, 1.0f });
        DrawQuad(transform, color);
    }

    /**
     * @brief Draws a colored quad with transform matrix.
     * 
     * @param transform Model matrix.
     * @param color Color.
     */
    void Renderer2D::DrawQuad(const glm::mat4& transform, const glm::vec4& color) {
        s_Data->TextureShader->Bind();
        s_Data->TextureShader->UploadUniformFloat4("u_Color", color);
        s_Data->TextureShader->UploadUniformMat4("u_Transform", transform);

        s_Data->WhiteTexture->Bind(0);
        
        s_Data->QuadVertexArray->Bind();
        RenderCommand::DrawIndexed(s_Data->QuadVertexArray);
    }

    /**
     * @brief Draws a textured quad at 2D position.
     * 
     * @param position Position X, Y.
     * @param size Width and Height.
     * @param texture Texture to bind.
     */
    void Renderer2D::DrawQuad(const glm::vec2& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture) {
        DrawQuad({ position.x, position.y, 0.0f }, size, texture);
    }

    /**
     * @brief Draws a textured quad at 3D position.
     * 
     * @param position Position X, Y, Z.
     * @param size Width and Height.
     * @param texture Texture to bind.
     */
    void Renderer2D::DrawQuad(const glm::vec3& position, const glm::vec2& size, const std::shared_ptr<Texture2D>& texture) {
        glm::mat4 transform = glm::translate(glm::mat4(1.0f), position) * glm::scale(glm::mat4(1.0f), { size.x, size.y, 1.0f });
        DrawQuad(transform, texture);
    }

    /**
     * @brief Draws a textured quad with transform matrix.
     * 
     * @param transform Model matrix.
     * @param texture Texture to bind.
     */
    void Renderer2D::DrawQuad(const glm::mat4& transform, const std::shared_ptr<Texture2D>& texture) {
        s_Data->TextureShader->Bind();
        s_Data->TextureShader->UploadUniformFloat4("u_Color", glm::vec4(1.0f));
        s_Data->TextureShader->UploadUniformMat4("u_Transform", transform);

        texture->Bind(0);

        s_Data->QuadVertexArray->Bind();
        RenderCommand::DrawIndexed(s_Data->QuadVertexArray);
    }

}