#include "Application.hpp"
#include <GLFW/glfw3.h>
#include <glad/glad.h>

#include "Events/ApplicationEvent.hpp"
#include "Renderer/VertexArray.hpp"
#include "Renderer/Shader.hpp"
#include "Core/Log.hpp"

namespace Voltra {

    Application::Application() {
        m_Window = std::make_unique<Window>();
        m_Window->SetEventCallback(std::bind(&Application::OnEvent, this, std::placeholders::_1));

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

        std::string vertexSrc = R"(
            #version 330 core
            
            layout(location = 0) in vec3 a_Position;
            
            out vec3 v_Position;
            
            void main()
            {
                v_Position = a_Position;
                gl_Position = vec4(a_Position, 1.0);
            }
        )";

        std::string fragmentSrc = R"(
            #version 330 core
            
            layout(location = 0) out vec4 color;
            
            in vec3 v_Position;
            
            void main()
            {
                color = vec4(1.0, 0.5, 0.2, 1.0);
            }
        )";

        m_Shader = std::make_shared<Shader>(vertexSrc, fragmentSrc);
    }

    Application::~Application() {
    }

    void Application::OnEvent(Event& e)
    {
        if (e.GetEventType() == EventType::WindowClose)
        {
            WindowCloseEvent& event = (WindowCloseEvent&)e;
            m_Running = false;
        }
        
        VOLTRA_CORE_TRACE("{0}", e.ToString());
    }

    void Application::Run() {
        while (m_Running) {
            glClearColor(0.2f, 0.2f, 0.2f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT);

            if (m_Shader) m_Shader->Bind();
            if (m_VertexArray) {
                m_VertexArray->Bind();
                glDrawElements(GL_TRIANGLES, m_VertexArray->GetIndexBuffer()->GetCount(), GL_UNSIGNED_INT, nullptr);
            }

            m_Window->OnUpdate();
        }
    }

}