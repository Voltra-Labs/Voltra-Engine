#include "Core/Application.hpp"
#include <memory>

// Entry point
int main() {
    // Heap allocation ensures stack safety for large engine objects
    auto app = std::make_unique<Voltra::Application>();
    
    app->Run();

    return 0;
}