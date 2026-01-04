#include "Shader.hpp"

#include <glad/glad.h>
#include <vector>
#include "../Core/Log.hpp"
#include <glm/gtc/type_ptr.hpp>

namespace Voltra {

    /**
     * @brief Compiles and links vertex and fragment shaders.
     * 
     * Checks for compilation and linking errors, logging them if present.
     * 
     * @param vertexSrc The source code for the vertex shader.
     * @param fragmentSrc The source code for the fragment shader.
     */
    Shader::Shader(const std::string& vertexSrc, const std::string& fragmentSrc) {
        GLuint vertexShader = glCreateShader(GL_VERTEX_SHADER);

        const GLchar* source = vertexSrc.c_str();
        glShaderSource(vertexShader, 1, &source, 0);

        glCompileShader(vertexShader);

        GLint isCompiled = 0;
        glGetShaderiv(vertexShader, GL_COMPILE_STATUS, &isCompiled);
        if(isCompiled == GL_FALSE) {
            GLint maxLength = 0;
            glGetShaderiv(vertexShader, GL_INFO_LOG_LENGTH, &maxLength);

            std::vector<GLchar> infoLog(maxLength);
            glGetShaderInfoLog(vertexShader, maxLength, &maxLength, &infoLog[0]);
            
            glDeleteShader(vertexShader);

            VOLTRA_CORE_ERROR("{0}", infoLog.data());
            VOLTRA_CORE_FATAL("Vertex shader compilation failure!");
            return;
        }

        GLuint fragmentShader = glCreateShader(GL_FRAGMENT_SHADER);

        source = fragmentSrc.c_str();
        glShaderSource(fragmentShader, 1, &source, 0);

        glCompileShader(fragmentShader);

        glGetShaderiv(fragmentShader, GL_COMPILE_STATUS, &isCompiled);
        if (isCompiled == GL_FALSE) {
            GLint maxLength = 0;
            glGetShaderiv(fragmentShader, GL_INFO_LOG_LENGTH, &maxLength);

            std::vector<GLchar> infoLog(maxLength);
            glGetShaderInfoLog(fragmentShader, maxLength, &maxLength, &infoLog[0]);
            
            glDeleteShader(vertexShader);
            glDeleteShader(fragmentShader);

            VOLTRA_CORE_ERROR("{0}", infoLog.data());
            VOLTRA_CORE_FATAL("Fragment shader compilation failure!");
            return;
        }

        m_RendererID = glCreateProgram();
        glAttachShader(m_RendererID, vertexShader);
        glAttachShader(m_RendererID, fragmentShader);

        glLinkProgram(m_RendererID);

        GLint isLinked = 0;
        glGetProgramiv(m_RendererID, GL_LINK_STATUS, (int*)&isLinked);
        if (isLinked == GL_FALSE) {
            GLint maxLength = 0;
            glGetProgramiv(m_RendererID, GL_INFO_LOG_LENGTH, &maxLength);

            std::vector<GLchar> infoLog(maxLength);
            glGetProgramInfoLog(m_RendererID, maxLength, &maxLength, &infoLog[0]);
            
            glDeleteProgram(m_RendererID);
            glDeleteShader(vertexShader);
            glDeleteShader(fragmentShader);

            VOLTRA_CORE_ERROR("{0}", infoLog.data());
            VOLTRA_CORE_FATAL("Shader link failure!");
            return;
        }

        glDetachShader(m_RendererID, vertexShader);
        glDetachShader(m_RendererID, fragmentShader);
    }

    /**
     * @brief Destructor. Deletes the OpenGL program.
     */
    Shader::~Shader() {
        glDeleteProgram(m_RendererID);
    }

    /**
     * @brief Activates the shader program.
     */
    void Shader::Bind() const {
        glUseProgram(m_RendererID);
    }

    /**
     * @brief Deactivates the shader program.
     */
    void Shader::Unbind() const {
        glUseProgram(0);
    }

    /**
     * @brief Helper to find uniform location.
     * 
     * @param programID Shader program ID.
     * @param name Uniform name.
     * @return Location index or -1 if not found.
     */
    GLint GetUniformLocation(uint32_t programID, const std::string& name) {
        GLint location = glGetUniformLocation(programID, name.c_str());
        if (location == -1) {
            VOLTRA_CORE_WARN("Warning: Uniform '{0}' doesn't exist!", name);
        }
        return location;
    }

    /**
     * @brief Uploads an int uniform.
     * @param name Uniform name.
     * @param value value.
     */
    void Shader::UploadUniformInt(const std::string& name, int value) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniform1i(location, value);
    }

    /**
     * @brief Uploads a float uniform.
     * @param name Uniform name.
     * @param value value.
     */
    void Shader::UploadUniformFloat(const std::string& name, float value) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniform1f(location, value);
    }

    /**
     * @brief Uploads a vec2 uniform.
     * @param name Uniform name.
     * @param value value.
     */
    void Shader::UploadUniformFloat2(const std::string& name, const glm::vec2& value) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniform2f(location, value.x, value.y);
    }

    /**
     * @brief Uploads a vec3 uniform.
     * @param name Uniform name.
     * @param value value.
     */
    void Shader::UploadUniformFloat3(const std::string& name, const glm::vec3& value) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniform3f(location, value.x, value.y, value.z);
    }

    /**
     * @brief Uploads a vec4 uniform.
     * @param name Uniform name.
     * @param value value.
     */
    void Shader::UploadUniformFloat4(const std::string& name, const glm::vec4& value) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniform4f(location, value.x, value.y, value.z, value.w);
    }

    /**
     * @brief Uploads a mat3 uniform.
     * @param name Uniform name.
     * @param matrix value.
     */
    void Shader::UploadUniformMat3(const std::string& name, const glm::mat3& matrix) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniformMatrix3fv(location, 1, GL_FALSE, glm::value_ptr(matrix));
    }

    /**
     * @brief Uploads a mat4 uniform.
     * @param name Uniform name.
     * @param matrix value.
     */
    void Shader::UploadUniformMat4(const std::string& name, const glm::mat4& matrix) {
        GLint location = GetUniformLocation(m_RendererID, name);
        glUniformMatrix4fv(location, 1, GL_FALSE, glm::value_ptr(matrix));
    }

}
