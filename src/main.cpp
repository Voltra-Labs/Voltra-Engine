#include "Core/Application.hpp"
#include "Sandbox/EditorLayer.hpp"
#include "Core/Log.hpp"
#include <memory>

// Entry point
int main() {

    Voltra::Log::Init();
    
    VOLTRA_CORE_INFO("Voltra Engine Initialized!");

    // Heap allocation ensures stack safety for large engine objects
    auto app = std::make_unique<Voltra::Application>();
    
    app->PushLayer(new Voltra::EditorLayer());
    
    app->Run();

    return 0;
}