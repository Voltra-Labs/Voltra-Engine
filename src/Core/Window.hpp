#pragma once

#include <cstdint>
#include <string>

struct GLFWwindow;

namespace Voltra {

    class Window {
    public:
        struct Properties {
            std::string Title;
            uint32_t Width;
            uint32_t Height;

            Properties(const std::string& title = "Voltra Engine",
                       uint32_t width = 1280,
                       uint32_t height = 720)
                : Title(title), Width(width), Height(height) {}
        };

        Window(const Properties& props = Properties());
        ~Window();

        void OnUpdate();

        [[nodiscard]] uint32_t GetWidth() const { return m_Data.Width; }
        [[nodiscard]] uint32_t GetHeight() const { return m_Data.Height; }
        [[nodiscard]] GLFWwindow* GetNativeWindow() const { return m_Window; }

        [[nodiscard]] bool ShouldClose() const;

    private:
        void Init(const Properties& props);
        void Shutdown();

        GLFWwindow* m_Window{ nullptr };
        Properties m_Data;
    };

}