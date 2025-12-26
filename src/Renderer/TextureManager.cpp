#include "TextureManager.hpp"
#include "Core/Log.hpp"

namespace Voltra {

    std::unordered_map<std::string, std::shared_ptr<Texture2D>> TextureManager::s_Textures;
    std::shared_ptr<Texture2D> TextureManager::s_DefaultTexture;

    void TextureManager::Init() {
        // --- CREATE ERROR TEXTURE (1x1 pixel) ---
        uint32_t magenta = 0xff00ffff;
        
        s_DefaultTexture = std::make_shared<Texture2D>(1, 1);
        s_DefaultTexture->SetData(&magenta, sizeof(uint32_t));
        
        VOLTRA_CORE_INFO("TextureManager Initialized. Fallback texture created.");
    }

    std::shared_ptr<Texture2D> TextureManager::GetTexture(const std::string& path) {
        // Normalize path (convert "assets//player.png" to "assets/player.png")
        std::string clean_path = std::filesystem::path(path).string();

        // Search in cache
        auto it = s_Textures.find(clean_path);
        if (it != s_Textures.end()) {
            return it->second;
        }

        // If not in cache, try to load
        std::shared_ptr<Texture2D> new_texture = LoadTexture(clean_path);

        // If loading failed, use fallback texture
        if (!new_texture) {
            VOLTRA_CORE_ERROR("TextureManager: Failed to load texture '{0}'. Using fallback.", clean_path);
            return s_DefaultTexture; 
        }

        // Save and return
        s_Textures[clean_path] = new_texture;
        return new_texture;
    }

    std::shared_ptr<Texture2D> TextureManager::LoadTexture(const std::string& path) {
        // Check if file exists
        if (!std::filesystem::exists(path)) {
            VOLTRA_CORE_WARN("TextureManager: File does not exist: {0}", path);
            return nullptr; 
        }

        try {
            auto tex = std::make_shared<Texture2D>(path);
            
            // Verify if texture loaded correctly
            if (tex->GetWidth() == 0) return nullptr;
            
            VOLTRA_CORE_TRACE("TextureManager: Loaded '{0}'", path);
            return tex;
        } 
        catch (...) {
            return nullptr;
        }
    }

    void TextureManager::Clean() {
        s_Textures.clear();
        s_DefaultTexture.reset();
        VOLTRA_CORE_INFO("TextureManager: Cache cleaned.");
    }

}