#pragma once

#include <string>
#include <glad/glad.h>

namespace Voltra {

    /**
     * @brief Texture filtering modes.
     */
    enum class TextureFilter {
        Linear = GL_LINEAR,
        Nearest = GL_NEAREST
    };

    /**
     * @brief 2D Texture abstraction.
     * 
     * Handles loading from files or manual creation.
     */
    class Texture2D {
    public:
        /**
         * @brief Loads a texture from a file.
         * 
         * @param path The filepath to the image.
         */
        Texture2D(const std::string& path);

        /**
         * @brief Creates a texture manually.
         * 
         * @param width Width in pixels.
         * @param height Height in pixels.
         * @param filter Initial filter mode.
         */
        Texture2D(uint32_t width, uint32_t height, TextureFilter filter = TextureFilter::Nearest);

        /**
         * @brief Destructor. Deletes the texture.
         */
        ~Texture2D();

        /**
         * @brief Gets the texture width.
         * @return Width in pixels.
         */
        uint32_t GetWidth() const { return m_Width; }

        /**
         * @brief Gets the texture height.
         * @return Height in pixels.
         */
        uint32_t GetHeight() const { return m_Height; }

        /**
         * @brief Gets the OpenGL renderer ID.
         * @return The ID.
         */
        uint32_t GetRendererID() const { return m_RendererID; }

        /**
         * @brief Binds the texture to a slot.
         * 
         * @param slot Texture unit slot.
         */
        void Bind(uint32_t slot = 0) const;

        /**
         * @brief Uploads data to the texture.
         * 
         * @param data Pointer to pixel data.
         * @param size Size in bytes.
         */
        void SetData(void* data, uint32_t size);

        /**
         * @brief Updates the texture filter mode.
         * 
         * @param minFilter Minification filter.
         * @param magFilter Magnification filter.
         */
        void SetFilter(TextureFilter minFilter, TextureFilter magFilter);

        /**
         * @brief Checks if the texture is successfully loaded.
         * @return true if loaded, false otherwise.
         */
        bool IsLoaded() const { return m_IsLoaded; }

        bool operator==(const Texture2D& other) const { return m_RendererID == other.m_RendererID; }

    private:
        std::string m_Path;
        uint32_t m_Width = 0, m_Height = 0;
        uint32_t m_RendererID = 0;
        GLenum m_InternalFormat = 0, m_DataFormat = 0;
        bool m_IsLoaded = false;
    };

}