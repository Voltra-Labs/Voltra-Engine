#pragma once

#include "Core/Core.hpp"
#include <vector>
#include <string>
#include <memory>
#include <cstdint>
#include <cassert>
#include <initializer_list>

namespace Voltra {

    /**
     * @brief Enum representing supported shader data types.
     */
    enum class ShaderDataType {
        None = 0, Float, Float2, Float3, Float4, Mat3, Mat4, Int, Int2, Int3, Int4, Bool
    };

    /**
     * @brief Gets the size in bytes of a specific ShaderDataType.
     * 
     * @param type The shader data type.
     * @return The size in bytes.
     */
    static uint32_t ShaderDataTypeSize(ShaderDataType type) {
        switch (type) {
            case ShaderDataType::Float:    return 4;
            case ShaderDataType::Float2:   return 4 * 2;
            case ShaderDataType::Float3:   return 4 * 3;
            case ShaderDataType::Float4:   return 4 * 4;
            case ShaderDataType::Mat3:     return 4 * 3 * 3;
            case ShaderDataType::Mat4:     return 4 * 4 * 4;
            case ShaderDataType::Int:      return 4;
            case ShaderDataType::Int2:     return 4 * 2;
            case ShaderDataType::Int3:     return 4 * 3;
            case ShaderDataType::Int4:     return 4 * 4;
            case ShaderDataType::Bool:     return 1;
        }

        assert(false && "Unknown ShaderDataType!");
        return 0;
    }

    /**
     * @brief Represents a single element in a buffer layout.
     */
    struct BufferElement {
        std::string Name;
        ShaderDataType Type;
        uint32_t Size;
        size_t Offset;
        bool Normalized;

        /**
         * @brief Default constructor.
         */
        BufferElement() = default;

        /**
         * @brief Constructs a BufferElement.
         * 
         * @param type The data type of the element.
         * @param name The name of the element.
         * @param normalized Whether the data should be normalized.
         */
        BufferElement(ShaderDataType type, const std::string& name, bool normalized = false)
            : Name(name), Type(type), Size(ShaderDataTypeSize(type)), Offset(0), Normalized(normalized) {}

        /**
         * @brief Gets the number of components in the element type.
         * 
         * @return Component count (e.g., Float3 -> 3).
         */
        uint32_t GetComponentCount() const {
            switch (Type) {
                case ShaderDataType::Float:   return 1;
                case ShaderDataType::Float2:  return 2;
                case ShaderDataType::Float3:  return 3;
                case ShaderDataType::Float4:  return 4;
                case ShaderDataType::Mat3:    return 3 * 3;
                case ShaderDataType::Mat4:    return 4 * 4;
                case ShaderDataType::Int:     return 1;
                case ShaderDataType::Int2:    return 2;
                case ShaderDataType::Int3:    return 3;
                case ShaderDataType::Int4:    return 4;
                case ShaderDataType::Bool:    return 1;
            }

            assert(false && "Unknown ShaderDataType!");
            return 0;
        }
    };

    /**
     * @brief Describes the layout of a VertexBuffer.
     * 
     * Manages a list of BufferElements and calculates strides and offsets.
     */
    class VOLTRA_API BufferLayout {
    public:
        /**
         * @brief Default constructor.
         */
        BufferLayout() {}

        /**
         * @brief Constructs a BufferLayout from a list of elements.
         * 
         * @param elements Initializer list of BufferElements.
         */
        BufferLayout(const std::initializer_list<BufferElement>& elements) : m_Elements(elements) {
            CalculateOffsetsAndStride();
        }

        /**
         * @brief Gets the stride of the layout.
         * 
         * @return stride in bytes.
         */
        inline uint32_t GetStride() const { return m_Stride; }

        /**
         * @brief Gets the list of elements.
         * 
         * @return Vector of BufferElements.
         */
        inline const std::vector<BufferElement>& GetElements() const { return m_Elements; }

        std::vector<BufferElement>::iterator begin() { return m_Elements.begin(); }
        std::vector<BufferElement>::iterator end() { return m_Elements.end(); }
        std::vector<BufferElement>::const_iterator begin() const { return m_Elements.begin(); }
        std::vector<BufferElement>::const_iterator end() const { return m_Elements.end(); }

    private:
        void CalculateOffsetsAndStride() {
            size_t offset = 0;
            m_Stride = 0;

            for (auto& element : m_Elements) {
                element.Offset = offset;
                offset += element.Size;
                m_Stride += element.Size;
            }
        }

    private:
        std::vector<BufferElement> m_Elements;
        uint32_t m_Stride = 0;
    };

    /**
     * @brief Interface for a Vertex Buffer.
     */
    class VOLTRA_API VertexBuffer {
    public:
        /**
         * @brief Virtual destructor.
         */
        virtual ~VertexBuffer() = default;

        /**
         * @brief Binds the buffer.
         */
        virtual void Bind() const = 0;

        /**
         * @brief Unbinds the buffer.
         */
        virtual void Unbind() const = 0;

        /**
         * @brief Gets the layout of the buffer.
         * 
         * @return The BufferLayout.
         */
        virtual const BufferLayout& GetLayout() const = 0;

        /**
         * @brief Sets the layout of the buffer.
         * 
         * @param layout The new layout.
         */
        virtual void SetLayout(const BufferLayout& layout) = 0;

        /**
         * @brief Updates the buffer data.
         * 
         * @param data Pointer to the new data.
         * @param size Size of the data in bytes.
         */
        virtual void SetData(const void* data, uint32_t size) = 0;

        /**
         * @brief Creates a VertexBuffer.
         * 
         * @param vertices Pointer to vertex data.
         * @param size Size of the data in bytes.
         * @return Unique pointer to the created buffer.
         */
        static std::unique_ptr<VertexBuffer> Create(float* vertices, uint32_t size);

        /**
         * @brief Creates a dynamic VertexBuffer.
         * 
         * @param size Size of the buffer in bytes.
         * @return Unique pointer to the created buffer.
         */
        static std::unique_ptr<VertexBuffer> Create(uint32_t size);
    };

    /**
     * @brief Interface for an Index Buffer.
     */
    class VOLTRA_API IndexBuffer {
    public:
        /**
         * @brief Virtual destructor.
         */
        virtual ~IndexBuffer() = default;

        /**
         * @brief Binds the buffer.
         */
        virtual void Bind() const = 0;

        /**
         * @brief Unbinds the buffer.
         */
        virtual void Unbind() const = 0;

        /**
         * @brief Gets the number of indices in the buffer.
         * 
         * @return Count of indices.
         */
        virtual uint32_t GetCount() const = 0;

        /**
         * @brief Creates an IndexBuffer.
         * 
         * @param indices Pointer to index data.
         * @param count Number of indices.
         * @return Unique pointer to the created buffer.
         */
        static std::unique_ptr<IndexBuffer> Create(uint32_t* indices, uint32_t count);
    };

}
