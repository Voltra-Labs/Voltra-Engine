#include "Entity.hpp"

namespace Voltra {

    /**
     * @brief Constructs an entity handle attached to a specific scene.
     * 
     * @param handle The internal ECS identifier.
     * @param scene The owning scene.
     */
    Entity::Entity(entt::entity handle, Scene* scene)
        : m_EntityHandle(handle), m_Scene(scene) {
    }

}