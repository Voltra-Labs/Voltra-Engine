#pragma once

#include "Event.hpp"

namespace Voltra {

    /**
     * @brief Base class for keyboard events.
     */
    class KeyEvent : public Event {
    public:
        /**
         * @brief Gets the key code associated with this event.
         * 
         * @return The GLFW key code.
         */
        inline int GetKeyCode() const { return m_KeyCode; }

        EVENT_CLASS_CATEGORY(EventCategoryKeyboard | EventCategoryInput)
    protected:
        /**
         * @brief Constructs a KeyEvent.
         * 
         * @param keycode The key code.
         */
        KeyEvent(int keycode)
            : m_KeyCode(keycode) {}

        int m_KeyCode;
    };

    /**
     * @brief Event triggered when a key is pressed.
     */
    class KeyPressedEvent : public KeyEvent {
    public:
        /**
         * @brief Constructs a KeyPressedEvent.
         * 
         * @param keycode The key code.
         * @param repeatCount The repeat count (0 for initial press).
         */
        KeyPressedEvent(int keycode, int repeatCount)
            : KeyEvent(keycode), m_RepeatCount(repeatCount) {}

        /**
         * @brief Gets the repeat count of the key press.
         * 
         * @return The number of times the key has repeated.
         */
        inline int GetRepeatCount() const { return m_RepeatCount; }

        std::string ToString() const override {
            std::stringstream ss;
            ss << "KeyPressedEvent: " << m_KeyCode << " (" << m_RepeatCount << " repeats)";
            return ss.str();
        }

        EVENT_CLASS_TYPE(KeyPressed)
    private:
        int m_RepeatCount;
    };

    /**
     * @brief Event triggered when a key is released.
     */
    class KeyReleasedEvent : public KeyEvent {
    public:
        /**
         * @brief Constructs a KeyReleasedEvent.
         * 
         * @param keycode The key code.
         */
        KeyReleasedEvent(int keycode)
            : KeyEvent(keycode) {}

        std::string ToString() const override {
            std::stringstream ss;
            ss << "KeyReleasedEvent: " << m_KeyCode;
            return ss.str();
        }

        EVENT_CLASS_TYPE(KeyReleased)
    };

    /**
     * @brief Event triggered when a key is typed (text input).
     */
    class KeyTypedEvent : public KeyEvent {
    public:
        /**
         * @brief Constructs a KeyTypedEvent.
         * 
         * @param keycode The character code.
         */
        KeyTypedEvent(int keycode)
            : KeyEvent(keycode) {}

        std::string ToString() const override {
            std::stringstream ss;
            ss << "KeyTypedEvent: " << m_KeyCode;
            return ss.str();
        }

        EVENT_CLASS_TYPE(KeyTyped)
    };
}
