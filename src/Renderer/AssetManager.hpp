#pragma once

#include <string>
#include <memory>
#include <unordered_map>

namespace Voltra {

    class Texture2D;
    class Shader;

    /**
     * @brief Static AssetManager for unified resource handling.
     * 
     * Manages the loading and caching of textures and shaders to ensure
     * resources are unique in memory.
     */
    class AssetManager {
    public:
        /**
         * @brief Loads a Texture2D from the specified path.
         * 
         * If the texture is already loaded, it returns the cached instance.
         * Otherwise, it loads it from disk, caches it, and returns it.
         * 
         * @param path Relative or absolute path to the texture file.
         * @return std::shared_ptr<Texture2D> Pointer to the loaded texture.
         */
        static std::shared_ptr<Texture2D> LoadTexture(const std::string& path);

        /**
         * @brief Loads a Shader from the specified path.
         * 
         * If the shader is already loaded, it returns the cached instance.
         * Otherwise, it loads it from disk, caches it, and returns it.
         * 
         * @param path Relative or absolute path to the shader source file.
         * @return std::shared_ptr<Shader> Pointer to the loaded shader.
         */
        static std::shared_ptr<Shader> LoadShader(const std::string& path);

    private:
        static std::unordered_map<std::string, std::shared_ptr<Texture2D>> s_TextureCache;
        static std::unordered_map<std::string, std::shared_ptr<Shader>> s_ShaderCache;
    };

}
