#pragma once

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include <memory>

namespace Voltra {

    class SceneHierarchyPanel {
    public:
        SceneHierarchyPanel() = default;
        SceneHierarchyPanel(const std::shared_ptr<Scene>& context);

        void SetContext(const std::shared_ptr<Scene>& context);

        void OnImGuiRender();

        Entity GetSelectedEntity() const { return m_SelectionContext; }

    private:
        template<typename T, typename UIFunction>
        void DrawComponent(const std::string& name, Entity entity, UIFunction uiFunction);

        void DrawEntityNode(Entity entity);
        void DrawComponents(Entity entity);

    private:
        std::shared_ptr<Scene> m_Context;
        Entity m_SelectionContext;
    };

}
