#include "EditorLayer.hpp"
#include "EditorLayer.hpp"
#include <imgui.h>
#include <imgui_internal.h>
#include <glm/gtc/type_ptr.hpp>
#include <glm/gtc/matrix_transform.hpp>

#include "Core/Application.hpp"

#include "Scene/Components.hpp"
#include "Renderer/RenderCommand.hpp"

namespace Voltra {

    EditorLayer::EditorLayer() : Layer("EditorLayer") {}

    void EditorLayer::OnAttach() {
        m_ActiveScene = std::make_shared<Scene>();

        FramebufferSpecification fbSpec;
        fbSpec.Width = 1280;
        fbSpec.Height = 720;
        m_Framebuffer = Framebuffer::Create(fbSpec);

        // Camera
        auto cameraEntity = m_ActiveScene->CreateEntity("Camera");
        auto& cc = cameraEntity.AddComponent<CameraComponent>();
        cc.Camera.SetProjection(-8.0f, 8.0f, -4.5f, 4.5f);

        // Ground (Green)
        auto ground = m_ActiveScene->CreateEntity("Ground");
        auto& groundTransform = ground.GetComponent<TransformComponent>();
        groundTransform.Translation = { 0.0f, -3.0f, 0.0f };
        groundTransform.Scale = { 3.0f, 1.0f, 1.0f };
        ground.AddComponent<SpriteRendererComponent>(glm::vec4{ 0.0f, 1.0f, 0.0f, 1.0f });
        
        auto& groundRb = ground.AddComponent<Rigidbody2DComponent>();
        groundRb.Type = Rigidbody2DComponent::BodyType::Static;
        ground.AddComponent<BoxCollider2DComponent>();

        // Box (Red)
        m_SquareEntity = m_ActiveScene->CreateEntity("Box");
        auto& boxTransform = m_SquareEntity.GetComponent<TransformComponent>();
        boxTransform.Translation = { 0.0f, 3.0f, 0.0f };
        boxTransform.Rotation = { 0.0f, 0.0f, glm::radians(10.0f) };
        m_SquareEntity.AddComponent<SpriteRendererComponent>(glm::vec4{ 1.0f, 0.0f, 0.0f, 1.0f });
        
        auto& boxRb = m_SquareEntity.AddComponent<Rigidbody2DComponent>();
        boxRb.Type = Rigidbody2DComponent::BodyType::Dynamic;
        
        auto& boxBc = m_SquareEntity.AddComponent<BoxCollider2DComponent>();
        boxBc.Density = 1.0f;
        boxBc.Friction = 0.5f;

        m_SceneHierarchyPanel.SetContext(m_ActiveScene);

        m_ActiveScene->OnRuntimeStart();
    }

    void EditorLayer::OnDetach() {
        if (m_ActiveScene)
            m_ActiveScene->OnRuntimeStop();
    }

