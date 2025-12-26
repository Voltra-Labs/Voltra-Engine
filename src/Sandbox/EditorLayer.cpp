#include "EditorLayer.hpp"
#include <imgui.h>
#include <glm/gtc/type_ptr.hpp>

#include "Renderer/Renderer2D.hpp"
#include "Renderer/RenderCommand.hpp"
#include "Renderer/OrthographicCamera.hpp"

namespace Voltra {

    EditorLayer::EditorLayer() : Layer("EditorLayer") {}

    void EditorLayer::OnAttach() {
    }

    void EditorLayer::OnDetach() {
    }

    void EditorLayer::OnUpdate(Timestep ts) {
        RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1 });
        RenderCommand::Clear();

        Renderer2D::BeginScene(OrthographicCamera(-1.6f, 1.6f, -0.9f, 0.9f));
        
        Renderer2D::DrawQuad({ 0.0f, 0.0f }, { 1.0f, 1.0f }, m_SquareColor);
        
        Renderer2D::EndScene();
    }

    void EditorLayer::OnImGuiRender() {
        ImGui::Begin("Settings");
        ImGui::ColorEdit4("Square Color", glm::value_ptr(m_SquareColor));
        ImGui::End();
    }

    void EditorLayer::OnEvent(Event& e) {
    }
}