#pragma once

#include "Scene.hpp"
#include "Core/Log.hpp"
#include <entt/entt.hpp>

namespace Voltra {

    class Entity {
    public:
        Entity() = default;
        Entity(entt::entity handle, Scene* scene);
        Entity(const Entity& other) = default;

        template<typename T, typename... Args>
        T& AddComponent(Args&&... args) {
            if (HasComponent<T>()) {
                VOLTRA_CORE_WARN("Entity already has component!");
                return GetComponent<T>();
            }
            return m_Scene->m_Registry.emplace<T>(m_EntityHandle, std::forward<Args>(args)...);
        }

        template<typename T>
        T& GetComponent() {
            if (!HasComponent<T>()) {
                VOLTRA_CORE_ERROR("Entity does not have component!");
                throw std::runtime_error("Entity missing component");
            }
            return m_Scene->m_Registry.get<T>(m_EntityHandle);
        }

        template<typename T>
        bool HasComponent() {
            return m_Scene->m_Registry.all_of<T>(m_EntityHandle);
        }

        template<typename T>
        void RemoveComponent() {
            if (!HasComponent<T>()) {
                VOLTRA_CORE_WARN("Entity does not have component to remove!");
                return;
            }
            m_Scene->m_Registry.remove<T>(m_EntityHandle);
        }

        operator bool() const { return m_EntityHandle != entt::null; }
        operator entt::entity() const { return m_EntityHandle; }

    private:
        entt::entity m_EntityHandle{ entt::null };
        Scene* m_Scene = nullptr;
    };

}