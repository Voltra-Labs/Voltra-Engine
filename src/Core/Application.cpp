#include "Application.hpp"
#include "Core/Log.hpp"
#include "Renderer/Renderer.hpp"
#include "Renderer/OrthographicCamera.hpp"

namespace Voltra {

    Application::Application()
        : m_Camera(-1.6f, 1.6f, -0.9f, 0.9f) // Aspect Ratio 16:9 
    {
        m_Window = std::make_unique<Window>(Window::Properties("Voltra Engine", 1280, 720));
        m_Window->SetEventCallback(std::bind(&Application::OnEvent, this, std::placeholders::_1));

        m_VertexArray = VertexArray::Create();

        float vertices[3 * 3] = {
            -0.5f, -0.5f, 0.0f,
             0.5f, -0.5f, 0.0f,
             0.0f,  0.5f, 0.0f
        };

        // Stack allocation for buffer creation is fine as ownership is passed to shared_ptr internally
        std::shared_ptr<VertexBuffer> vertex_buffer = VertexBuffer::Create(vertices, sizeof(vertices));
        
        BufferLayout layout = {
            { ShaderDataType::Float3, "a_Position" }
        };
        
        vertex_buffer->SetLayout(layout);
        m_VertexArray->AddVertexBuffer(vertex_buffer);

        uint32_t indices[3] = { 0, 1, 2 };
        std::shared_ptr<IndexBuffer> index_buffer = IndexBuffer::Create(indices, sizeof(indices) / sizeof(uint32_t));
        m_VertexArray->SetIndexBuffer(index_buffer);

        std::string vertex_src = R"(
            #version 330 core
            
            layout(location = 0) in vec3 a_Position;

            uniform mat4 u_ViewProjection;
            uniform mat4 u_Transform;

            out vec3 v_Position;

            void main() {
                v_Position = a_Position;
                // Multiplicación: Proyección * Vista * Modelo * Vértice
                gl_Position = u_ViewProjection * u_Transform * vec4(a_Position, 1.0);
            }
        )";

        std::string fragment_src = R"(
            #version 330 core
            layout(location = 0) out vec4 color;
            in vec3 v_Position;
            void main() {
                color = vec4(0.2, 0.8, 0.3, 1.0);
            }
        )";

        m_Shader = std::make_shared<Shader>(vertex_src, fragment_src);
    }

    Application::~Application() {
    }

    void Application::OnEvent(Event& e) {
        if (e.GetEventType() == EventType::WindowClose) {
            // Simplified handling
            m_Running = false;
        }
        VOLTRA_CORE_TRACE("{0}", e.ToString());
    }

    void Application::Run() {
        while (m_Running) {
            RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1.0f });
            RenderCommand::Clear();

            Renderer::BeginScene(m_Camera);

            Renderer::Submit(m_VertexArray, m_Shader);

            Renderer::EndScene();

            m_Window->OnUpdate();
        }
    }
}