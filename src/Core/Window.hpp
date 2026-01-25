#pragma once

#include "Core.hpp"

#include <cstdint>
#include <string>
#include <functional>

struct GLFWwindow;

namespace Voltra {

    class Event;

    /**
     * @brief Abstraction of a desktop window.
     * 
     * Wraps GLFW window creation, event polling, and buffer swapping.
     */
    class VOLTRA_API Window {
    public:
        /**
         * @brief Configuration properties for creating a window.
         */
        struct Properties {
            std::string Title;
            uint32_t Width;
            uint32_t Height;

            /**
             * @brief Constructs Window Properties.
             * 
             * @param title The title of the window.
             * @param width The width of the window in pixels.
             * @param height The height of the window in pixels.
             */
            Properties(const std::string& title = "Voltra Engine",
                       uint32_t width = 1280,
                       uint32_t height = 720)
                : Title(title), Width(width), Height(height) {}
        };

        using EventCallbackFn = std::function<void(Event&)>;

        /**
         * @brief Creates a new Window with the specified properties.
         * 
         * @param props The configuration properties.
         */
        Window(const Properties& props = Properties());

        /**
         * @brief Destroys the Window and cleans up resources.
         */
        ~Window();

        /**
         * @brief Updates the window (polls events and swaps buffers).
         */
        void OnUpdate();

        /**
         * @brief Sets the event callback function.
         * 
         * @param callback The function to call when an event occurs.
         */
        void SetEventCallback(const EventCallbackFn& callback) { 
            m_Data.EventCallback = callback;
        }

        /**
         * @brief Gets the window width.
         * 
         * @return The width in pixels.
         */
        [[nodiscard]] uint32_t GetWidth() const { 
            return m_Data.Width;
        }
        /**
         * @brief Gets the window height.
         * 
         * @return The height in pixels.
         */
        [[nodiscard]] uint32_t GetHeight() const { 
            return m_Data.Height;
        }
        /**
         * @brief Gets the native GLFW window handle.
         * 
         * @return Pointer to the GLFWwindow.
         */
        [[nodiscard]] GLFWwindow* GetNativeWindow() const { 
            return m_Window;
        }

        /**
         * @brief Checks if the window should close (e.g. user clicked X).
         * 
         * @return true if the window should close, false otherwise.
         */
        [[nodiscard]] bool ShouldClose() const;

    private:
        // Initializes the window system
        void Init(const Properties& props);
        // Shuts down the window system
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