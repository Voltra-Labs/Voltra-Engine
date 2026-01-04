#pragma once

#include <memory>
#include "Buffer.hpp"
#include <vector>

namespace Voltra {

    /**
     * @brief Abstraction for OpenGL Vertex Array Objects (VAO).
     * 
     * Manages VertexBuffers and IndexBuffers relationships.
     */
    class VertexArray {
    public:
        /**
         * @brief Constructs a new VertexArray.
         */
        VertexArray();

        /**
         * @brief Destructor. Deletes the VAO.
         */
        ~VertexArray();

        /**
         * @brief Binds the VAO.
         */
        void Bind() const;

        /**
         * @brief Unbinds the VAO.
         */
        void Unbind() const;

        /**
         * @brief Adds a VertexBuffer to the VAO.
         * 
         * Automatically sets up vertex attributes based on buffer layout.
         * 
         * @param vertexBuffer The buffer to add.
         */
        void AddVertexBuffer(const std::shared_ptr<VertexBuffer>& vertexBuffer);

        /**
         * @brief Sets the IndexBuffer for the VAO.
         * 
         * @param indexBuffer The index buffer.
         */
        void SetIndexBuffer(const std::shared_ptr<IndexBuffer>& indexBuffer);

        /**
         * @brief Gets the list of attached vertex buffers.
         * @return Vector of vertex buffers.
         */
        const std::vector<std::shared_ptr<VertexBuffer>>& GetVertexBuffers() const { return m_VertexBuffers; }

        /**
         * @brief Gets the attached index buffer.
         * @return The index buffer.
         */
        const std::shared_ptr<IndexBuffer>& GetIndexBuffer() const { return m_IndexBuffer; }

        /**
         * @brief Factory method to create a VertexArray.
         * @return Shared pointer to the new VAO.
         */
        static std::shared_ptr<VertexArray> Create();
    private:
        uint32_t m_RendererID;
        uint32_t m_VertexBufferIndex = 0;
        std::vector<std::shared_ptr<VertexBuffer>> m_VertexBuffers;
        std::shared_ptr<IndexBuffer> m_IndexBuffer;
    };

}
