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

        entt::registry& GetRegistry() { return m_Registry; }

    private:
        entt::registry m_Registry;
        b2World* m_PhysicsWorld = nullptr;

        friend class Entity;
        friend class SceneHierarchyPanel;
        friend class SceneSerializer;
    };

}