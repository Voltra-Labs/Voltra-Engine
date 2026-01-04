#include "LayerStack.hpp"

namespace Voltra {

    /**
     * @brief Destroys the LayerStack and deletes all managed layers.
     */
    LayerStack::~LayerStack() {
        for (Layer* layer : m_Layers) {
            delete layer;
        }
    }

    /**
     * @brief Pushes a normal layer into the stack.
     * 
     * The layer is inserted at the current insertion index, keeping it below overlays.
     * 
     * @param layer The layer to add.
     */
    void LayerStack::PushLayer(Layer* layer) {
        m_Layers.emplace(m_Layers.begin() + m_LayerInsertIndex, layer);
        m_LayerInsertIndex++;
        layer->OnAttach();
    }

    /**
     * @brief Pushes an overlay on top of the stack.
     * 
     * Overlays are always added to the end of the list.
     * 
     * @param overlay The overlay to add.
     */
    void LayerStack::PushOverlay(Layer* overlay) {
        m_Layers.emplace_back(overlay);
        overlay->OnAttach();
    }

    /**
     * @brief Removes a normal layer from the stack.
     * 
     * @param layer The layer to remove.
     */
    void LayerStack::PopLayer(Layer* layer) {
        auto it = std::find(m_Layers.begin(), m_Layers.begin() + m_LayerInsertIndex, layer);
        if (it != m_Layers.begin() + m_LayerInsertIndex) {
            layer->OnDetach();
            m_Layers.erase(it);
            m_LayerInsertIndex--;
        }
    }

    /**
     * @brief Removes an overlay from the stack.
     * 
     * @param overlay The overlay to remove.
     */
    void LayerStack::PopOverlay(Layer* overlay) {
        auto it = std::find(m_Layers.begin() + m_LayerInsertIndex, m_Layers.end(), overlay);
        if (it != m_Layers.end()) {
            overlay->OnDetach();
            m_Layers.erase(it);
        }
    }
}