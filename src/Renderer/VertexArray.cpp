#include "VertexArray.hpp"

#include <glad/glad.h>
#include <iostream> 

namespace Voltra {

    /**
     * @brief Converts internal ShaderDataType to OpenGL types.
     * 
     * @param type The internal type.
     * @return The GLenum type.
     */
    static GLenum ShaderDataTypeToOpenGLBaseType(ShaderDataType type) {
        switch (type) {
            case ShaderDataType::Float:    return GL_FLOAT;
            case ShaderDataType::Float2:   return GL_FLOAT;
            case ShaderDataType::Float3:   return GL_FLOAT;
            case ShaderDataType::Float4:   return GL_FLOAT;
            case ShaderDataType::Mat3:     return GL_FLOAT;
            case ShaderDataType::Mat4:     return GL_FLOAT;
            case ShaderDataType::Int:      return GL_INT;
            case ShaderDataType::Int2:     return GL_INT;
            case ShaderDataType::Int3:     return GL_INT;
            case ShaderDataType::Int4:     return GL_INT;
            case ShaderDataType::Bool:     return GL_BOOL;
        }
        return 0;
    }

    /**
     * @brief Generates the OpenGL VAO.
     */
    VertexArray::VertexArray() {
        glGenVertexArrays(1, &m_RendererID);
    }

    /**
     * @brief Deletes the OpenGL VAO.
     */
    VertexArray::~VertexArray() {
        glDeleteVertexArrays(1, &m_RendererID);
    }

    /**
     * @brief Binds the VAO.
     */
    void VertexArray::Bind() const {
        glBindVertexArray(m_RendererID);
    }

    /**
     * @brief Unbinds the VAO.
     */
    void VertexArray::Unbind() const {
        glBindVertexArray(0);
    }

    /**
     * @brief Links a VertexBuffer to this VAO using the buffer's layout.
     * 
     * @param vertexBuffer The buffer to attach.
     */
    void VertexArray::AddVertexBuffer(const std::shared_ptr<VertexBuffer>& vertexBuffer) {
        glBindVertexArray(m_RendererID);
        vertexBuffer->Bind();

        const auto& layout = vertexBuffer->GetLayout();

        for (const auto& element : layout) {
            glEnableVertexAttribArray(m_VertexBufferIndex);
            glVertexAttribPointer(m_VertexBufferIndex,
                element.GetComponentCount(),
                ShaderDataTypeToOpenGLBaseType(element.Type),
                element.Normalized ? GL_TRUE : GL_FALSE,
                layout.GetStride(),
                (const void*)(uintptr_t)element.Offset);
            m_VertexBufferIndex++;
        }
        
        m_VertexBuffers.push_back(vertexBuffer);
    }

    /**
     * @brief Sets the global IndexBuffer for this VAO.
     * 
     * @param indexBuffer The index buffer.
     */
    void VertexArray::SetIndexBuffer(const std::shared_ptr<IndexBuffer>& indexBuffer) {
        glBindVertexArray(m_RendererID);
        indexBuffer->Bind();

        m_IndexBuffer = indexBuffer;
    }
    
    /**
     * @brief Factory creator.
     * @return New VertexArray instance.
     */
    std::shared_ptr<VertexArray> VertexArray::Create() {
        return std::make_shared<VertexArray>();
    }

}
