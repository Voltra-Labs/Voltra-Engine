#pragma once

#include <Voltra.hpp>
#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"

class ExampleLayer : public Voltra::Layer {
public:
    ExampleLayer();
    virtual ~ExampleLayer() = default;

    virtual void OnAttach() override;
    virtual void OnDetach() override;

    virtual void OnUpdate(Voltra::Timestep ts) override;
    virtual void OnEvent(Voltra::Event& event) override;

private:
    std::shared_ptr<Voltra::Scene> m_ActiveScene;
    Voltra::Entity m_SquareEntity;
    Voltra::Entity m_CameraEntity;
};