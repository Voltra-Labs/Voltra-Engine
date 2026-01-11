#pragma once

// Platform detection using predefined macros
#ifdef _WIN32
    /* Windows x64/x86 */
    #ifdef _WIN64
        #define VOLTRA_PLATFORM_WINDOWS
    #else
        #error "x86 Builds are not supported!"
    #endif
#elif defined(__APPLE__) || defined(__MACH__)
    #include <TargetConditionals.h>
    /* TARGET_OS_MAC exists on all the platforms
     * so we must check all of them (in this order)
     * to ensure that we're running on MAC
     * and not some other Apple platform */
    #if TARGET_IPHONE_SIMULATOR == 1
        #error "IOS simulator is not supported!"
    #elif TARGET_OS_IPHONE == 1
        #error "IOS is not supported!"
    #elif TARGET_OS_MAC == 1
        #define VOLTRA_PLATFORM_MACOS
    #else
        #error "Unknown Apple Platform!"
    #endif
#elif defined(__ANDROID__)
    #define VOLTRA_PLATFORM_ANDROID
    #error "Android is not supported!"
#elif defined(__linux__)
    #define VOLTRA_PLATFORM_LINUX
#else
    /* Unknown compiler/platform */
    #error "Unknown platform!"
#endif

// DLL support
#ifdef VOLTRA_PLATFORM_WINDOWS
    #if defined(VOLTRA_BUILD_DLL)
        #define VOLTRA_API __declspec(dllexport)
    #else
        #define VOLTRA_API __declspec(dllimport)
    #endif
#elif defined(VOLTRA_PLATFORM_LINUX) || defined(VOLTRA_PLATFORM_MACOS)
    #if defined(VOLTRA_BUILD_DLL)
        #define VOLTRA_API __attribute__((visibility("default")))
    #else
        #define VOLTRA_API
    #endif
#else
    #error "Voltra currently only supports Windows, Linux and MacOS!"
#endif
