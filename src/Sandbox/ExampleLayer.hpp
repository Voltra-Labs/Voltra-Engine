#pragma once

#include "Core/Layer.hpp"
#include "Renderer/Shader.hpp"
#include "Renderer/VertexArray.hpp"
#include "Renderer/OrthographicCamera.hpp"

namespace Voltra {

    class ExampleLayer : public Layer {
    public:
        ExampleLayer();
        virtual ~ExampleLayer() = default;

        virtual void OnAttach() override;
        virtual void OnDetach() override;
        virtual void OnUpdate(Timestep ts) override;
        virtual void OnEvent(Event& event) override;

    private:
        std::shared_ptr<Shader> m_Shader;
        std::shared_ptr<VertexArray> m_VertexArray;
        OrthographicCamera m_Camera;
        
        glm::vec3 m_CameraPosition;
        float m_CameraSpeed = 2.0f;
    };

}