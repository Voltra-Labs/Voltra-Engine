#pragma once

#include "Core/Core.hpp"
#include "Entity.hpp"

namespace Voltra {

    /**
     * @brief Base class for native scripts attached to entities.
     * 
     * Allows writing logic in C++ that interacts with the ECS.
     */
    class VOLTRA_API ScriptableEntity {
    public:
        virtual ~ScriptableEntity() {}

        /**
         * @brief Gets a component from the attached entity.
         * 
         * @tparam T The component type.
         * @return Reference to the component.
         */
        template<typename T>
        T& GetComponent() {
            return m_Entity.GetComponent<T>();
        }

    protected:
        /**
         * @brief Called when the script is instantiated.
         */
        virtual void OnCreate() {}

        /**
         * @brief Called when the script (or entity) is destroyed.
         */
        virtual void OnDestroy() {}

        /**
         * @brief Called every frame.
         * 
         * @param ts Time step.
         */
        virtual void OnUpdate(Timestep ts) {}

    private:
        Entity m_Entity;
        friend class Scene;
    };
}