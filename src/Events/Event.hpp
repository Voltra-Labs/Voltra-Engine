#pragma once

#include "Core/Core.hpp"
#include <string>
#include <functional>
#include <iostream>
#include <sstream>

namespace Voltra {

    /**
     * @brief Enum representing the type of an event.
     */
    enum class EventType {
        None = 0,
        WindowClose,
        WindowResize,
        WindowFocus,
        WindowLostFocus,
        WindowMoved,
        AppTick,
        AppUpdate,
        AppRender,
        KeyPressed,
        KeyReleased,
        KeyTyped,
        MouseButtonPressed,
        MouseButtonReleased,
        MouseMoved,
        MouseScrolled
    };

#define BIT(x) (1 << x)

    /**
     * @brief Bitfield enum for event categories.
     * 
     * Allows events to belong to multiple categories (e.g., Input + Keyboard).
     */
    enum EventCategory {
        None = 0,
        EventCategoryApplication    = BIT(0),
        EventCategoryInput          = BIT(1),
        EventCategoryKeyboard       = BIT(2),
        EventCategoryMouse          = BIT(3),
        EventCategoryMouseButton    = BIT(4)
    };
    
#define VOLTRA_BIND_EVENT_FN(fn) [this](auto&&... args) -> decltype(auto) { return this->fn(std::forward<decltype(args)>(args)...); }

#define EVENT_CLASS_TYPE(type) static EventType GetStaticType() { return EventType::type; }\
                               virtual EventType GetEventType() const override { return GetStaticType(); }\
                               virtual const char* GetName() const override { return #type; }

#define EVENT_CLASS_CATEGORY(category) virtual int GetCategoryFlags() const override { return category; }

    /**
     * @brief Base class for all events in the engine.
     * 
     * Events are currently blocking (dispatched immediately).
     */
    class VOLTRA_API Event {
    public:
        virtual ~Event() = default;

        bool Handled = false;

        virtual EventType GetEventType() const = 0;
        virtual const char* GetName() const = 0;
        virtual int GetCategoryFlags() const = 0;
        virtual std::string ToString() const { return GetName(); }

        /**
         * @brief Checks if the event belongs to a specific category.
         * 
         * @param category The category to check against.
         * @return true if the event is in the category, false otherwise.
         */
        inline bool IsInCategory(EventCategory category) {
            return GetCategoryFlags() & category;
        }
    protected:
    };

    /**
     * @brief Utility class to dispatch events based on their type.
     */
    class VOLTRA_API EventDispatcher {
    public:
        /**
         * @brief Constructs an EventDispatcher.
         * 
         * @param event The event to be dispatched.
         */
        EventDispatcher(Event& event)
            : m_Event(event) {}

        /**
         * @brief Dispatches the event if it matches the template type T.
         * 
         * @tparam T The event type to check against.
         * @tparam F The function type (deduced).
         * @param func The callback function to handle the event.
         * @return true if the event was dispatched, false otherwise.
         */
        template<typename T, typename F>
        bool Dispatch(const F& func) {
            if (m_Event.GetEventType() == T::GetStaticType()) {
                m_Event.Handled |= func(static_cast<T&>(m_Event));
                return true;
            }
            return false;
        }
    private:
        Event& m_Event;
    };

    inline std::ostream& operator<<(std::ostream& os, const Event& e) {
        return os << e.ToString();
    }
}
