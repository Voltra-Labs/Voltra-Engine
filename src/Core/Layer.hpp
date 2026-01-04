#pragma once

#include "Core/Timestep.hpp"
#include "Events/Event.hpp"
#include <string>

namespace Voltra {

    /**
     * @brief Base class for application layers.
     * 
     * Layers are pushed to the LayerStack and updated sequentially.
     * They can handle events and render ImGui elements.
     */
    class Layer {
    public:
        /**
         * @brief Constructs a new Layer object.
         * 
         * @param name The debug name of the layer.
         */
        Layer(const std::string& name = "Layer");

        /**
         * @brief Destroys the Layer object.
         */
        virtual ~Layer() = default;

        /**
         * @brief Called when the layer is attached to the Application.
         */
        virtual void OnAttach() {}

        /**
         * @brief Called when the layer is detached from the Application.
         */
        virtual void OnDetach() {}

        /**
         * @brief Updates the layer logic.
         * 
         * @param ts The timestep (delta time) for this frame.
         */
        virtual void OnUpdate(Timestep ts) {}

        /**
         * @brief Renders ImGui elements for this layer.
         */
        virtual void OnImGuiRender() {}

        /**
         * @brief Handles events dispatched to this layer.
         * 
         * @param event The event to handle.
         */
        virtual void OnEvent(Event& event) {}

        /**
         * @brief Gets the debug name of the layer.
         * 
         * @return The name of the layer.
         */
        [[nodiscard]] const std::string& GetName() const { return m_DebugName; }
    
    protected:
        std::string m_DebugName;
    };

}