#include "EditorLayer.hpp"
#include <imgui.h>
#include <imgui_internal.h>
#include <glm/gtc/type_ptr.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <ImGuizmo.h>
#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>

#include "Core/Application.hpp"

#include "Scene/Components.hpp"
#include "Scene/SceneSerializer.hpp"
#include "Renderer/RenderCommand.hpp"
#include "Core/Input.hpp"
#include "Events/KeyEvent.hpp"

namespace Voltra {

    EditorLayer::EditorLayer() : Layer("EditorLayer"), m_EditorCamera(1.778f) {}


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
            m_EditorCamera.SetViewportSize(m_ViewportSize.x, m_ViewportSize.y);
            
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

        switch (m_SceneState)
        {
            case SceneState::Edit:
            {
                if (m_ViewportFocused)
                    m_EditorCamera.OnUpdate(ts);
                
                m_ActiveScene->OnUpdateEditor(ts, m_EditorCamera);
                break;
            }
            case SceneState::Play:
            {
                m_ActiveScene->OnUpdate(ts);
                break;
            }
        }
        
        m_Framebuffer->Unbind();
    }


    void EditorLayer::OnImGuiRender() {
        // Main Menu Bar
        if (ImGui::BeginMainMenuBar())
        {
            if (ImGui::BeginMenu("File")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Edit")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Assets")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("GameObject")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Component")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Services")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Jobs")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Window")) { ImGui::EndMenu(); }
            if (ImGui::BeginMenu("Help")) { ImGui::EndMenu(); }

            ImGui::EndMainMenuBar();
        }

        // UI Toolbar
        ImGuiWindowFlags toolbarFlags = ImGuiWindowFlags_NoDecoration | ImGuiWindowFlags_NoScrollbar | ImGuiWindowFlags_NoScrollWithMouse;
        
        ImGuiViewport* viewport = ImGui::GetMainViewport();
        ImGui::SetNextWindowPos(ImVec2(viewport->Pos.x, viewport->Pos.y + 24.0f)); // Assuming 24 height for main menu bar
        ImGui::SetNextWindowSize(ImVec2(viewport->Size.x, 32.0f)); // Fixed height for toolbar

        ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2(0, 2));
        ImGui::PushStyleVar(ImGuiStyleVar_WindowBorderSize, 0.0f);
        ImGui::Begin("##toolbar", nullptr, toolbarFlags);
        
        bool toolbarEnabled = (m_SceneState == SceneState::Edit);
        ImVec4 buttonColor = ImVec4(0.4f, 0.4f, 0.4f, 1.0f); // Standard Grey
        ImVec4 activeColor = ImVec4(0.3f, 0.3f, 0.3f, 1.0f); // Slightly darker for active/interaction

        float size = ImGui::GetWindowHeight() - 4.0f;
        ImGui::SetCursorPosX((ImGui::GetWindowContentRegionMax().x * 0.5f) - (size * 0.5f));



        if (m_SceneState == SceneState::Edit)
        {
            if (ImGui::Button("Play", ImVec2(80, size))) 
            {
                OnScenePlay();
            }
        }
        else
        {
            if (ImGui::Button("Stop", ImVec2(80, size))) 
            {
                OnSceneStop();
            }
        }
        
        ImGui::SameLine();
        if (ImGui::Button("Pause", ImVec2(80, size)))
        {
            // TODO: Toggle pause
        }

        ImGui::End();
        ImGui::PopStyleVar(2);

        ImGuizmo::BeginFrame();

        static bool dockspaceOpen = true;
        static bool opt_fullscreen = true;
        static bool opt_padding = false;
        static ImGuiDockNodeFlags dockspace_flags = ImGuiDockNodeFlags_None;

        ImGuiWindowFlags window_flags = ImGuiWindowFlags_MenuBar | ImGuiWindowFlags_NoDocking;
        if (opt_fullscreen)
        {
            const ImGuiViewport* viewport = ImGui::GetMainViewport();
            // Start below toolbar
            ImGui::SetNextWindowPos(ImVec2(viewport->Pos.x, viewport->Pos.y + 24.0f + 32.0f)); 
            ImGui::SetNextWindowSize(ImVec2(viewport->Size.x, viewport->Size.y - 24.0f - 32.0f));
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
                ImGuiID dock_id_right = ImGui::DockBuilderSplitNode(dock_main_id, ImGuiDir_Right, 0.25f, nullptr, &dock_main_id);
                ImGuiID dock_id_left = ImGui::DockBuilderSplitNode(dock_main_id, ImGuiDir_Left, 0.20f, nullptr, &dock_main_id);
                
                // Dock windows
                ImGui::DockBuilderDockWindow("Scene", dock_main_id);
                ImGui::DockBuilderDockWindow("Game", dock_main_id);
                ImGui::DockBuilderDockWindow("Inspector", dock_id_right);
                ImGui::DockBuilderDockWindow("Hierarchy", dock_id_left);
                ImGui::DockBuilderFinish(dockspace_id);
            }
        }

        // ==========================================
        // SCENE WINDOW (Edit Mode + Gizmos)
        // ==========================================
        ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{ 0, 0 });
        ImGui::Begin("Scene");
        
        m_ViewportFocused = ImGui::IsWindowFocused(); // We consider Scene window specifically for input
        m_ViewportHovered = ImGui::IsWindowHovered();
        
        ImVec2 viewportPanelSize = ImGui::GetContentRegionAvail();
        
        // Resize Framebuffer based on Scene window size
        m_ViewportSize = { viewportPanelSize.x, viewportPanelSize.y };

        uint32_t textureID = m_Framebuffer->GetColorAttachmentRendererID();
        ImGui::Image((void*)(uintptr_t)textureID, ImVec2{ m_ViewportSize.x, m_ViewportSize.y }, ImVec2{ 0, 1 }, ImVec2{ 1, 0 });

        // Gizmos (Only in active Scene window)
        Entity selectedEntity = m_SceneHierarchyPanel.GetSelectedEntity();
        if (selectedEntity && m_GizmoType != -1 && m_SceneState == SceneState::Edit) 
        {
            ImGuizmo::SetOrthographic(true);
            ImGuizmo::SetDrawlist();
            
            float windowWidth = (float)ImGui::GetWindowWidth();
            float windowHeight = (float)ImGui::GetWindowHeight();
            ImGuizmo::SetRect(ImGui::GetWindowPos().x, ImGui::GetWindowPos().y, windowWidth, windowHeight);

            // Camera
            const auto& camera = m_EditorCamera;
            const glm::mat4& cameraProjection = camera.GetProjectionMatrix();
            glm::mat4 cameraView = camera.GetViewMatrix();

            // Entity Transform
            auto& tc = selectedEntity.GetComponent<TransformComponent>();
            glm::mat4 transform = tc.GetTransform();

            // Snapping
            bool snap = Input::IsKeyPressed(GLFW_KEY_LEFT_CONTROL);
            float snapValue = 0.5f; 
            if (m_GizmoType == ImGuizmo::OPERATION::ROTATE)
                snapValue = 45.0f;

            float snapValues[3] = { snapValue, snapValue, snapValue };

            ImGuizmo::Manipulate(glm::value_ptr(cameraView), glm::value_ptr(cameraProjection),
                (ImGuizmo::OPERATION)m_GizmoType, ImGuizmo::LOCAL, glm::value_ptr(transform),
                nullptr, snap ? snapValues : nullptr);

            if (ImGuizmo::IsUsing())
            {
                glm::vec3 translation, rotation, scale;
                ImGuizmo::DecomposeMatrixToComponents(glm::value_ptr(transform), glm::value_ptr(translation), glm::value_ptr(rotation), glm::value_ptr(scale));
                
                glm::vec3 deltaRotation = rotation - glm::degrees(tc.Rotation);
                tc.Translation = translation;
                tc.Rotation += glm::radians(deltaRotation);
                tc.Scale = scale;
            }
        }
        ImGui::End(); // Scene
        ImGui::PopStyleVar();

        // ==========================================
        // GAME WINDOW (Play/Mockup View Only)
        // ==========================================
        ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{ 0, 0 });
        ImGui::Begin("Game");
        
        // Just render the framebuffer image. No Gizmos.
        ImVec2 gamePanelSize = ImGui::GetContentRegionAvail();
        
        // Note: For now we share the framebuffer and it's resized by the Scene window logic above.
        // We just draw it here. If the aspect ratios differ significantly, it might look stretched.
        // The user agreed to this compromise for now.
        ImGui::Image((void*)(uintptr_t)textureID, ImVec2{ gamePanelSize.x, gamePanelSize.y }, ImVec2{ 0, 1 }, ImVec2{ 1, 0 });
        
        ImGui::End(); // Game
        ImGui::PopStyleVar();
        
        // Input Handling for shortcuts (inside ImGui context)
        if (!ImGui::GetIO().WantTextInput) {
             if (ImGui::IsKeyPressed(ImGuiKey_Q)) m_GizmoType = -1;
             if (ImGui::IsKeyPressed(ImGuiKey_W)) m_GizmoType = ImGuizmo::OPERATION::TRANSLATE;
             if (ImGui::IsKeyPressed(ImGuiKey_E)) m_GizmoType = ImGuizmo::OPERATION::ROTATE;
             if (ImGui::IsKeyPressed(ImGuiKey_R)) m_GizmoType = ImGuizmo::OPERATION::SCALE;
        }

        // Scene Serialization
        bool control = Input::IsKeyPressed(GLFW_KEY_LEFT_CONTROL) || Input::IsKeyPressed(GLFW_KEY_RIGHT_CONTROL);
        if (control && Input::IsKeyPressed(GLFW_KEY_S)) {
            SceneSerializer serializer(m_ActiveScene);
            serializer.Serialize("../assets/scenes/Example.voltra");
            VOLTRA_INFO("Scene Saved to ../assets/scenes/Example.voltra");
        }
        if (control && Input::IsKeyPressed(GLFW_KEY_O)) {
             std::shared_ptr<Scene> newScene = std::make_shared<Scene>();
             SceneSerializer serializer(newScene);
             if (serializer.Deserialize("../assets/scenes/Example.voltra"))
             {
                 m_ActiveScene->OnRuntimeStop();
                 m_ActiveScene = newScene;
                 m_ActiveScene->OnRuntimeStart();
                 
                 m_SceneHierarchyPanel.SetContext(m_ActiveScene);
                 VOLTRA_INFO("Scene Loaded from ../assets/scenes/Example.voltra");
             }
        }

        m_SceneHierarchyPanel.OnImGuiRender();

        ImGui::End(); // DockSpace
    }

    void EditorLayer::OnEvent(Event& e) {
        if (m_ViewportHovered)
             m_EditorCamera.OnEvent(e);

        EventDispatcher dispatcher(e);


        dispatcher.Dispatch<KeyPressedEvent>([&](KeyPressedEvent& e) {
             // Shortcuts that don't depend on ImGui frame
             bool control = Input::IsKeyPressed(GLFW_KEY_LEFT_CONTROL) || Input::IsKeyPressed(GLFW_KEY_RIGHT_CONTROL);
             bool shift = Input::IsKeyPressed(GLFW_KEY_LEFT_SHIFT) || Input::IsKeyPressed(GLFW_KEY_RIGHT_SHIFT);

             switch (e.GetKeyCode())
             {
                 case GLFW_KEY_S:
                 {
                     if (control && shift) {
                         // Save As logic could go here
                     }
                     break;
                 }
                 case GLFW_KEY_D:
                 {
                     if (control) {
                         // Duplicate Entity Logic could go here
                     }
                     break;
                 }
             }
             return false;
        });
    }

    void EditorLayer::OnScenePlay() {
        m_SceneState = SceneState::Play;
        
        m_EditorScene = m_ActiveScene;
        m_ActiveScene = std::make_shared<Scene>();
        
        // Use Serializer
        try {
            SceneSerializer serializer(m_EditorScene);
            std::filesystem::path tempPath = "scenes/temp_physics.voltra";
            serializer.Serialize(tempPath.string());
            
            SceneSerializer newSerializer(m_ActiveScene);
            newSerializer.Deserialize(tempPath.string());
        } catch (...) {
            VOLTRA_ERROR("Scene copy for play mode failed!");
            m_SceneState = SceneState::Edit;
            m_ActiveScene = m_EditorScene;
            return;
        }

         m_ActiveScene->OnRuntimeStart();
         m_SceneHierarchyPanel.SetContext(m_ActiveScene);
    }

    void EditorLayer::OnSceneStop() {
        m_SceneState = SceneState::Edit;
        m_ActiveScene->OnRuntimeStop();
        
        m_ActiveScene = m_EditorScene;
        m_SceneHierarchyPanel.SetContext(m_ActiveScene);
        
        // Cleanup temp file
        try {
            std::filesystem::path tempPath = "scenes/temp_physics.voltra";
            if (std::filesystem::exists(tempPath))
                std::filesystem::remove(tempPath);
        } catch (...) {
            VOLTRA_ERROR("Failed to delete temp scene file!");
        }
    }
}
