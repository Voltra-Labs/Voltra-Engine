#include "Texture.hpp"
#include "Core/Log.hpp"
#include <stb_image.h>

namespace Voltra {

    Texture2D::Texture2D(const std::string& path)
        : m_Path(path) {
        
        int width, height, channels;
        // Invert the image vertically because OpenGL expects the origin (0,0) at bottom-left
        stbi_set_flip_vertically_on_load(1);
        
        stbi_uc* data = stbi_load(path.c_str(), &width, &height, &channels, 0);

        if (data) {
            m_IsLoaded = true;
            m_Width = width;
            m_Height = height;

            // Determine format (RGB vs RGBA)
            if (channels == 4) {
                m_InternalFormat = GL_RGBA8;
                m_DataFormat = GL_RGBA;
            } else if (channels == 3) {
                m_InternalFormat = GL_RGB8;
                m_DataFormat = GL_RGB;
            }

            // Create texture in OpenGL
            glCreateTextures(GL_TEXTURE_2D, 1, &m_RendererID);
            glTextureStorage2D(m_RendererID, 1, m_InternalFormat, m_Width, m_Height);

            // Configure parameters (Pixelated scaling for pixel art, smooth for HD)
            glTextureParameteri(m_RendererID, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTextureParameteri(m_RendererID, GL_TEXTURE_MAG_FILTER, GL_LINEAR); // Use GL_NEAREST for pixel art

            glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_S, GL_REPEAT);
            glTextureParameteri(m_RendererID, GL_TEXTURE_WRAP_T, GL_REPEAT);

            // Upload data
            glTextureSubImage2D(m_RendererID, 0, 0, 0, m_Width, m_Height, m_DataFormat, GL_UNSIGNED_BYTE, data);

            // Free CPU memory
            stbi_image_free(data);
        } else {
            VOLTRA_CORE_ERROR("Failed to load texture: {0}", path);
        }
    }

    Texture2D::~Texture2D() {
        glDeleteTextures(1, &m_RendererID);
    }

    void Texture2D::Bind(uint32_t slot) const {
        glBindTextureUnit(slot, m_RendererID);
    }

}