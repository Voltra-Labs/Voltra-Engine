#include "TextureManager.hpp"
#include "Core/Log.hpp"

namespace Voltra {

    std::unordered_map<std::string, std::shared_ptr<Texture2D>> TextureManager::s_Textures;
    std::shared_ptr<Texture2D> TextureManager::s_DefaultTexture;

    /**
     * @brief Initializes the manager and creates the default 1x1 magenta texture.
     */
    void TextureManager::Init() {
        uint32_t magenta = 0xff00ffff;
        
        s_DefaultTexture = std::make_shared<Texture2D>(1, 1);
        s_DefaultTexture->SetData(&magenta, sizeof(uint32_t));
        
        VOLTRA_CORE_INFO("TextureManager Initialized. Fallback texture created.");
    }

    /**
     * @brief Looks up a texture by path, loading it if necessary.
     * 
     * @param path The file path.
     * @return The cached or new texture.
     */
    std::shared_ptr<Texture2D> TextureManager::GetTexture(const std::string& path) {
        std::string clean_path = std::filesystem::path(path).string();

        auto it = s_Textures.find(clean_path);
        if (it != s_Textures.end()) {
            return it->second;
        }

        std::shared_ptr<Texture2D> new_texture = LoadTexture(clean_path);

        if (!new_texture) {
            VOLTRA_CORE_ERROR("TextureManager: Failed to load texture '{0}'. Using fallback.", clean_path);
            return s_DefaultTexture; 
        }

        s_Textures[clean_path] = new_texture;
        return new_texture;
    }

    /**
     * @brief Helper to load a texture file safely.
     * 
     * @param path The file path.
     * @return The texture instance or nullptr on failure.
     */
    std::shared_ptr<Texture2D> TextureManager::LoadTexture(const std::string& path) {
        if (!std::filesystem::exists(path)) {
            VOLTRA_CORE_WARN("TextureManager: File does not exist: {0}", path);
            return nullptr; 
        }

        try {
            auto tex = std::make_shared<Texture2D>(path);
            
            if (tex->GetWidth() == 0) return nullptr;
            
            VOLTRA_CORE_TRACE("TextureManager: Loaded '{0}'", path);
            return tex;
        } 
        catch (...) {
            return nullptr;
        }
    }

    /**
     * @brief Empties the texture cache and releases resources.
     */
    void TextureManager::Clean() {
        s_Textures.clear();
        s_DefaultTexture.reset();
        VOLTRA_CORE_INFO("TextureManager: Cache cleaned.");
    }

}