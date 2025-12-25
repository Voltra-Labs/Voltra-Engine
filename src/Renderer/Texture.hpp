#pragma once

#include <string>
#include <glad/glad.h>

namespace Voltra {

    class Texture2D {
    public:
        Texture2D(const std::string& path);
        ~Texture2D();

        uint32_t GetWidth() const { return m_Width; }
        uint32_t GetHeight() const { return m_Height; }
        uint32_t GetRendererID() const { return m_RendererID; }

        void Bind(uint32_t slot = 0) const;

        bool IsLoaded() const { return m_IsLoaded; }

    private:
        std::string m_Path;
        uint32_t m_Width = 0, m_Height = 0;
        uint32_t m_RendererID = 0;
        GLenum m_InternalFormat = 0, m_DataFormat = 0;
        bool m_IsLoaded = false;
    };

}