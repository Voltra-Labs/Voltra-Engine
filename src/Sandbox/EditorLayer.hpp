#pragma once

#include "Voltra.hpp"

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include "ImGui/SceneHierarchyPanel.hpp"
#include "Renderer/Framebuffer.hpp"
#include "Renderer/EditorCamera.hpp"
#include <filesystem>


namespace Voltra {

    /**
     * @brief The main editor layer containing the docking system and panels.
     */
    class EditorLayer : public Layer {
    public:
        /**
         * @brief Constructs the Editor Layer.
         */
        EditorLayer();
        virtual ~EditorLayer() = default;

        /**
         * @brief Called when the layer is attached to the application.
         */
        virtual void OnAttach() override;

        /**
         * @brief Called when the layer is detached.
         */
        virtual void OnDetach() override;

        /**
         * @brief Updates the editor state.
         * 
         * @param ts Time step.
         */
        virtual void OnUpdate(Timestep ts) override;

        /**
         * @brief Renders the ImGui interface.
         */
        virtual void OnImGuiRender() override;

        /**
         * @brief Handles events.
         * 
         * @param e The event.
         */
        virtual void OnEvent(Event& e) override;

    private:
        /**
         * @brief Starts the scene runtime.
         */
        void OnScenePlay();

        /**
         * @brief Stops the scene runtime.
         */
        void OnSceneStop();

    private:
        /**
         * @brief State of the scene (Edit or Play).
         */
        enum class SceneState { Edit = 0, Play = 1 };
        SceneState m_SceneState = SceneState::Edit;

        std::shared_ptr<Scene> m_ActiveScene;
        std::shared_ptr<Scene> m_EditorScene;
        EditorCamera m_EditorCamera;

        Entity m_SquareEntity;
        glm::vec4 m_SquareColor = { 0.2f, 0.3f, 0.8f, 1.0f };
        
        std::shared_ptr<Framebuffer> m_Framebuffer;
        std::shared_ptr<Framebuffer> m_GameFramebuffer;
        glm::vec2 m_ViewportSize = { 0.0f, 0.0f };
        glm::vec2 m_GameViewportSize = { 0.0f, 0.0f };
        bool m_ViewportFocused = false;
        bool m_ViewportHovered = false;


        int m_GizmoType = 7;

        SceneHierarchyPanel m_SceneHierarchyPanel;
    };

}