#pragma once

#include "Texture.hpp"
#include <string>
#include <unordered_map>
#include <memory>
#include <filesystem>

namespace Voltra {

    class TextureManager {
    public:
        TextureManager() = delete;

        // Initializes the system and creates the default fallback texture
        static void Init();

        // Requests a texture. NEVER returns nullptr.
        static std::shared_ptr<Texture2D> GetTexture(const std::string& path);

        // Cleans up memory
        static void Clean();

    private:
        // Loads the texture from disk
        static std::shared_ptr<Texture2D> LoadTexture(const std::string& path);

        static std::unordered_map<std::string, std::shared_ptr<Texture2D>> s_Textures;
        static std::shared_ptr<Texture2D> s_DefaultTexture; 
    };

}