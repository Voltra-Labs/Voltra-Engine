#pragma once

#include "Core/Timestep.hpp"
#include "Events/Event.hpp"
#include <string>

namespace Voltra {

    class Layer {
    public:
        Layer(const std::string& name = "Layer");
        virtual ~Layer() = default;

        virtual void OnAttach() {}
        virtual void OnDetach() {}
        virtual void OnUpdate(Timestep ts) {}
        virtual void OnImGuiRender() {}
        virtual void OnEvent(Event& event) {}

        [[nodiscard]] const std::string& GetName() const { return m_DebugName; }
    
    protected:
        std::string m_DebugName;
    };

}