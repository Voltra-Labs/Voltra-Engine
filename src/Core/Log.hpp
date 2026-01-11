#pragma once

#include "Core.hpp"

#include <memory>
#include <spdlog/spdlog.h>

namespace Voltra {

    /**
     * @brief Logging system wrapper.
     * 
     * Initialized spdlog loggers for the Engine (Core) and the Client (App).
     */
    class VOLTRA_API Log {
    public:
        /**
         * @brief Initializes the logging subsystem.
         * 
         * Sets up the patterns and creates the loggers.
         */
        static void Init();

        /**
         * @brief Retrieves the engine's core logger.
         * 
         * @return Reference to the core logger shared pointer.
         */
        static std::shared_ptr<spdlog::logger>& GetCoreLogger() { return s_CoreLogger; }
        /**
         * @brief Retrieves the client's logger.
         * 
         * @return Reference to the client logger shared pointer.
         */
        static std::shared_ptr<spdlog::logger>& GetClientLogger() { return s_ClientLogger; }

    private:
        static std::shared_ptr<spdlog::logger> s_CoreLogger;
        static std::shared_ptr<spdlog::logger> s_ClientLogger;
    };

}

// Core Logging Macros (Use these inside the Engine code)
#define VOLTRA_CORE_TRACE(...)    ::Voltra::Log::GetCoreLogger()->trace(__VA_ARGS__)
#define VOLTRA_CORE_INFO(...)     ::Voltra::Log::GetCoreLogger()->info(__VA_ARGS__)
#define VOLTRA_CORE_WARN(...)     ::Voltra::Log::GetCoreLogger()->warn(__VA_ARGS__)
#define VOLTRA_CORE_ERROR(...)    ::Voltra::Log::GetCoreLogger()->error(__VA_ARGS__)
#define VOLTRA_CORE_FATAL(...)    ::Voltra::Log::GetCoreLogger()->critical(__VA_ARGS__)

// Client Logging Macros (Use these in the Game/App code)
#define VOLTRA_TRACE(...)         ::Voltra::Log::GetClientLogger()->trace(__VA_ARGS__)
#define VOLTRA_INFO(...)          ::Voltra::Log::GetClientLogger()->info(__VA_ARGS__)
#define VOLTRA_WARN(...)          ::Voltra::Log::GetClientLogger()->warn(__VA_ARGS__)
#define VOLTRA_ERROR(...)         ::Voltra::Log::GetClientLogger()->error(__VA_ARGS__)
#define VOLTRA_FATAL(...)         ::Voltra::Log::GetClientLogger()->critical(__VA_ARGS__)