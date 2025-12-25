#include "Texture.hpp"
#include "Core/Log.hpp"
#include <stb_image.h>

namespace Voltra {

    // Create a texture manually (Data/Pixel Art)
    Texture2D::Texture2D(uint32_t width, uint32_t height, TextureFilter filter)
        : m_Width(width), m_Height(height) {
        
        m_InternalFormat = GL_RGBA8;
        m_DataFormat = GL_RGBA;

        glCreateTextures(GL_TEXTURE_2D, 1, &m_RendererID);
        glTextureStorage2D(m_RendererID, 1, m_InternalFormat, m_Width, m_Height);

        // Apply the filter requested (By default Nearest)
        glTextureParameteri(m_RendererID, GL_TEXTURE_MIN_FILTER, (GLenum)filter);
        glTextureParameteri(m_RendererID, GL_TEXTURE_MAG_FILTER, (GLenum)filter);
        
        glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_S, GL_REPEAT);
        glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_T, GL_REPEAT);
        
        m_IsLoaded = true;
    }

    // Create a texture from an image file
    Texture2D::Texture2D(const std::string& path)
        : m_Path(path) {
        
        int width, height, channels;
        stbi_set_flip_vertically_on_load(1);
        stbi_uc* data = stbi_load(path.c_str(), &width, &height, &channels, 0);

        if (data) {
            m_IsLoaded = true;
            m_Width = width;
            m_Height = height;

            if (channels == 4) {
                m_InternalFormat = GL_RGBA8;
                m_DataFormat = GL_RGBA;
            } else if (channels == 3) {
                m_InternalFormat = GL_RGB8;
                m_DataFormat = GL_RGB;
            }

            glCreateTextures(GL_TEXTURE_2D, 1, &m_RendererID);
            glTextureStorage2D(m_RendererID, 1, m_InternalFormat, m_Width, m_Height);

            glTextureParameteri(m_RendererID, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTextureParameteri(m_RendererID, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

            glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_S, GL_REPEAT);
            glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_T, GL_REPEAT);

            glTextureSubImage2D(m_RendererID, 0, 0, 0, m_Width, m_Height, m_DataFormat, GL_UNSIGNED_BYTE, data);

            stbi_image_free(data);
        } else {
            VOLTRA_CORE_ERROR("Failed to load texture: {0}", path);
        }
    }

    Texture2D::~Texture2D() {
        glDeleteTextures(1, &m_RendererID);
    }

    void Texture2D::SetData(void* data, uint32_t size) {
        // Basic security check (4 bytes per pixel for RGBA)
        uint32_t bpp = m_DataFormat == GL_RGBA ? 4 : 3;
        if (size != m_Width * m_Height * bpp) {
            VOLTRA_CORE_ERROR("Data size must be entire texture!");
            return;
        }

        glTextureSubImage2D(m_RendererID, 0, 0, 0, m_Width, m_Height, m_DataFormat, GL_UNSIGNED_BYTE, data);
    }

    void Texture2D::SetFilter(TextureFilter minFilter, TextureFilter magFilter) {
        glTextureParameteri(m_RendererID, GL_TEXTURE_MIN_FILTER, (GLenum)minFilter);
        glTextureParameteri(m_RendererID, GL_TEXTURE_MAG_FILTER, (GLenum)magFilter);
    }

    void Texture2D::Bind(uint32_t slot) const {
        glBindTextureUnit(slot, m_RendererID);
    }

}