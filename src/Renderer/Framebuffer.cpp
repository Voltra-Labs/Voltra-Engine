#include "Framebuffer.hpp"

#include <glad/glad.h>
#include "Core/Log.hpp"

namespace Voltra {

    /**
     * @brief Constructs the Framebuffer and initializes it.
     * 
     * @param spec Configuration specification.
     */
    Framebuffer::Framebuffer(const FramebufferSpecification& spec)
        : m_Specification(spec) {
        Invalidate();
    }

    /**
     * @brief Destructor. Clean up OpenGL resources.
     */
    Framebuffer::~Framebuffer() {
        glDeleteFramebuffers(1, &m_RendererID);
        glDeleteTextures(1, &m_ColorAttachment);
        glDeleteTextures(1, &m_DepthAttachment);
    }

    /**
     * @brief Recreates the framebuffer and its attachments.
     * 
     * Handles deletion of old resources if they exist.
     */
    void Framebuffer::Invalidate() {
        if (m_RendererID) {
            glDeleteFramebuffers(1, &m_RendererID);
            glDeleteTextures(1, &m_ColorAttachment);
            glDeleteTextures(1, &m_DepthAttachment);
        }

        glGenFramebuffers(1, &m_RendererID);
        glBindFramebuffer(GL_FRAMEBUFFER, m_RendererID);

        glGenTextures(1, &m_ColorAttachment);
        glBindTexture(GL_TEXTURE_2D, m_ColorAttachment);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, m_Specification.Width, m_Specification.Height, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
        
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

        glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, m_ColorAttachment, 0);

        glGenTextures(1, &m_DepthAttachment);
        glBindTexture(GL_TEXTURE_2D, m_DepthAttachment);
        glTexStorage2D(GL_TEXTURE_2D, 1, GL_DEPTH24_STENCIL8, m_Specification.Width, m_Specification.Height);
        
        glFramebufferTexture2D(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_TEXTURE_2D, m_DepthAttachment, 0);

        if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
            VOLTRA_CORE_ERROR("Framebuffer is incomplete!");

        glBindFramebuffer(GL_FRAMEBUFFER, 0);
    }

    /**
     * @brief Binds the framebuffer and sets the viewport.
     */
    void Framebuffer::Bind() {
        glBindFramebuffer(GL_FRAMEBUFFER, m_RendererID);
        glViewport(0, 0, m_Specification.Width, m_Specification.Height);
    }

    /**
     * @brief Unbinds the framebuffer (binds default 0).
     */
    void Framebuffer::Unbind() {
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
    }

    /**
     * @brief Resizes the framebuffer.
     * 
     * @param width New width.
     * @param height New height.
     */
    void Framebuffer::Resize(uint32_t width, uint32_t height) {
        if (width == 0 || height == 0 || width > 8192 || height > 8192) {
            VOLTRA_CORE_WARN("Attempted to resize framebuffer to {0}, {1}", width, height);
            return;
        }

        m_Specification.Width = width;
        m_Specification.Height = height;
        
        Invalidate();
    }

    /**
     * @brief Factory method to create a Framebuffer.
     * 
     * @param spec Configuration.
     * @return Shared pointer to the new Framebuffer.
     */
    std::shared_ptr<Framebuffer> Framebuffer::Create(const FramebufferSpecification& spec) {
        return std::make_shared<Framebuffer>(spec);
    }

}
