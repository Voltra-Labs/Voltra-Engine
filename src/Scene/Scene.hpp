#pragma once

#include "Core/Timestep.hpp"
#include <entt/entt.hpp>

class b2World;

namespace Voltra {

    class Entity;

    class Scene {
    public:
        Scene();
        ~Scene();

        Entity CreateEntity(const std::string& name = std::string());
        void DestroyEntity(Entity entity);

        Entity GetPrimaryCameraEntity();

        void OnRuntimeStart();
        void OnRuntimeStop();

        void OnUpdate(Timestep ts); // Existing
        void OnUpdateEditor(Timestep ts, class EditorCamera& camera);
        void OnRenderRuntime(Timestep ts); // Render only, no update
        void OnViewportResize(uint32_t width, uint32_t height);


        entt::registry& GetRegistry() { return m_Registry; }

    private:
        entt::registry m_Registry;
        uint32_t m_ViewportWidth = 0, m_ViewportHeight = 0;
        b2World* m_PhysicsWorld = nullptr;

        friend class Entity;
        friend class SceneHierarchyPanel;
        friend class SceneSerializer;
    };

}