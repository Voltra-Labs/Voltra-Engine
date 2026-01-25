#pragma once

#include "Core.hpp"

namespace Voltra {

    /**
     * @brief Wrapper class for delta time.
     * 
     * Allows easy conversion between seconds and milliseconds.
     */
    class VOLTRA_API Timestep {
    public:
        /**
         * @brief Constructs a Timestep.
         * 
         * @param time Time in seconds.
         */
        Timestep(float time = 0.0f) : m_Time(time) {}

        /**
         * @brief Implicit conversion to float (seconds).
         * 
         * @return The time in seconds.
         */
        operator float() const { 
            return m_Time; 
        }

        /**
         * @brief Gets the time in seconds.
         * 
         * @return Time in seconds.
         */
        [[nodiscard]] float GetSeconds() const { 
            return m_Time; 
        }

        /**
         * @brief Gets the time in milliseconds.
         * 
         * @return Time in milliseconds.
         */
        [[nodiscard]] float GetMilliseconds() const { 
            return m_Time * 1000.0f;
        }

    private:
        float m_Time;
    };

}