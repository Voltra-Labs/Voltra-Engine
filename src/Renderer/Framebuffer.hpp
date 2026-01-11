#pragma once

#include "Core/Core.hpp"
#include <memory>
#include <cstdint>

namespace Voltra {

    /**
     * @brief Configuration for a Framebuffer.
     */
    struct VOLTRA_API FramebufferSpecification {
        uint32_t Width, Height;
    };

    /**
     * @brief OpenGL Framebuffer abstraction.
     * 
     * Manages color and depth attachments.
     */
    class VOLTRA_API Framebuffer {
    public:
        /**
         * @brief Constructs a Framebuffer.
         * 
         * @param spec The configuration specification.
         */
        Framebuffer(const FramebufferSpecification& spec);

        /**
         * @brief Destroys the Framebuffer.
         */
        ~Framebuffer();

        /**
         * @brief Recreates the framebuffer attachments.
         */
        void Invalidate();

        /**
         * @brief Binds the framebuffer.
         */
        void Bind();

        /**
         * @brief Unbinds the framebuffer.
         */
        void Unbind();

        /**
         * @brief Resizes the framebuffer attachments.
         * 
         * @param width New width.
         * @param height New height.
         */
        void Resize(uint32_t width, uint32_t height);
        
        /**
         * @brief Gets the renderer ID of the color attachment.
         * 
         * @return The texture ID.
         */
        uint32_t GetColorAttachmentRendererID() const { return m_ColorAttachment; }

        /**
         * @brief Gets the framebuffer specification.
         * 
         * @return The specification.
         */
        const FramebufferSpecification& GetSpecification() const { return m_Specification; }

        /**
         * @brief Creates a Framebuffer.
         * 
         * @param spec Configuration.
         * @return Shared pointer to the new Framebuffer.
         */
        static std::shared_ptr<Framebuffer> Create(const FramebufferSpecification& spec);

    private:
        uint32_t m_RendererID = 0;
        uint32_t m_ColorAttachment = 0;
        uint32_t m_DepthAttachment = 0;
        FramebufferSpecification m_Specification;
    };

}
