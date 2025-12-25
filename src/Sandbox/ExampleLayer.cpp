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

    // Create entity "Green Square"
    m_SquareEntity = m_ActiveScene->CreateEntity("Green Square");
    
    // Add Sprite component (Green color)
    m_SquareEntity.AddComponent<Voltra::SpriteRendererComponent>(glm::vec4{0.2f, 0.8f, 0.3f, 1.0f});
    
    // Optional: Modify its Transform (position/scale)
    auto& transform = m_SquareEntity.GetComponent<Voltra::TransformComponent>();
    transform.Transform = glm::scale(glm::mat4(1.0f), glm::vec3(0.5f)); // Make the square a bit smaller

    // Create entity "Main Camera"
    m_CameraEntity = m_ActiveScene->CreateEntity("Main Camera");
    m_CameraEntity.AddComponent<Voltra::CameraComponent>();

    // Note: CameraComponent by default has 'Primary = true', so the scene will use it automatically.
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