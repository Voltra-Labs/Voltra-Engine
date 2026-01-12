#include "Log.hpp"
#include <spdlog/sinks/stdout_color_sinks.h>

namespace Voltra {

    std::shared_ptr<spdlog::logger> Log::s_CoreLogger;
    std::shared_ptr<spdlog::logger> Log::s_ClientLogger;

    /**
     * @brief Initializes the logging subsystem.
     * 
     * Sets the log pattern to "[Time] Name: Message" with colors.
     * Creates "VOLTRA" and "APP" loggers.
     */
    void Log::Init() {
        spdlog::set_pattern("%^[%T] %n: %v%$");

        s_CoreLogger = spdlog::stdout_color_mt("VOLTRA");
        s_CoreLogger->set_level(spdlog::level::trace);

        s_ClientLogger = spdlog::stdout_color_mt("APP");
        s_ClientLogger->set_level(spdlog::level::trace);
    }

    void Log::Shutdown() {
        s_ClientLogger.reset();
        s_CoreLogger.reset();
        spdlog::shutdown();
    }

}