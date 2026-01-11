#pragma once

#include "Core/Core.hpp"
#include "Event.hpp"
#include <cstdint>
#include <sstream>

namespace Voltra {

    /**
     * @brief Event triggered when the window is resized.
     */
    class VOLTRA_API WindowResizeEvent : public Event {
    public:
        /**
         * @brief Constructs a WindowResizeEvent.
         * 
         * @param width The new width of the window.
         * @param height The new height of the window.
         */
        WindowResizeEvent(uint32_t width, uint32_t height)
            : m_Width(width), m_Height(height) {}

        /**
         * @brief Gets the new width.
         * 
         * @return The width in pixels.
         */
        uint32_t GetWidth() const { return m_Width; }

        /**
         * @brief Gets the new height.
         * 
         * @return The height in pixels.
         */
        uint32_t GetHeight() const { return m_Height; }

        /**
         * @brief Returns a string representation of the event.
         * 
         * @return Debug string.
         */
        std::string ToString() const override {
            std::stringstream ss;
            ss << "WindowResizeEvent: " << m_Width << ", " << m_Height;
            return ss.str();
        }

        EVENT_CLASS_TYPE(WindowResize)
        EVENT_CLASS_CATEGORY(EventCategoryApplication)
    private:
        uint32_t m_Width, m_Height;
    };

    /**
     * @brief Event triggered when the window is closed.
     */
    class VOLTRA_API WindowCloseEvent : public Event {
    public:
        WindowCloseEvent() = default;

        EVENT_CLASS_TYPE(WindowClose)
        EVENT_CLASS_CATEGORY(EventCategoryApplication)
    };

    /**
     * @brief Event triggered on application tick.
     */
    class VOLTRA_API AppTickEvent : public Event {
    public:
        AppTickEvent() = default;

        EVENT_CLASS_TYPE(AppTick)
        EVENT_CLASS_CATEGORY(EventCategoryApplication)
    };

    /**
     * @brief Event triggered on application update.
     */
    class VOLTRA_API AppUpdateEvent : public Event {
    public:
        AppUpdateEvent() = default;

        EVENT_CLASS_TYPE(AppUpdate)
        EVENT_CLASS_CATEGORY(EventCategoryApplication)
    };

    /**
     * @brief Event triggered on application render.
     */
    class VOLTRA_API AppRenderEvent : public Event {
    public:
        AppRenderEvent() = default;

        EVENT_CLASS_TYPE(AppRender)
        EVENT_CLASS_CATEGORY(EventCategoryApplication)
    };
}
