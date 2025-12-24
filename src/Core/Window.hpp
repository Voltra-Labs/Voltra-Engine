#pragma once

#include <cstdint>
#include <string>
#include <functional>

struct GLFWwindow;

namespace Voltra {

    class Event;

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

        using EventCallbackFn = std::function<void(Event&)>;

        Window(const Properties& props = Properties());
        ~Window();

        void OnUpdate();

        void SetEventCallback(const EventCallbackFn& callback) { m_Data.EventCallback = callback; }

        [[nodiscard]] uint32_t GetWidth() const { return m_Data.Width; }
        [[nodiscard]] uint32_t GetHeight() const { return m_Data.Height; }
        [[nodiscard]] GLFWwindow* GetNativeWindow() const { return m_Window; }

        [[nodiscard]] bool ShouldClose() const;

    private:
        void Init(const Properties& props);
        void Shutdown();

        GLFWwindow* m_Window{ nullptr };
        
        struct WindowData {
            std::string Title;
            uint32_t Width, Height;
            EventCallbackFn EventCallback;
        };

        WindowData m_Data;
    };

}