#pragma once

#include "Voltra.hpp"

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include "ImGui/SceneHierarchyPanel.hpp"
#include "Renderer/Framebuffer.hpp"
#include "Renderer/EditorCamera.hpp"
#include <filesystem>


namespace Voltra {

    class EditorLayer : public Layer {
    public:
        EditorLayer();
        virtual ~EditorLayer() = default;

        virtual void OnAttach() override;
        virtual void OnDetach() override;
        virtual void OnUpdate(Timestep ts) override;
        virtual void OnImGuiRender() override;
        virtual void OnEvent(Event& e) override;

    private:
        void OnScenePlay();
        void OnSceneStop();

    private:
        enum class SceneState { Edit = 0, Play = 1 };
        SceneState m_SceneState = SceneState::Edit;

        std::shared_ptr<Scene> m_ActiveScene;
        std::shared_ptr<Scene> m_EditorScene;
        EditorCamera m_EditorCamera;

        Entity m_SquareEntity;
        glm::vec4 m_SquareColor = { 0.2f, 0.3f, 0.8f, 1.0f };
        
        std::shared_ptr<Framebuffer> m_Framebuffer;
        glm::vec2 m_ViewportSize = { 0.0f, 0.0f };
        bool m_ViewportFocused = false;
        bool m_ViewportHovered = false;


        int m_GizmoType = 7; // ImGuizmo::OPERATION::TRANSLATE

        SceneHierarchyPanel m_SceneHierarchyPanel;
    };

}