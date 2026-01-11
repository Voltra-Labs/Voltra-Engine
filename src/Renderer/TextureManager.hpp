#pragma once

#include "Core/Core.hpp"
#include "Texture.hpp"
#include <string>
#include <unordered_map>
#include <memory>
#include <filesystem>

namespace Voltra {

    /**
     * @brief Manages texture resources and caching.
     * 
     * Ensures textures are loaded only once and reused.
     */
    class VOLTRA_API TextureManager {
    public:
        TextureManager() = delete;

        /**
         * @brief Initializes the texture manager.
         * 
         * Creates a default fallback texture.
         */
        static void Init();

        /**
         * @brief Retrieves a texture from the cache or loads it.
         * 
         * @param path Filepath to the texture.
         * @return Shared pointer to the texture (or fallback if failed).
         */
        static std::shared_ptr<Texture2D> GetTexture(const std::string& path);

        /**
         * @brief Clears the texture cache.
         */
        static void Clean();

    private:
        /**
         * @brief Internally loads a texture from disk.
         * 
         * @param path Filepath.
         * @return Shared pointer to the loaded texture or nullptr.
         */
        static std::shared_ptr<Texture2D> LoadTexture(const std::string& path);

        static std::unordered_map<std::string, std::shared_ptr<Texture2D>> s_Textures;
        static std::shared_ptr<Texture2D> s_DefaultTexture; 
    };

}