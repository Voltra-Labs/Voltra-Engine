#pragma once

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include <memory>

namespace Voltra {

    /**
     * @brief Panel for displaying the scene hierarchy and entity properties.
     * 
     * Allows selection of entities and modification of their components.
     */
    class SceneHierarchyPanel {
    public:
        SceneHierarchyPanel() = default;
        /**
         * @brief Constructs the panel with an optional context.
         * 
         * @param context The scene context to display.
         */
        SceneHierarchyPanel(const std::shared_ptr<Scene>& context);

        /**
         * @brief Sets the scene context for the panel.
         * 
         * @param context The new scene context.
         */
        void SetContext(const std::shared_ptr<Scene>& context);

        /**
         * @brief Renders the ImGui UI for the panel.
         * 
         * Draws the hierarchy window and the inspector window.
         */
        void OnImGuiRender();

        /**
         * @brief Gets the currently selected entity.
         * 
         * @return The selected Entity.
         */
        Entity GetSelectedEntity() const { return m_SelectionContext; }

    private:
        /**
         * @brief Helper to draw a component's UI.
         * 
         * @tparam T The component type.
         * @tparam UIFunction The lambda type for drawing the UI.
         * @param name The display name of the component.
         * @param entity The entity owning the component.
         * @param uiFunction The function to call to draw the component's fields.
         */
        template<typename T, typename UIFunction>
        void DrawComponent(const std::string& name, Entity entity, UIFunction uiFunction);

        /**
         * @brief Draws a single entity node in the hierarchy tree.
         * 
         * @param entity The entity to draw.
         */
        void DrawEntityNode(Entity entity);

        /**
         * @brief Draws all components for a specific entity in the inspector.
         * 
         * @param entity The entity to inspect.
         */
        void DrawComponents(Entity entity);

    private:
        std::shared_ptr<Scene> m_Context;
        Entity m_SelectionContext;
    };

}
