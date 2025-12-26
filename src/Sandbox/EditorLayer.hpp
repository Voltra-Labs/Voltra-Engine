#pragma once

#include "Voltra.hpp"

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"

namespace Voltra {

    class EditorLayer : public Layer {
    public:
        EditorLayer();
        virtual ~EditorLayer() = default;

        virtual void OnAttach() override;
        virtual void OnDetach() override;
        virtual void OnUpdate(Timestep ts) override;
        virtual void OnImGuiRender() override;
        virtual void OnEvent(Event& e) override;

    private:
        std::shared_ptr<Scene> m_ActiveScene;
        Entity m_SquareEntity;
        glm::vec4 m_SquareColor = { 0.2f, 0.3f, 0.8f, 1.0f };
    };

}