#include "Scene/SceneSerializer.hpp"

#include "Scene/Entity.hpp"
#include "Scene/Components.hpp"
#include "Renderer/AssetManager.hpp"


#include <yaml-cpp/yaml.h>
#include <fstream>

namespace YAML {

    template<>
    struct convert<glm::vec2> {
        static Node encode(const glm::vec2& rhs) {
            Node node;
            node.push_back(rhs.x);
            node.push_back(rhs.y);
            node.SetStyle(EmitterStyle::Flow);
            return node;
        }

        static bool decode(const Node& node, glm::vec2& rhs) {
            if (!node.IsSequence() || node.size() != 2)
                return false;

            rhs.x = node[0].as<float>();
            rhs.y = node[1].as<float>();
            return true;
        }
    };

    template<>
    struct convert<glm::vec3> {
        static Node encode(const glm::vec3& rhs) {
            Node node;
            node.push_back(rhs.x);
            node.push_back(rhs.y);
            node.push_back(rhs.z);
            node.SetStyle(EmitterStyle::Flow);
            return node;
        }

        static bool decode(const Node& node, glm::vec3& rhs) {
            if (!node.IsSequence() || node.size() != 3)
                return false;

            rhs.x = node[0].as<float>();
            rhs.y = node[1].as<float>();
            rhs.z = node[2].as<float>();
            return true;
        }
    };

    template<>
    struct convert<glm::vec4> {
        static Node encode(const glm::vec4& rhs) {
            Node node;
            node.push_back(rhs.x);
            node.push_back(rhs.y);
            node.push_back(rhs.z);
            node.push_back(rhs.w);
            node.SetStyle(EmitterStyle::Flow);
            return node;
        }

        static bool decode(const Node& node, glm::vec4& rhs) {
            if (!node.IsSequence() || node.size() != 4)
                return false;

            rhs.x = node[0].as<float>();
            rhs.y = node[1].as<float>();
            rhs.z = node[2].as<float>();
            rhs.w = node[3].as<float>();
            return true;
        }
    };
}

namespace Voltra {

    YAML::Emitter& operator<<(YAML::Emitter& out, const glm::vec2& v) {
        out << YAML::Flow;
        out << YAML::BeginSeq << v.x << v.y << YAML::EndSeq;
        return out;
    }

    YAML::Emitter& operator<<(YAML::Emitter& out, const glm::vec3& v) {
        out << YAML::Flow;
        out << YAML::BeginSeq << v.x << v.y << v.z << YAML::EndSeq;
        return out;
    }

    YAML::Emitter& operator<<(YAML::Emitter& out, const glm::vec4& v) {
        out << YAML::Flow;
        out << YAML::BeginSeq << v.x << v.y << v.z << v.w << YAML::EndSeq;
        return out;
    }

    /**
     * @brief Serializer Constructor.
     * 
     * @param scene The scene context to operate on.
     */
    SceneSerializer::SceneSerializer(const std::shared_ptr<Scene>& scene)
        : m_Scene(scene) {
    }

