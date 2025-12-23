#pragma once

#include "Window.hpp"
#include <memory>

namespace Voltra {

    class Application {
    public:
        Application();
        virtual ~Application();

        void Run();

    private:
        std::unique_ptr<Window> m_Window;
        bool m_Running = true;
    };

    // Defined in client (main.cpp)
    Application* CreateApplication();

}