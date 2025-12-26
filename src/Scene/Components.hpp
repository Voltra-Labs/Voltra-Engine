#pragma once

#include "Renderer/Texture.hpp"
#include <memory>
#include "Renderer/OrthographicCamera.hpp"
#include <glm/glm.hpp>
#include <string>
#include "ScriptableEntity.hpp"

namespace Voltra {

    struct TagComponent {
        std::string Tag;

        TagComponent() = default;
        TagComponent(const TagComponent&) = default;
        TagComponent(const std::string& tag) : Tag(tag) {}
    };

    struct TransformComponent {
        glm::mat4 Transform{ 1.0f };

        TransformComponent() = default;
        TransformComponent(const TransformComponent&) = default;
        TransformComponent(const glm::mat4& transform) : Transform(transform) {}

        operator glm::mat4& () { return Transform; }
        operator const glm::mat4& () const { return Transform; }
    };

    struct NativeScriptComponent {
        ScriptableEntity* Instance = nullptr;

        ScriptableEntity* (*InstantiateScript)();
        void (*DestroyScript)(NativeScriptComponent*);

        template<typename T>
        void Bind() {
            InstantiateScript = []() { return static_cast<ScriptableEntity*>(new T()); };
            DestroyScript = [](NativeScriptComponent* nsc) { delete nsc->Instance; nsc->Instance = nullptr; };
        }
    };

    // Define the color (and future support for textures)
    struct SpriteRendererComponent {
        glm::vec4 Color{ 1.0f, 1.0f, 1.0f, 1.0f };
        std::shared_ptr<Texture2D> Texture;
        float TilingFactor = 1.0f;

        SpriteRendererComponent() = default;
        SpriteRendererComponent(const SpriteRendererComponent&) = default;
        
        // Color constructor
        SpriteRendererComponent(const glm::vec4& color) : Color(color) {}
        
        // Texture constructor
        SpriteRendererComponent(const std::shared_ptr<Texture2D>& texture) 
            : Texture(texture), Color(1.0f) {}
    };

    // Allows an entity to act as a camera
    struct CameraComponent {
        Voltra::OrthographicCamera Camera;
        bool Primary = true; // If there are multiple cameras, is this the main one?
        bool FixedAspectRatio = false;

        // Constructor by default initializing a basic orthographic camera (-1 to 1)
        CameraComponent() 
            : Camera(-1.0f, 1.0f, -1.0f, 1.0f) {} // Default aspect ratio 1.0
            
        operator Voltra::OrthographicCamera& () { return Camera; }
        operator const Voltra::OrthographicCamera& () const { return Camera; }
    };

    // Physics

    struct Rigidbody2DComponent {
        enum class BodyType { Static = 0, Kinematic, Dynamic };
        BodyType Type = BodyType::Static;
        bool FixedRotation = false;

        // Storage for runtime
        void* RuntimeBody = nullptr;

        Rigidbody2DComponent() = default;
        Rigidbody2DComponent(const Rigidbody2DComponent&) = default;
    };

    struct BoxCollider2DComponent {
        glm::vec2 Offset = { 0.0f, 0.0f };
        glm::vec2 Size = { 0.5f, 0.5f };

        // Physics material
        float Density = 1.0f;
        float Friction = 0.5f;
        float Restitution = 0.0f;
        float RestitutionThreshold = 0.5f;

        // Storage for runtime
        void* RuntimeFixture = nullptr;

        BoxCollider2DComponent() = default;
        BoxCollider2DComponent(const BoxCollider2DComponent&) = default;
    };

}