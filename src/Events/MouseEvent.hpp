#pragma once

#include "Core/Core.hpp"
#include "Event.hpp"

namespace Voltra {

    /**
     * @brief Event triggered when the mouse moves.
     */
    class VOLTRA_API MouseMovedEvent : public Event {
    public:
        /**
         * @brief Constructs a MouseMovedEvent.
         * 
         * @param x The X coordinate of the mouse.
         * @param y The Y coordinate of the mouse.
         */
        MouseMovedEvent(float x, float y)
            : m_MouseX(x), m_MouseY(y) {}

        /**
         * @brief Gets the X coordinate.
         * 
         * @return X position.
         */
        inline float GetX() const { return m_MouseX; }

        /**
         * @brief Gets the Y coordinate.
         * 
         * @return Y position.
         */
        inline float GetY() const { return m_MouseY; }

        std::string ToString() const override {
            std::stringstream ss;
            ss << "MouseMovedEvent: " << m_MouseX << ", " << m_MouseY;
            return ss.str();
        }

        EVENT_CLASS_TYPE(MouseMoved)
        EVENT_CLASS_CATEGORY(EventCategoryMouse | EventCategoryInput)
    private:
        float m_MouseX, m_MouseY;
    };

    /**
     * @brief Event triggered when the mouse scroll wheel is used.
     */
    class VOLTRA_API MouseScrolledEvent : public Event {
    public:
        /**
         * @brief Constructs a MouseScrolledEvent.
         * 
         * @param xOffset The horizontal scroll offset.
         * @param yOffset The vertical scroll offset.
         */
        MouseScrolledEvent(float xOffset, float yOffset)
            : m_XOffset(xOffset), m_YOffset(yOffset) {}

        /**
         * @brief Gets the horizontal scroll offset.
         * 
         * @return X offset.
         */
        inline float GetXOffset() const { return m_XOffset; }

        /**
         * @brief Gets the vertical scroll offset.
         * 
         * @return Y offset.
         */
        inline float GetYOffset() const { return m_YOffset; }

        std::string ToString() const override {
            std::stringstream ss;
            ss << "MouseScrolledEvent: " << m_XOffset << ", " << m_YOffset;
            return ss.str();
        }

        EVENT_CLASS_TYPE(MouseScrolled)
        EVENT_CLASS_CATEGORY(EventCategoryMouse | EventCategoryInput)
    private:
        float m_XOffset, m_YOffset;
    };

    /**
     * @brief Base class for mouse button events.
     */
    class VOLTRA_API MouseButtonEvent : public Event {
    public:
        /**
         * @brief Gets the mouse button code.
         * 
         * @return The mouse button code.
         */
        inline int GetMouseButton() const { return m_Button; }

        EVENT_CLASS_CATEGORY(EventCategoryMouse | EventCategoryInput | EventCategoryMouseButton)
    protected:
        /**
         * @brief Constructs a MouseButtonEvent.
         * 
         * @param button The mouse button code.
         */
        MouseButtonEvent(int button)
            : m_Button(button) {}

        int m_Button;
    };

    /**
     * @brief Event triggered when a mouse button is pressed.
     */
    class VOLTRA_API MouseButtonPressedEvent : public MouseButtonEvent {
    public:
        /**
         * @brief Constructs a MouseButtonPressedEvent.
         * 
         * @param button The mouse button code.
         */
        MouseButtonPressedEvent(int button)
            : MouseButtonEvent(button) {}

        std::string ToString() const override {
            std::stringstream ss;
            ss << "MouseButtonPressedEvent: " << m_Button;
            return ss.str();
        }

        EVENT_CLASS_TYPE(MouseButtonPressed)
    };

    /**
     * @brief Event triggered when a mouse button is released.
     */
    class VOLTRA_API MouseButtonReleasedEvent : public MouseButtonEvent {
    public:
        /**
         * @brief Constructs a MouseButtonReleasedEvent.
         * 
         * @param button The mouse button code.
         */
        MouseButtonReleasedEvent(int button)
            : MouseButtonEvent(button) {}

        std::string ToString() const override {
            std::stringstream ss;
            ss << "MouseButtonReleasedEvent: " << m_Button;
            return ss.str();
        }

        EVENT_CLASS_TYPE(MouseButtonReleased)
    };
}
