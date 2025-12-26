#include <gtest/gtest.h>
#include "Events/ApplicationEvent.hpp"
#include "Events/KeyEvent.hpp"
#include "Events/MouseEvent.hpp"

TEST(EventSystem, WindowResizeEvent) {
    Voltra::WindowResizeEvent e(1280, 720);
    
    EXPECT_EQ(e.GetEventType(), Voltra::EventType::WindowResize);

    EXPECT_EQ(e.GetWidth(), 1280);
    EXPECT_EQ(e.GetHeight(), 720);
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryApplication));
    EXPECT_FALSE(e.IsInCategory(Voltra::EventCategoryInput));
    EXPECT_STREQ(e.GetName(), "WindowResize");
}

TEST(EventSystem, KeyEvent) {
    Voltra::KeyPressedEvent e(32, 1); // Space key, 1 repeat
    
    EXPECT_EQ(e.GetEventType(), Voltra::EventType::KeyPressed);
    EXPECT_EQ(e.GetKeyCode(), 32);
    EXPECT_EQ(e.GetRepeatCount(), 1);
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryKeyboard));
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryInput));
}

TEST(EventSystem, Dispatcher) {
    Voltra::WindowResizeEvent e(800, 600);
    Voltra::EventDispatcher dispatcher(e);
    
    bool dispatched = dispatcher.Dispatch<Voltra::WindowResizeEvent>([](Voltra::WindowResizeEvent& ev) {
        return true; // Mark as handled
    });
    
    EXPECT_TRUE(dispatched);
    EXPECT_TRUE(e.Handled);
}

TEST(EventSystem, DispatcherFail) {
    Voltra::WindowResizeEvent e(800, 600);
    Voltra::EventDispatcher dispatcher(e);
    
    // Try to dispatch a completely different event type
    bool dispatched = dispatcher.Dispatch<Voltra::KeyPressedEvent>([](Voltra::KeyPressedEvent& ev) {
        return true;
    });
    
    EXPECT_FALSE(dispatched);
    EXPECT_FALSE(e.Handled);
}

TEST(EventSystem, MouseMovedEvent) {
    Voltra::MouseMovedEvent e(150.0f, 200.0f);
    
    EXPECT_EQ(e.GetEventType(), Voltra::EventType::MouseMoved);
    EXPECT_FLOAT_EQ(e.GetX(), 150.0f);
    EXPECT_FLOAT_EQ(e.GetY(), 200.0f);
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryMouse));
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryInput));
}

TEST(EventSystem, MouseButtonEvent) {
    Voltra::MouseButtonPressedEvent e(0);
    
    EXPECT_EQ(e.GetEventType(), Voltra::EventType::MouseButtonPressed);
    EXPECT_EQ(e.GetMouseButton(), 0);
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryMouseButton));
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryInput));
}

TEST(EventSystem, MouseScrolledEvent) {
    Voltra::MouseScrolledEvent e(0.0f, 1.0f);
    
    EXPECT_EQ(e.GetEventType(), Voltra::EventType::MouseScrolled);
    EXPECT_FLOAT_EQ(e.GetXOffset(), 0.0f);
    EXPECT_FLOAT_EQ(e.GetYOffset(), 1.0f);
    EXPECT_TRUE(e.IsInCategory(Voltra::EventCategoryMouse));
}