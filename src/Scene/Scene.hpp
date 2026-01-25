#pragma once

#include "Core/Core.hpp"
#include "Core/Timestep.hpp"
#include "Core/UUID.hpp"
#include <entt/entt.hpp>

class b2World;

namespace Voltra {

    class Entity;

    /**
     * @brief The main container for entities, components, and systems.
     * 
     * Manages the Entity Component System (ECS) registry and the physics world.
     */
    class VOLTRA_API Scene {
    public:
        /**
         * @brief Constructs a new Scene.
         */
        Scene();
        /**
         * @brief Destroys the Scene.
         */
        ~Scene();

        /**
         * @brief Creates a new entity in the scene.
         * 
         * @param name Optional debug name (tag).
         * @return The created entity.
         */
        Entity CreateEntity(const std::string& name = std::string(), UUID uuid = UUID());

        /**
         * @brief Destroys an entity.
         * 
         * @param entity The entity to destroy.
         */
        void DestroyEntity(Entity entity);

        /**
         * @brief Finds the first camera marked as primary.
         * 
         * @return The primary camera entity.
         */
        Entity GetPrimaryCameraEntity();

        /**
         * @brief Initializes physics and scripts.
         */
        void OnRuntimeStart();

        /**
         * @brief Shuts down physics and scripts.
         */
        void OnRuntimeStop();

        /**
         * @brief Updates the scene logic (Physics, Scripts, Rendering).
         * 
         * @param ts Time step.
         */
        void OnUpdate(Timestep ts);

        /**
         * @brief Updates the scene for the Editor (Rendering only).
         * 
         * @param ts Time step.
         * @param camera The editor camera.
         */
        void OnUpdateEditor(Timestep ts, class EditorCamera& camera);

        /**
         * @brief Renders the scene without updating logic.
         * 
         * @param ts Time step.
         */
        void OnRenderRuntime(Timestep ts);

        /**
         * @brief Handles viewport resize events.
         * 
         * @param width New width.
         * @param height New height.
         */
        void OnViewportResize(uint32_t width, uint32_t height);


        /**
         * @brief Access the internal ENTT registry.
         * 
         * @return Reference to the registry.
         */
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