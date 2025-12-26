#pragma once

namespace Voltra {

    // Wrapper for delta time to allow easy conversion between seconds and milliseconds
    class Timestep {
    public:
        Timestep(float time = 0.0f) : m_Time(time) {}

        // Allow implicit conversion to float
        operator float() const { return m_Time; }

        [[nodiscard]] float GetSeconds() const { return m_Time; }
        [[nodiscard]] float GetMilliseconds() const { return m_Time * 1000.0f; }

    private:
        float m_Time;
    };

}