    /**
     * @brief Helper to serialize a single entity and its components.
     * 
     * @param out The YAML emitter.
     * @param entity The entity handle.
     */
    static void SerializeEntity(YAML::Emitter& out, Entity entity) {
        out << YAML::BeginMap; // Entity
        out << YAML::Key << "Entity" << YAML::Value << entity.GetComponent<IDComponent>().ID;

        if (entity.HasComponent<TagComponent>()) {
            out << YAML::Key << "TagComponent";
            out << YAML::BeginMap;

            auto& tag = entity.GetComponent<TagComponent>().Tag;
            out << YAML::Key << "Tag" << YAML::Value << tag;

            out << YAML::EndMap;
        }

        if (entity.HasComponent<TransformComponent>()) {
            out << YAML::Key << "TransformComponent";
            out << YAML::BeginMap;

            auto& tc = entity.GetComponent<TransformComponent>();
            out << YAML::Key << "Translation" << YAML::Value << tc.Translation;
            out << YAML::Key << "Rotation" << YAML::Value << tc.Rotation;
            out << YAML::Key << "Scale" << YAML::Value << tc.Scale;

            out << YAML::EndMap;
        }

        if (entity.HasComponent<CameraComponent>()) {
            out << YAML::Key << "CameraComponent";
            out << YAML::BeginMap;

            auto& cameraComponent = entity.GetComponent<CameraComponent>();
            auto& camera = cameraComponent.Camera;

            out << YAML::Key << "Camera" << YAML::Value;
            out << YAML::BeginMap;
            out << YAML::Key << "ProjectionType" << YAML::Value << 1;
            out << YAML::Key << "OrthographicSize" << YAML::Value << cameraComponent.OrthographicSize;
            out << YAML::Key << "OrthographicNear" << YAML::Value << cameraComponent.OrthographicNear;
            out << YAML::Key << "OrthographicFar" << YAML::Value << cameraComponent.OrthographicFar;
            out << YAML::EndMap;

            out << YAML::Key << "Primary" << YAML::Value << cameraComponent.Primary;
            out << YAML::Key << "FixedAspectRatio" << YAML::Value << cameraComponent.FixedAspectRatio;

            out << YAML::EndMap;
        }

        if (entity.HasComponent<SpriteRendererComponent>()) {
            out << YAML::Key << "SpriteRendererComponent";
            out << YAML::BeginMap;

            auto& spriteRendererComponent = entity.GetComponent<SpriteRendererComponent>();
            out << YAML::Key << "Color" << YAML::Value << spriteRendererComponent.Color;
            
            if (spriteRendererComponent.Texture)
                out << YAML::Key << "TexturePath" << YAML::Value << spriteRendererComponent.Texture->GetPath();
            
            out << YAML::Key << "TilingFactor" << YAML::Value << spriteRendererComponent.TilingFactor;

            out << YAML::EndMap;
        }

        if (entity.HasComponent<Rigidbody2DComponent>()) {
            out << YAML::Key << "Rigidbody2DComponent";
            out << YAML::BeginMap;

            auto& rb2dComponent = entity.GetComponent<Rigidbody2DComponent>();
            out << YAML::Key << "BodyType" << YAML::Value << (int)rb2dComponent.Type;
            out << YAML::Key << "FixedRotation" << YAML::Value << rb2dComponent.FixedRotation;

            out << YAML::EndMap;
        }

        if (entity.HasComponent<BoxCollider2DComponent>()) {
            out << YAML::Key << "BoxCollider2DComponent";
            out << YAML::BeginMap;

            auto& bc2dComponent = entity.GetComponent<BoxCollider2DComponent>();
            out << YAML::Key << "Offset" << YAML::Value << bc2dComponent.Offset;
            out << YAML::Key << "Size" << YAML::Value << bc2dComponent.Size;
            out << YAML::Key << "Density" << YAML::Value << bc2dComponent.Density;
            out << YAML::Key << "Friction" << YAML::Value << bc2dComponent.Friction;
            out << YAML::Key << "Restitution" << YAML::Value << bc2dComponent.Restitution;
            out << YAML::Key << "RestitutionThreshold" << YAML::Value << bc2dComponent.RestitutionThreshold;

            out << YAML::EndMap;
        }

        out << YAML::EndMap;
    }

    /**
     * @brief Serializes the entire scene to a .voltra file.
     * 
     * @param filepath Path to write.
     */
    void SceneSerializer::Serialize(const std::string& filepath) {
        YAML::Emitter out;
        out << YAML::BeginMap;
        out << YAML::Key << "Scene" << YAML::Value << "Untitled";
        out << YAML::Key << "Entities" << YAML::Value << YAML::BeginSeq;

        auto view = m_Scene->m_Registry.view<TagComponent>();
        for (auto entityID : view) {
            Entity entity = { entityID, m_Scene.get() };
            if (!entity)
                continue;

            SerializeEntity(out, entity);
        }

        out << YAML::EndSeq;
        out << YAML::EndMap;

        std::ofstream fout(filepath);
        fout << out.c_str();
    }

    /**
     * @brief Defines runtime serialization (Binary format - Future implementation).
     * 
     * @param filepath Path.
     */
    void SceneSerializer::SerializeRuntime(const std::string& filepath) {
    }

