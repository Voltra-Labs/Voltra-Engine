#include "OrthographicCamera.hpp"
#include <glm/gtc/matrix_transform.hpp>

namespace Voltra {

    /**
     * @brief Constructs an OrthographicCamera.
     * 
     * @param left Left bound.
     * @param right Right bound.
     * @param bottom Bottom bound.
     * @param top Top bound.
     */
    OrthographicCamera::OrthographicCamera(float left, float right, float bottom, float top)
        : m_ProjectionMatrix(glm::ortho(left, right, bottom, top, -1.0f, 1.0f)), m_ViewMatrix(1.0f) {
        m_ViewProjectionMatrix = m_ProjectionMatrix * m_ViewMatrix;
    }

    /**
     * @brief Updates the projection matrix.
     * 
     * @param left Left bound.
     * @param right Right bound.
     * @param bottom Bottom bound.
     * @param top Top bound.
     */
    void OrthographicCamera::SetProjection(float left, float right, float bottom, float top) {
        m_ProjectionMatrix = glm::ortho(left, right, bottom, top, -1.0f, 1.0f);
        m_ViewProjectionMatrix = m_ProjectionMatrix * m_ViewMatrix;
    }

    /**
     * @brief Recalculates the view matrix based on position and rotation.
     * 
     * Called whenever position or rotation changes.
     */
    void OrthographicCamera::RecalculateViewMatrix() {
        glm::mat4 transform = glm::translate(glm::mat4(1.0f), m_Position) *
                              glm::rotate(glm::mat4(1.0f), glm::radians(m_Rotation), glm::vec3(0, 0, 1));

        m_ViewMatrix = glm::inverse(transform);
        m_ViewProjectionMatrix = m_ProjectionMatrix * m_ViewMatrix;
    }

}