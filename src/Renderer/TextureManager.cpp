#include "TextureManager.h"
#include <iostream>

std::unordered_map<std::string, std::shared_ptr<Texture2D>> TextureManager::s_Textures;
std::shared_ptr<Texture2D> TextureManager::s_DefaultTexture;

void TextureManager::Init() {
    // --- CREATE ERROR TEXTURE (1x1 pixel) ---

    unsigned char data[] = { 255, 0, 255, 255 }; 
    s_DefaultTexture = std::make_shared<Texture2D>(1, 1, data);
    
    LOG_CORE_INFO("TextureManager Initialized. Fallback texture created.");
}

std::shared_ptr<Texture2D> TextureManager::GetTexture(const std::string& path) {
    // Normalize path (convert "assets//player.png" to "assets/player.png")
    std::string cleanPath = std::filesystem::path(path).string();

    // Search in cache
    auto it = s_Textures.find(cleanPath);
    if (it != s_Textures.end()) {
        return it->second;
    }

    // If not in cache, try to load
    std::shared_ptr<Texture2D> newTexture = LoadTexture(cleanPath);

    // If loading failed (returns nullptr), use fallback texture
    if (!newTexture) {
        LOG_CORE_ERROR("TextureManager: Failed to load texture '{0}'. Using fallback.", cleanPath);
        return s_DefaultTexture; 
    }

    // Save and return
    s_Textures[cleanPath] = newTexture;
    return newTexture;
}

std::shared_ptr<Texture2D> TextureManager::LoadTexture(const std::string& path) {
    // Verify file exists before loading
    if (!std::filesystem::exists(path)) {
        LOG_CORE_WARN("TextureManager: File does not exist: {0}", path);
        return nullptr; 
    }

    // Try to load
    try {
        auto tex = std::make_shared<Texture2D>(path);
        // Verify if texture loaded correctly
        if (tex->GetWidth() == 0) return nullptr;
        
        LOG_CORE_TRACE("TextureManager: Loaded '{0}'", path);
        return tex;
    } 
    catch (...) {
        return nullptr;
    }
}

void TextureManager::Clean() {
    s_Textures.clear();
    s_DefaultTexture.reset();
    LOG_CORE_INFO("TextureManager: Cache cleaned.");
}