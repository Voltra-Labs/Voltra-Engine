#include "UUID.hpp"

#include <random>

namespace Voltra {

    static std::random_device s_RandomDevice;
    static std::mt19937_64 s_Engine(s_RandomDevice());
    static std::uniform_int_distribution<uint64_t> s_UniformDistribution;

    /**
     * @brief Default constructor. Generates a new random UUID.
     */
    UUID::UUID()
        : m_Value(s_UniformDistribution(s_Engine)) {
    }

    /**
     * @brief Constructs a UUID from an existing 64-bit integer.
     * 
     * @param uuid The 64-bit integer to use as the UUID.
     */
    UUID::UUID(uint64_t uuid)
        : m_Value(uuid) {
    }

}
