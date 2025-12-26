#pragma once

#include <string>
#include <glad/glad.h>

namespace Voltra {

    enum class TextureFilter {
        Linear = GL_LINEAR,
        Nearest = GL_NEAREST
    };

    class Texture2D {
    public:
        // Constructor for load from file
        Texture2D(const std::string& path);
        // Constructor for create texture manually
        Texture2D(uint32_t width, uint32_t height, TextureFilter filter = TextureFilter::Nearest);

        ~Texture2D();

        uint32_t GetWidth() const { return m_Width; }
        uint32_t GetHeight() const { return m_Height; }
        uint32_t GetRendererID() const { return m_RendererID; }

        void Bind(uint32_t slot = 0) const;

        // Upload data to texture
        void SetData(void* data, uint32_t size);

        // Allows to change the filter in runtime
        void SetFilter(TextureFilter minFilter, TextureFilter magFilter);

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