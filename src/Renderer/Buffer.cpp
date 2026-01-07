#include "Buffer.hpp"
#include <glad/glad.h>

namespace Voltra {

    /**
     * @brief OpenGL implementation of a VertexBuffer.
     */
    class OpenGLVertexBuffer : public VertexBuffer
    {
    public:
        /**
         * @brief Constructs an OpenGLVertexBuffer.
         * 
         * @param vertices Data pointer.
         * @param size Size in bytes.
         */
        OpenGLVertexBuffer(float* vertices, uint32_t size) {
            glCreateBuffers(1, &m_RendererID);
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
            glBufferData(GL_ARRAY_BUFFER, size, vertices, GL_STATIC_DRAW);
        }

        /**
         * @brief Constructs a dynamic OpenGLVertexBuffer.
         * 
         * @param size Size in bytes.
         */
        OpenGLVertexBuffer(uint32_t size) {
            glCreateBuffers(1, &m_RendererID);
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
            glBufferData(GL_ARRAY_BUFFER, size, nullptr, GL_DYNAMIC_DRAW);
        }

        /**
         * @brief Destructor. Deletes the buffer.
         */
        virtual ~OpenGLVertexBuffer() {
            glDeleteBuffers(1, &m_RendererID);
        }

        /**
         * @brief Binds the OpenGL buffer.
         */
        virtual void Bind() const override {
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
        }

        /**
         * @brief Unbinds the OpenGL buffer.
         */
        virtual void Unbind() const override {
            glBindBuffer(GL_ARRAY_BUFFER, 0);
        }

        virtual void SetData(const void* data, uint32_t size) override {
            glBindBuffer(GL_ARRAY_BUFFER, m_RendererID);
            glBufferSubData(GL_ARRAY_BUFFER, 0, size, data);
        }

        virtual const BufferLayout& GetLayout() const override { return m_Layout; }
        virtual void SetLayout(const BufferLayout& layout) override { m_Layout = layout; }

    private:
        uint32_t m_RendererID;
        BufferLayout m_Layout;
    };

    /**
     * @brief OpenGL implementation of an IndexBuffer.
     */
    class OpenGLIndexBuffer : public IndexBuffer
    {
    public:
        /**
         * @brief Constructs an OpenGLIndexBuffer.
         * 
         * @param indices Index data pointer.
         * @param count Number of indices.
         */
        OpenGLIndexBuffer(uint32_t* indices, uint32_t count) : m_Count(count) {
            glCreateBuffers(1, &m_RendererID);
            
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_RendererID);
            glBufferData(GL_ELEMENT_ARRAY_BUFFER, count * sizeof(uint32_t), indices, GL_STATIC_DRAW);
        }

        /**
         * @brief Destructor. Deletes the buffer.
         */
        virtual ~OpenGLIndexBuffer() {
            glDeleteBuffers(1, &m_RendererID);
        }

        /**
         * @brief Binds the OpenGL buffer.
         */
        virtual void Bind() const override {
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_RendererID);
        }

        /**
         * @brief Unbinds the OpenGL buffer.
         */
        virtual void Unbind() const override {
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);
        }

        virtual uint32_t GetCount() const override { return m_Count; }

    private:
        uint32_t m_RendererID;
        uint32_t m_Count;
    };

    /**
     * @brief Factory method to create a vertex buffer.
     * 
     * @param vertices Data pointer.
     * @param size Size in bytes.
     * @return Unique ptr to the created VertexBuffer.
     */
    std::unique_ptr<VertexBuffer> VertexBuffer::Create(float* vertices, uint32_t size) {
        return std::make_unique<OpenGLVertexBuffer>(vertices, size);
    }

    /**
     * @brief Factory method to create a dynamic vertex buffer.
     * 
     * @param size Size in bytes.
     * @return Unique ptr to the created VertexBuffer.
     */
    std::unique_ptr<VertexBuffer> VertexBuffer::Create(uint32_t size) {
        return std::make_unique<OpenGLVertexBuffer>(size);
    }

    /**
     * @brief Factory method to create an index buffer.
     * 
     * @param indices Index data pointer.
     * @param count Number of indices.
     * @return Unique ptr to the created IndexBuffer.
     */
    std::unique_ptr<IndexBuffer> IndexBuffer::Create(uint32_t* indices, uint32_t count) {
        return std::make_unique<OpenGLIndexBuffer>(indices, count);
    }
}
