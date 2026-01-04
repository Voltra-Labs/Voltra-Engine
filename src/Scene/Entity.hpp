#pragma once

#include "Scene.hpp"
#include "Core/Log.hpp"
#include <entt/entt.hpp>

namespace Voltra {

    /**
     * @brief Represents an entity in the scene (ECS wrapper around entt::entity).
     */
    class Entity {
    public:
        Entity() = default;
        
        /**
         * @brief Constructs an Entity from a handle and scene.
         * 
         * @param handle The internal entt handle.
         * @param scene The scene context.
         */
        Entity(entt::entity handle, Scene* scene);
        Entity(const Entity& other) = default;

        /**
         * @brief Adds a component to the entity.
         * 
         * @tparam T The component type.
         * @tparam Args Constructor arguments for the component.
         * @param args Arguments forwarded to component constructor.
         * @return Reference to the created component.
         */
        template<typename T, typename... Args>
        T& AddComponent(Args&&... args) {
            if (HasComponent<T>()) {
                VOLTRA_CORE_WARN("Entity already has component!");
                return GetComponent<T>();
            }
            return m_Scene->m_Registry.emplace<T>(m_EntityHandle, std::forward<Args>(args)...);
        }

        /**
         * @brief Gets a reference to a component.
         * 
         * @tparam T The component type.
         * @return Reference to the component.
         * @throws std::runtime_error if component is missing.
         */
        template<typename T>
        T& GetComponent() {
            if (!HasComponent<T>()) {
                VOLTRA_CORE_ERROR("Entity does not have component!");
                throw std::runtime_error("Entity missing component");
            }
            return m_Scene->m_Registry.get<T>(m_EntityHandle);
        }

        /**
         * @brief Checks if the entity has a specific component.
         * 
         * @tparam T The component type.
         * @return true if the entity has the component.
         */
        template<typename T>
        bool HasComponent() {
            return m_Scene->m_Registry.all_of<T>(m_EntityHandle);
        }

        /**
         * @brief Removes a component from the entity.
         * 
         * @tparam T The component type.
         */
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