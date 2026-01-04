#pragma once

#include "Layer.hpp"
#include <vector>

namespace Voltra {

    /**
     * @brief Manages a stack of layers.
     * 
     * Layers are organized into normal layers and overlays.
     * Normal layers are pushed to the first half of the list, while overlays are always on top.
     */
    class LayerStack {
    public:
        /**
         * @brief Default constructor for LayerStack.
         */
        LayerStack() = default;

        /**
         * @brief Destroys the LayerStack and cleans up all layers.
         */
        ~LayerStack();

        /**
         * @brief Pushes a normal layer into the stack.
         * 
         * @param layer The layer to be added.
         */
        void PushLayer(Layer* layer);

        /**
         * @brief Pushes an overlay on top of the stack.
         * 
         * @param overlay The overlay to be added.
         */
        void PushOverlay(Layer* overlay);

        /**
         * @brief Removes a normal layer from the stack.
         * 
         * @param layer The layer to be removed.
         */
        void PopLayer(Layer* layer);

        /**
         * @brief Removes an overlay from the stack.
         * 
         * @param overlay The overlay to be removed.
         */
        void PopOverlay(Layer* overlay);

        /**
         * @brief Returns an iterator to the beginning of the layer stack.
         * 
         * @return std::vector<Layer*>::iterator 
         */
        std::vector<Layer*>::iterator begin() { return m_Layers.begin(); }

        /**
         * @brief Returns an iterator to the end of the layer stack.
         * 
         * @return std::vector<Layer*>::iterator 
         */
        std::vector<Layer*>::iterator end() { return m_Layers.end(); }

    private:
        std::vector<Layer*> m_Layers;
        unsigned int m_LayerInsertIndex = 0;
    };

}