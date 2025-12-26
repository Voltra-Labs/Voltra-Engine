#include "EditorLayer.hpp"
#include <imgui.h>
#include <glm/gtc/type_ptr.hpp>
#include <glm/gtc/matrix_transform.hpp>

#include "Scene/Components.hpp"
#include "Renderer/RenderCommand.hpp"

namespace Voltra {

    EditorLayer::EditorLayer() : Layer("EditorLayer") {}

    void EditorLayer::OnAttach() {
        m_ActiveScene = std::make_shared<Scene>();

        // Camera
        auto cameraEntity = m_ActiveScene->CreateEntity("Camera");
        auto& cc = cameraEntity.AddComponent<CameraComponent>();
        cc.Camera.SetProjection(-8.0f, 8.0f, -4.5f, 4.5f);

        // Ground (Green)
        auto ground = m_ActiveScene->CreateEntity("Ground");
        auto& groundTransform = ground.GetComponent<TransformComponent>();
        groundTransform.Transform = glm::translate(glm::mat4(1.0f), { 0.0f, -3.0f, 0.0f }) 
            * glm::scale(glm::mat4(1.0f), { 3.0f, 1.0f, 1.0f });
        ground.AddComponent<SpriteRendererComponent>(glm::vec4{ 0.0f, 1.0f, 0.0f, 1.0f });
        
        auto& groundRb = ground.AddComponent<Rigidbody2DComponent>();
        groundRb.Type = Rigidbody2DComponent::BodyType::Static;
        ground.AddComponent<BoxCollider2DComponent>();

        // Box (Red)
        m_SquareEntity = m_ActiveScene->CreateEntity("Box");
        auto& boxTransform = m_SquareEntity.GetComponent<TransformComponent>();
        boxTransform.Transform = glm::translate(glm::mat4(1.0f), { 0.0f, 3.0f, 0.0f })
            * glm::rotate(glm::mat4(1.0f), glm::radians(10.0f), { 0.0f, 0.0f, 1.0f });
        m_SquareEntity.AddComponent<SpriteRendererComponent>(glm::vec4{ 1.0f, 0.0f, 0.0f, 1.0f });
        
        auto& boxRb = m_SquareEntity.AddComponent<Rigidbody2DComponent>();
        boxRb.Type = Rigidbody2DComponent::BodyType::Dynamic;
        
        auto& boxBc = m_SquareEntity.AddComponent<BoxCollider2DComponent>();
        boxBc.Density = 1.0f;
        boxBc.Friction = 0.5f;

        m_ActiveScene->OnRuntimeStart();
    }

    void EditorLayer::OnDetach() {
        if (m_ActiveScene)
            m_ActiveScene->OnRuntimeStop();
    }

    void EditorLayer::OnUpdate(Timestep ts) {
        RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1 });
        RenderCommand::Clear();

        if (m_ActiveScene)
            m_ActiveScene->OnUpdate(ts);
    }

    void EditorLayer::OnImGuiRender() {
        ImGui::Begin("Settings");
        
        if (m_SquareEntity) {
            ImGui::Separator();
            auto& src = m_SquareEntity.GetComponent<SpriteRendererComponent>();
            ImGui::ColorEdit4("Box Color", glm::value_ptr(src.Color));
            ImGui::Separator();
        }

        if (ImGui::Button("Reset Physics")) {
            if (m_ActiveScene)
                m_ActiveScene->OnRuntimeStop();
            OnAttach(); // Re-initialize scene
        }

        ImGui::End();
    }

    void EditorLayer::OnEvent(Event& e) {
    }
}