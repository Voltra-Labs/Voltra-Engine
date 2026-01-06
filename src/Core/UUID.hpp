#pragma once

#include <cstdint>
#include <functional>

namespace Voltra {

    /**
     * @brief A unique identifier class (UUID).
     * 
     * Represents a 64-bit universally unique identifier using random number generation.
     */
    class UUID {
    public:
        /**
         * @brief Default constructor. Generates a new random UUID.
         */
        UUID();

        /**
         * @brief Constructs a UUID from an existing 64-bit integer.
         * 
         * @param uuid The 64-bit integer to use as the UUID.
         */
        UUID(uint64_t uuid);

        /**
         * @brief Copy constructor.
         * 
         * @param other The other UUID to copy from.
         */
        UUID(const UUID& other) = default;

        /**
         * @brief Conversion operator to uint64_t.
         * 
         * Allows the UUID to be treated as a uint64_t implicitly.
         * 
         * @return The underlying 64-bit integer value.
         */
        operator uint64_t() const { return m_Value; }

    private:
        uint64_t m_Value;
    };

}

namespace std {

    /**
     * @brief Hash specialization for Voltra::UUID.
     * 
     * Allows Voltra::UUID to be used as a key in std::unordered_map.
     */
    template<>
    struct hash<Voltra::UUID> {
        std::size_t operator()(const Voltra::UUID& uuid) const {
            return (uint64_t)uuid;
        }
    };

}
