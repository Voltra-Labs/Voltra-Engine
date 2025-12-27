#include "ExampleLayer.hpp"
#include "Scene/Components.hpp"

#include <glm/gtc/matrix_transform.hpp>
#include <glm/gtc/type_ptr.hpp>

ExampleLayer::ExampleLayer()
    : Layer("ExampleLayer") {
}

void ExampleLayer::OnAttach() {
    // Initialize scene
    m_ActiveScene = std::make_shared<Voltra::Scene>();

    // Create a procedural texture (Checkerboard 2x2)
    auto checkerTexture = std::make_shared<Voltra::Texture2D>(2, 2);
    
    uint32_t white = 0xFFFFFFFF;
    uint32_t black = 0xFF000000;
    uint32_t data[] = { white, black, black, white };
    
    checkerTexture->SetData(data, sizeof(data));

    // Create entity with texture
    m_SquareEntity = m_ActiveScene->CreateEntity("Checker Square");
    
    // Add component with texture
    m_SquareEntity.AddComponent<Voltra::SpriteRendererComponent>(checkerTexture);
    
    // Optional: Scale to see the pixels better
    m_SquareEntity.GetComponent<Voltra::TransformComponent>().Scale = glm::vec3(1.5f);


    // Create camera
    m_CameraEntity = m_ActiveScene->CreateEntity("Main Camera");
    m_CameraEntity.AddComponent<Voltra::CameraComponent>();
}

void ExampleLayer::OnDetach() {
}

void ExampleLayer::OnUpdate(Voltra::Timestep ts) {
    // Clear screen (Dark gray)
    Voltra::RenderCommand::SetClearColor({ 0.1f, 0.1f, 0.1f, 1.0f });
    Voltra::RenderCommand::Clear();

    // Update Scene
    // This automatically handles: Scripts -> Transform Calculation -> Rendering
    if (m_ActiveScene) {
        m_ActiveScene->OnUpdate(ts);
    }
}

void ExampleLayer::OnEvent(Voltra::Event& event) {
    // Handle window resize events to adjust camera aspect ratio
}