    void EditorLayer::OnUpdate(Timestep ts) {
        // Resize logic if framebuffer size != viewport size
        if (m_ViewportSize.x > 0.0f && m_ViewportSize.y > 0.0f && // zero sized framebuffer is invalid
            (m_Framebuffer->GetSpecification().Width != m_ViewportSize.x || m_Framebuffer->GetSpecification().Height != m_ViewportSize.y))
        {
            m_Framebuffer->Resize((uint32_t)m_ViewportSize.x, (uint32_t)m_ViewportSize.y);
            
            // Update camera projection
            auto cameraEntity = m_ActiveScene->GetPrimaryCameraEntity();
            if (cameraEntity)
            {
                auto& cc = cameraEntity.GetComponent<CameraComponent>();
                float aspectRatio = m_ViewportSize.x / m_ViewportSize.y;
                float orthoSize = 9.0f; // Hardcoded default size for now
                cc.Camera.SetProjection(-orthoSize * aspectRatio * 0.5f, orthoSize * aspectRatio * 0.5f, -orthoSize * 0.5f, orthoSize * 0.5f);
            }
        }

        m_Framebuffer->Bind();
        RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1 });
        RenderCommand::Clear();

        if (m_ActiveScene)
            m_ActiveScene->OnUpdate(ts);
        
        m_Framebuffer->Unbind();
    }

    void EditorLayer::OnImGuiRender() {
        static bool dockspaceOpen = true;
        static bool opt_fullscreen = true;
        static bool opt_padding = false;
        static ImGuiDockNodeFlags dockspace_flags = ImGuiDockNodeFlags_None;

        ImGuiWindowFlags window_flags = ImGuiWindowFlags_MenuBar | ImGuiWindowFlags_NoDocking;
        if (opt_fullscreen)
        {
            const ImGuiViewport* viewport = ImGui::GetMainViewport();
            ImGui::SetNextWindowPos(viewport->WorkPos);
            ImGui::SetNextWindowSize(viewport->WorkSize);
            ImGui::SetNextWindowViewport(viewport->ID);
            ImGui::PushStyleVar(ImGuiStyleVar_WindowRounding, 0.0f);
            ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
            window_flags |= ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoCollapse | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoMove;
            window_flags |= ImGuiWindowFlags_NoBringToFrontOnFocus | ImGuiWindowFlags_NoNavFocus;
        }

        if (dockspace_flags & ImGuiDockNodeFlags_PassthruCentralNode)
            window_flags |= ImGuiWindowFlags_NoBackground;

        if (!opt_padding)
            ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0.0f, 0.0f));
        
        ImGui::Begin("DockSpace Demo", &dockspaceOpen, window_flags);

        if (!opt_padding)
            ImGui::PopStyleVar();

        if (opt_fullscreen)
            ImGui::PopStyleVar(2);

        // DockSpace
        ImGuiIO& io = ImGui::GetIO();
        if (io.ConfigFlags & ImGuiConfigFlags_DockingEnable)
        {
            ImGuiID dockspace_id = ImGui::GetID("MyDockSpace");
            ImGui::DockSpace(dockspace_id, ImVec2(0.0f, 0.0f), dockspace_flags);

            static bool first_time = true;
            if (first_time)
            {
                first_time = false;

                ImGui::DockBuilderRemoveNode(dockspace_id); // clear any previous layout
                ImGui::DockBuilderAddNode(dockspace_id, dockspace_flags | ImGuiDockNodeFlags_DockSpace);
                ImGui::DockBuilderSetNodeSize(dockspace_id, ImGui::GetMainViewport()->Size);

                ImGuiID dock_main_id = dockspace_id;
                ImGuiID dock_id_right = ImGui::DockBuilderSplitNode(dock_main_id, ImGuiDir_Right, 0.2f, nullptr, &dock_main_id);
                ImGuiID dock_id_left = ImGui::DockBuilderSplitNode(dock_main_id, ImGuiDir_Left, 0.2f, nullptr, &dock_main_id);
                
                ImGui::DockBuilderDockWindow("Viewport", dock_main_id);
                ImGui::DockBuilderDockWindow("Properties", dock_id_right);
                ImGui::DockBuilderDockWindow("Scene Hierarchy", dock_id_left);
                ImGui::DockBuilderFinish(dockspace_id);
            }
        }

        // Viewport
        ImGui::Begin("Viewport");
        
        ImVec2 viewportPanelSize = ImGui::GetContentRegionAvail();
        m_ViewportSize = { viewportPanelSize.x, viewportPanelSize.y };

        uint32_t textureID = m_Framebuffer->GetColorAttachmentRendererID();
        ImGui::Image((void*)(uintptr_t)textureID, ImVec2{ m_ViewportSize.x, m_ViewportSize.y }, ImVec2{ 0, 1 }, ImVec2{ 1, 0 });
        
        ImGui::End(); // Viewport

        m_SceneHierarchyPanel.OnImGuiRender();

        ImGui::End(); // DockSpace
    }

    void EditorLayer::OnEvent(Event& e) {
    }
}