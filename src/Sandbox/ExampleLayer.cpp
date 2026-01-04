#include "ExampleLayer.hpp"
#include "Scene/Components.hpp"

#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>

/**
 * @brief Constructs the layer with a debug name.
 */
ExampleLayer::ExampleLayer()
    : Layer("ExampleLayer") {
}

/**
 * @brief Creates a checkerboard texture and assigns it to a sprite in the scene.
 */
void ExampleLayer::OnAttach() {
    m_ActiveScene = std::make_shared<Voltra::Scene>();

    auto checkerTexture = std::make_shared<Voltra::Texture2D>(2, 2);
    
    uint32_t white = 0xFFFFFFFF;
    uint32_t black = 0xFF000000;
    uint32_t data[] = { white, black, black, white };
    
    checkerTexture->SetData(data, sizeof(data));

    m_SquareEntity = m_ActiveScene->CreateEntity("Checker Square");
    
    m_SquareEntity.AddComponent<Voltra::SpriteRendererComponent>(checkerTexture);
    
    m_SquareEntity.GetComponent<Voltra::TransformComponent>().Scale = glm::vec3(1.5f);


    m_CameraEntity = m_ActiveScene->CreateEntity("Main Camera");
    m_CameraEntity.AddComponent<Voltra::CameraComponent>();
}

/**
 * @brief Called on layer detachment.
 */
void ExampleLayer::OnDetach() {
}

/**
 * @brief Clears the screen and updates the active scene.
 * 
 * @param ts Time step.
 */
void ExampleLayer::OnUpdate(Voltra::Timestep ts) {
    Voltra::RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1.0f });
    Voltra::RenderCommand::Clear();
    if (m_ActiveScene) {
        m_ActiveScene->OnUpdate(ts);
    }
}

/**
 * @brief Handles window resize or input events.
 * 
 * @param event The event.
 */
void ExampleLayer::OnEvent(Voltra::Event& event) {
}