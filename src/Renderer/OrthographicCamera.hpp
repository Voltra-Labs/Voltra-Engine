#pragma once

#include <glm/glm.hpp>

namespace Voltra {

    /**
     * @brief A 2D Orthographic Camera.
     */
    class OrthographicCamera {
    public:
        /**
         * @brief Constructs the camera with a specific projection.
         * 
         * @param left Left bound.
         * @param right Right bound.
         * @param bottom Bottom bound.
         * @param top Top bound.
         */
        OrthographicCamera(float left, float right, float bottom, float top);

        /**
         * @brief Sets the projection matrix boundaries.
         * 
         * @param left Left bound.
         * @param right Right bound.
         * @param bottom Bottom bound.
         * @param top Top bound.
         */
        void SetProjection(float left, float right, float bottom, float top);

        /**
         * @brief Sets the camera position.
         * 
         * @param position New position.
         */
        void SetPosition(const glm::vec3& position) { m_Position = position; RecalculateViewMatrix(); }

        /**
         * @brief Gets the camera position.
         * 
         * @return Position vector.
         */
        const glm::vec3& GetPosition() const { return m_Position; }

        /**
         * @brief Sets the camera rotation (Z-axis).
         * 
         * @param rotation Rotation in degrees.
         */
        void SetRotation(float rotation) { m_Rotation = rotation; RecalculateViewMatrix(); }

        /**
         * @brief Gets the camera rotation.
         * 
         * @return Rotation in degrees.
         */
        float GetRotation() const { return m_Rotation; }

        /**
         * @brief Gets the projection matrix.
         * 
         * @return Projection matrix.
         */
        const glm::mat4& GetProjectionMatrix() const { return m_ProjectionMatrix; }

        /**
         * @brief Gets the view matrix.
         * 
         * @return View matrix.
         */
        const glm::mat4& GetViewMatrix() const { return m_ViewMatrix; }

        /**
         * @brief Gets the combined View-Projection matrix.
         * 
         * @return View-Projection matrix.
         */
        const glm::mat4& GetViewProjectionMatrix() const { return m_ViewProjectionMatrix; }

    private:
        void RecalculateViewMatrix();

    private:
        glm::mat4 m_ProjectionMatrix;
        glm::mat4 m_ViewMatrix;
        glm::mat4 m_ViewProjectionMatrix;

        glm::vec3 m_Position = { 0.0f, 0.0f, 0.0f };
        float m_Rotation = 0.0f;
    };

}