#include "Core/Application.hpp"
#include "Sandbox/EditorLayer.hpp"
#include "Core/Log.hpp"
#include <memory>

/**
 * @brief Application entry point.
 * 
 * Initializes the logging system, creates the application instance,
 * attaches the Editor layer, and starts the main run loop.
 * 
 * @return Execution status code (0 for success).
 */

int main() {
    
    Voltra::Log::Init();

    VOLTRA_CORE_INFO("Voltra Engine Initialized!");

    auto app = CreateApplication();
    
    app->Run();

    delete app;

    return 0;
}