    /**
     * @brief Deserializes a scene from a .voltra file.
     * 
     * Reconstructs entities and components from YAML.
     * 
     * @param filepath Path to read.
     * @return true if successful, false if file is invalid.
     */
    bool SceneSerializer::Deserialize(const std::string& filepath) {
        std::ifstream stream(filepath);
        std::stringstream strStream;
        strStream << stream.rdbuf();

        YAML::Node data = YAML::Load(strStream.str());
        if (!data["Scene"])
            return false;

        std::string sceneName = data["Scene"].as<std::string>();
        VOLTRA_CORE_TRACE("Deserializing scene '{0}'", sceneName);

        auto entities = data["Entities"];
        if (entities) {
            for (auto entity : entities) {
                uint64_t uuid = entity["Entity"].as<uint64_t>();

                std::string name;
                auto tagComponent = entity["TagComponent"];
                if (tagComponent)
                    name = tagComponent["Tag"].as<std::string>();

                Entity deserializedEntity = m_Scene->CreateEntity(name, uuid);

                auto transformComponent = entity["TransformComponent"];
                if (transformComponent) {
                    auto& tc = deserializedEntity.GetComponent<TransformComponent>();
                    tc.Translation = transformComponent["Translation"].as<glm::vec3>();
                    tc.Rotation = transformComponent["Rotation"].as<glm::vec3>();
                    tc.Scale = transformComponent["Scale"].as<glm::vec3>();
                }

                auto cameraComponent = entity["CameraComponent"];
                if (cameraComponent) {
                    auto& cc = deserializedEntity.AddComponent<CameraComponent>();

                    auto cameraProps = cameraComponent["Camera"];
                    cc.OrthographicSize = cameraProps["OrthographicSize"].as<float>();
                    cc.OrthographicNear = cameraProps["OrthographicNear"].as<float>();
                    cc.OrthographicFar = cameraProps["OrthographicFar"].as<float>();

                    cc.Camera.SetProjection(-cc.OrthographicSize * 0.5f, cc.OrthographicSize * 0.5f, -cc.OrthographicSize * 0.5f, cc.OrthographicSize * 0.5f);

                    cc.Primary = cameraComponent["Primary"].as<bool>();
                    cc.FixedAspectRatio = cameraComponent["FixedAspectRatio"].as<bool>();
                }

                auto spriteRendererComponent = entity["SpriteRendererComponent"];
                if (spriteRendererComponent) {
                auto& src = deserializedEntity.AddComponent<SpriteRendererComponent>();
                    src.Color = spriteRendererComponent["Color"].as<glm::vec4>();
                    
                    if (spriteRendererComponent["TexturePath"]) {
                        std::string texturePath = spriteRendererComponent["TexturePath"].as<std::string>();
                        src.Texture = AssetManager::LoadTexture(texturePath);
                    }

                    if (spriteRendererComponent["TilingFactor"])
                        src.TilingFactor = spriteRendererComponent["TilingFactor"].as<float>();
                }

                auto rigidbody2DComponent = entity["Rigidbody2DComponent"];
                if (rigidbody2DComponent) {
                    auto& rb2d = deserializedEntity.AddComponent<Rigidbody2DComponent>();
                    rb2d.Type = (Rigidbody2DComponent::BodyType)rigidbody2DComponent["BodyType"].as<int>();
                    rb2d.FixedRotation = rigidbody2DComponent["FixedRotation"].as<bool>();
                }

                auto boxCollider2DComponent = entity["BoxCollider2DComponent"];
                if (boxCollider2DComponent) {
                    auto& bc2d = deserializedEntity.AddComponent<BoxCollider2DComponent>();
                    bc2d.Offset = boxCollider2DComponent["Offset"].as<glm::vec2>();
                    bc2d.Size = boxCollider2DComponent["Size"].as<glm::vec2>();
                    bc2d.Density = boxCollider2DComponent["Density"].as<float>();
                    bc2d.Friction = boxCollider2DComponent["Friction"].as<float>();
                    bc2d.Restitution = boxCollider2DComponent["Restitution"].as<float>();
                    bc2d.RestitutionThreshold = boxCollider2DComponent["RestitutionThreshold"].as<float>();
                }
            }
        }

        return true;
    }

    /**
     * @brief Deserializes runtime binary data.
     * 
     * @param filepath Path.
     * @return Always false (Not Implemented).
     */
    bool SceneSerializer::DeserializeRuntime(const std::string& filepath) {
        return false;
    }

}
