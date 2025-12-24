#include "Buffer.hpp"
#include <glad/glad.h>

namespace Voltra {

    // ==============================================================================
    // OpenGLVertexBuffer
    // ==============================================================================

    class OpenGLVertexBuffer : public VertexBuffer
    {
    public:
        OpenGLVertexBuffer(float* vertices, uint32_t size)
        {
            glCreateBuffers(1, &m_RendererID);
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
            glBufferData(GL_ARRAY_BUFFER, size, vertices, GL_STATIC_DRAW);
        }

        virtual ~OpenGLVertexBuffer()
        {
            glDeleteBuffers(1, &m_RendererID);
        }

        virtual void Bind() const override
        {
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
        }

        virtual void Unbind() const override
        {
            glBindBuffer(GL_ARRAY_BUFFER, 0);
        }

        virtual const BufferLayout& GetLayout() const override { return m_Layout; }
        virtual void SetLayout(const BufferLayout& layout) override { m_Layout = layout; }

    private:
        uint32_t m_RendererID;
        BufferLayout m_Layout;
    };

    // ==============================================================================
    // OpenGLIndexBuffer
    // ==============================================================================

    class OpenGLIndexBuffer : public IndexBuffer
    {
    public:
        OpenGLIndexBuffer(uint32_t* indices, uint32_t count)
            : m_Count(count)
        {
            glCreateBuffers(1, &m_RendererID);
            
            // GL_ELEMENT_ARRAY_BUFFER is not valid without an actively bound VAO used in many DSA cases
            // But since we are binding it immediately to upload data, standard binding is fine.
            // For Index Buffers, binding to GL_ELEMENT_ARRAY_BUFFER effectively binds it to the current VAO if one is bound.
            // We should just use GlBufferData with the specific target.
            
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_RendererID);
            glBufferData(GL_ELEMENT_ARRAY_BUFFER, count * sizeof(uint32_t), indices, GL_STATIC_DRAW);
        }

        virtual ~OpenGLIndexBuffer()
        {
            glDeleteBuffers(1, &m_RendererID);
        }

        virtual void Bind() const override
        {
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_RendererID);
        }

        virtual void Unbind() const override
        {
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);
        }

        virtual uint32_t GetCount() const override { return m_Count; }

    private:
        uint32_t m_RendererID;
        uint32_t m_Count;
    };

    // ==============================================================================
    // Factory Methods
    // ==============================================================================

    std::unique_ptr<VertexBuffer> VertexBuffer::Create(float* vertices, uint32_t size)
    {
        return std::make_unique<OpenGLVertexBuffer>(vertices, size);
    }

    std::unique_ptr<IndexBuffer> IndexBuffer::Create(uint32_t* indices, uint32_t count)
    {
        return std::make_unique<OpenGLIndexBuffer>(indices, count);
    }
}
