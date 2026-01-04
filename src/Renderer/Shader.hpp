#pragma once

#include <string>
#include <glm/glm.hpp>

namespace Voltra {

    /**
     * @brief OpenGL Shader program wrapper.
     * 
     * Handles compilation, linking, and uniform management.
     */
    class Shader {
    public:
        /**
         * @brief Constructs a shader from source code strings.
         * 
         * @param vertexSrc Vertex shader source.
         * @param fragmentSrc Fragment shader source.
         */
        Shader(const std::string& vertexSrc, const std::string& fragmentSrc);

        /**
         * @brief Destructor. Deletes the shader program.
         */
        ~Shader();

        /**
         * @brief Binds this shader program.
         */
        void Bind() const;

        /**
         * @brief Unbinds this shader program.
         */
        void Unbind() const;

        /**
         * @brief Uploads an integer uniform.
         * 
         * @param name Uniform name.
         * @param value Integer value.
         */
        void UploadUniformInt(const std::string& name, int value);

        /**
         * @brief Uploads a float uniform.
         * 
         * @param name Uniform name.
         * @param value Float value.
         */
        void UploadUniformFloat(const std::string& name, float value);

        /**
         * @brief Uploads a vec2 uniform.
         * 
         * @param name Uniform name.
         * @param value Vec2 value.
         */
        void UploadUniformFloat2(const std::string& name, const glm::vec2& value);

        /**
         * @brief Uploads a vec3 uniform.
         * 
         * @param name Uniform name.
         * @param value Vec3 value.
         */
        void UploadUniformFloat3(const std::string& name, const glm::vec3& value);

        /**
         * @brief Uploads a vec4 uniform.
         * 
         * @param name Uniform name.
         * @param value Vec4 value.
         */
        void UploadUniformFloat4(const std::string& name, const glm::vec4& value);
        
        /**
         * @brief Uploads a mat3 uniform.
         * 
         * @param name Uniform name.
         * @param matrix 3x3 Matrix.
         */
        void UploadUniformMat3(const std::string& name, const glm::mat3& matrix);

        /**
         * @brief Uploads a mat4 uniform.
         * 
         * @param name Uniform name.
         * @param matrix 4x4 Matrix.
         */
        void UploadUniformMat4(const std::string& name, const glm::mat4& matrix);

    private:
        uint32_t m_RendererID;
    };

}