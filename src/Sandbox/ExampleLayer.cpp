#include "ExampleLayer.hpp"
#include "Renderer/Renderer.hpp"
#include "Renderer/RenderCommand.hpp"
#include "Core/Input.hpp"
#include "Core/Log.hpp"

#include <GLFW/glfw3.h> 

namespace Voltra {

    ExampleLayer::ExampleLayer()
        : Layer("Example"), m_Camera(-1.6f, 1.6f, -0.9f, 0.9f), m_CameraPosition(0.0f)
    {
    }

    void ExampleLayer::OnAttach() {
        VOLTRA_TRACE("ExampleLayer::OnAttach");

        // Configure geometry (Triangle)
        m_VertexArray = VertexArray::Create();

        float vertices[3 * 3] = {
            -0.5f, -0.5f, 0.0f,
             0.5f, -0.5f, 0.0f,
             0.0f,  0.5f, 0.0f
        };

        std::shared_ptr<VertexBuffer> vertexBuffer = VertexBuffer::Create(vertices, sizeof(vertices));
        
        BufferLayout layout = {
            { ShaderDataType::Float3, "a_Position" }
        };
        
        vertexBuffer->SetLayout(layout);
        m_VertexArray->AddVertexBuffer(vertexBuffer);

        uint32_t indices[3] = { 0, 1, 2 };
        std::shared_ptr<IndexBuffer> indexBuffer = IndexBuffer::Create(indices, sizeof(indices) / sizeof(uint32_t));
        m_VertexArray->SetIndexBuffer(indexBuffer);

        // Configure Shader
        std::string vertexSrc = R"(
            #version 330 core
            layout(location = 0) in vec3 a_Position;
            uniform mat4 u_ViewProjection;
            uniform mat4 u_Transform;
            out vec3 v_Position;
            void main() {
                v_Position = a_Position;
                gl_Position = u_ViewProjection * u_Transform * vec4(a_Position, 1.0);
            }
        )";

        std::string fragmentSrc = R"(
            #version 330 core
            layout(location = 0) out vec4 color;
            in vec3 v_Position;
            void main() {
                color = vec4(0.2, 0.8, 0.3, 1.0);
            }
        )";

        m_Shader = std::make_shared<Shader>(vertexSrc, fragmentSrc);
    }

    void ExampleLayer::OnDetach() {
        VOLTRA_TRACE("ExampleLayer::OnDetach");
    }

    void ExampleLayer::OnUpdate(Timestep ts) {
        
        // Movement of camera using DeltaTime (ts)
        if (Input::IsKeyPressed(GLFW_KEY_A)) // Left
            m_CameraPosition.x -= m_CameraSpeed * ts;
        else if (Input::IsKeyPressed(GLFW_KEY_D)) // Right
            m_CameraPosition.x += m_CameraSpeed * ts;

        if (Input::IsKeyPressed(GLFW_KEY_W)) // Up
            m_CameraPosition.y += m_CameraSpeed * ts;
        else if (Input::IsKeyPressed(GLFW_KEY_S)) // Down
            m_CameraPosition.y -= m_CameraSpeed * ts;

        m_Camera.SetPosition(m_CameraPosition);

        // Render logic

        Renderer::BeginScene(m_Camera);
        Renderer::Submit(m_VertexArray, m_Shader);
        Renderer::EndScene();
    }

    void ExampleLayer::OnEvent(Event& event) {
        // if (event.GetEventType() == EventType::KeyPressed) { ... }
    }
}