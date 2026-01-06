#include "AssetManager.hpp"
#include "Texture.hpp"
#include "Shader.hpp"
#include "Core/Log.hpp"

namespace Voltra {

    std::unordered_map<std::string, std::shared_ptr<Texture2D>> AssetManager::s_TextureCache;
    std::unordered_map<std::string, std::shared_ptr<Shader>> AssetManager::s_ShaderCache;

    /**
     * @brief Loads a Texture2D from the specified path.
     * 
     * If the texture is already loaded, it returns the cached instance.
     * Otherwise, it loads it from disk, caches it, and returns it.
     * 
     * @param path Relative or absolute path to the texture file.
     * @return std::shared_ptr<Texture2D> Pointer to the loaded texture.
     */
    std::shared_ptr<Texture2D> AssetManager::LoadTexture(const std::string& path) {
        if (s_TextureCache.find(path) != s_TextureCache.end()) {
            return s_TextureCache[path];
        }

        VOLTRA_CORE_INFO("Loading Texture: {}", path);

        // Assume Texture2D::Create(path) exists as per instructions
        std::shared_ptr<Texture2D> texture = Texture2D::Create(path);
        s_TextureCache[path] = texture;
        return texture;
    }

    /**
     * @brief Loads a Shader from the specified path.
     * 
     * If the shader is already loaded, it returns the cached instance.
     * Otherwise, it loads it from disk, caches it, and returns it.
     * 
     * @param path Relative or absolute path to the shader source file.
     * @return std::shared_ptr<Shader> Pointer to the loaded shader.
     */
    std::shared_ptr<Shader> AssetManager::LoadShader(const std::string& path) {
        if (s_ShaderCache.find(path) != s_ShaderCache.end()) {
            return s_ShaderCache[path];
        }

        VOLTRA_CORE_INFO("Loading Shader: {}", path);

        std::shared_ptr<Shader> shader = Shader::Create(path);
        s_ShaderCache[path] = shader;
        return shader;
    }

    /**
     * @brief Clears the internal cache.
     */
    void AssetManager::Clear() {
        s_TextureCache.clear();
        s_ShaderCache.clear();
    }

}
