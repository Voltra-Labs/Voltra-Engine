#pragma once

#include "Renderer/Texture.hpp"
#include "Core/UUID.hpp"
#include <memory>
#include "Renderer/OrthographicCamera.hpp"
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include <string>
#include "ScriptableEntity.hpp"

namespace Voltra {



    /**
     * @brief Component for identifying entities with a unique ID.
     */
    struct IDComponent {
        UUID ID;

        IDComponent() = default;
        IDComponent(const IDComponent&) = default;
        IDComponent(const UUID& uuid) : ID(uuid) {}
    };

    /**
     * @brief Component for identifying entities with a name.
     */
    struct TagComponent {
        std::string Tag;

        TagComponent() = default;
        TagComponent(const TagComponent&) = default;
        TagComponent(const std::string& tag) : Tag(tag) {}
    };

    /**
     * @brief Component describing the position, rotation, and scale.
     */
    struct TransformComponent {
        glm::vec3 Translation = { 0.0f, 0.0f, 0.0f };
        glm::vec3 Rotation = { 0.0f, 0.0f, 0.0f };
        glm::vec3 Scale = { 1.0f, 1.0f, 1.0f };

        TransformComponent() = default;
        TransformComponent(const TransformComponent&) = default;
        TransformComponent(const glm::vec3& translation) : Translation(translation) {}

        /**
         * @brief Calculates the transformation matrix.
         * 
         * @return The 4x4 model matrix.
         */
        glm::mat4 GetTransform() const {
             glm::mat4 rotation = glm::rotate(glm::mat4(1.0f), Rotation.x, { 1, 0, 0 })
                * glm::rotate(glm::mat4(1.0f), Rotation.y, { 0, 1, 0 })
                * glm::rotate(glm::mat4(1.0f), Rotation.z, { 0, 0, 1 });

            return glm::translate(glm::mat4(1.0f), Translation)
                * rotation
                * glm::scale(glm::mat4(1.0f), Scale);
        }
    };

    /**
     * @brief Component for attaching C++ script logic to an entity.
     */
    struct NativeScriptComponent {
        ScriptableEntity* Instance = nullptr;

        ScriptableEntity* (*InstantiateScript)();
        void (*DestroyScript)(NativeScriptComponent*);

        /**
         * @brief Binds a class T to this component.
         * 
         * T must inherit from ScriptableEntity.
         * 
         * @tparam T The script class type.
         */
        template<typename T>
        void Bind() {
            InstantiateScript = []() { return static_cast<ScriptableEntity*>(new T()); };
            DestroyScript = [](NativeScriptComponent* nsc) { delete nsc->Instance; nsc->Instance = nullptr; };
        }
    };

    /**
     * @brief Component for rendering a sprite (color or texture).
     */
    struct SpriteRendererComponent {
        glm::vec4 Color{ 1.0f, 1.0f, 1.0f, 1.0f };
        std::shared_ptr<Texture2D> Texture; 
        float TilingFactor = 1.0f;          

        SpriteRendererComponent() = default;
        SpriteRendererComponent(const SpriteRendererComponent&) = default;
        
        SpriteRendererComponent(const glm::vec4& color) : Color(color) {}

        SpriteRendererComponent(const std::shared_ptr<Texture2D>& texture) 
            : Texture(texture), Color(1.0f) {}
    };

    /**
     * @brief Component for camera properties.
     */
    struct CameraComponent {
        Voltra::OrthographicCamera Camera;
        bool Primary = true; 
        bool FixedAspectRatio = false;

        float OrthographicSize = 10.0f;
        float OrthographicNear = -1.0f;
        float OrthographicFar = 1.0f;

        CameraComponent() 
            : Camera(-1.0f, 1.0f, -1.0f, 1.0f) {} 
            
        operator Voltra::OrthographicCamera& () { return Camera; }
        operator const Voltra::OrthographicCamera& () const { return Camera; }
    };

    /**
     * @brief Component for 2D rigid body physics (Box2D wrapper).
     */
    struct Rigidbody2DComponent {
        enum class BodyType { Static = 0, Kinematic, Dynamic };
        BodyType Type = BodyType::Static;
        bool FixedRotation = false;

        void* RuntimeBody = nullptr;

        Rigidbody2DComponent() = default;
        Rigidbody2DComponent(const Rigidbody2DComponent&) = default;
    };

    /**
     * @brief Component for 2D box collider (Box2D fixture).
     */
    struct BoxCollider2DComponent {
        glm::vec2 Offset = { 0.0f, 0.0f };
        glm::vec2 Size = { 0.5f, 0.5f };

        float Density = 1.0f;
        float Friction = 0.5f;
        float Restitution = 0.0f;
        float RestitutionThreshold = 0.5f;

        void* RuntimeFixture = nullptr;

        BoxCollider2DComponent() = default;
        BoxCollider2DComponent(const BoxCollider2DComponent&) = default;
    };

}