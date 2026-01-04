#pragma once

#include <Voltra.hpp>
#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"

/**
 * @brief A simple example layer demonstrating basic usage of the engine.
 */
class ExampleLayer : public Voltra::Layer {
public:
    /**
     * @brief Constructs the Example Layer.
     */
    ExampleLayer();
    virtual ~ExampleLayer() = default;

    /**
     * @brief Initializes the scene and creates entities.
     */
    virtual void OnAttach() override;

    /**
     * @brief Cleans up resources.
     */
    virtual void OnDetach() override;

    /**
     * @brief Updates the logic and rendering.
     * 
     * @param ts Time step.
     */
    virtual void OnUpdate(Voltra::Timestep ts) override;

    /**
     * @brief Handles events.
     * 
     * @param event The event.
     */
    virtual void OnEvent(Voltra::Event& event) override;

private:
    std::shared_ptr<Voltra::Scene> m_ActiveScene;
    Voltra::Entity m_SquareEntity;
    Voltra::Entity m_CameraEntity;
};