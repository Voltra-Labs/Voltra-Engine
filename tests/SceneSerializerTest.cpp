#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>

#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include "Scene/Components.hpp"
#include "Scene/SceneSerializer.hpp"

using namespace Voltra;

class SceneSerializerTest : public ::testing::Test {
protected:
    std::string filepath = "test_scene_serialization.voltra";

    void TearDown() override {
        // Limpieza: Borrar el archivo creado tras cada test
        if (std::filesystem::exists(filepath)) {
            std::filesystem::remove(filepath);
        }
    }
};

TEST_F(SceneSerializerTest, SerializeAndDeserializeComplexEntity) {
    // SETUP: Create scene and entity with specific data
    auto sceneSource = std::make_shared<Scene>();
    Entity originalEntity = sceneSource->CreateEntity("Player");
    
    // Transform modified
    auto& tc = originalEntity.GetComponent<TransformComponent>();
    tc.Translation = { 10.0f, 5.0f, -1.0f };
    tc.Rotation = { 0.0f, 0.0f, 1.57f };
    tc.Scale = { 2.0f, 2.0f, 1.0f };

    // Sprite Renderer
    originalEntity.AddComponent<SpriteRendererComponent>(glm::vec4{ 1.0f, 0.0f, 0.0f, 1.0f });

    // Rigidbody Dynamic
    auto& rb = originalEntity.AddComponent<Rigidbody2DComponent>();
    rb.Type = Rigidbody2DComponent::BodyType::Dynamic;
    rb.FixedRotation = true;

    // Box Collider
    auto& bc = originalEntity.AddComponent<BoxCollider2DComponent>();
    bc.Size = { 5.0f, 5.0f };
    bc.Density = 0.8f;

    // ACT: Serialize to disk
    SceneSerializer serializer(sceneSource);
    serializer.Serialize(filepath);

    // ACT: Deserialize into a NEW scene
    auto sceneDest = std::make_shared<Scene>();
    SceneSerializer deserializer(sceneDest);
    bool success = deserializer.Deserialize(filepath);

    // ASSERT: Check results
    ASSERT_TRUE(success) << "La deserialización falló.";

    // Search for the entity in the new scene (iterating, since we don't have a public GetEntityByTag)
    Entity loadedEntity;
    bool found = false;
    sceneDest->GetRegistry().view<TagComponent>().each([&](auto entityID, auto& tagComp) {
        if (tagComp.Tag == "Player") {
            loadedEntity = Entity(entityID, sceneDest.get());
            found = true;
        }
    });

    ASSERT_TRUE(found) << "No se encontró la entidad 'Player' en la escena cargada.";

    // ASSERT: Check Transform
    auto& loadedTC = loadedEntity.GetComponent<TransformComponent>();
    EXPECT_FLOAT_EQ(loadedTC.Translation.x, 10.0f);
    EXPECT_FLOAT_EQ(loadedTC.Translation.y, 5.0f);
    EXPECT_FLOAT_EQ(loadedTC.Rotation.z, 1.57f);

    // ASSERT: Check extra components
    EXPECT_TRUE(loadedEntity.HasComponent<SpriteRendererComponent>());
    EXPECT_TRUE(loadedEntity.HasComponent<Rigidbody2DComponent>());
    EXPECT_TRUE(loadedEntity.HasComponent<BoxCollider2DComponent>());

    // ASSERT: Check physics values
    auto& loadedRB = loadedEntity.GetComponent<Rigidbody2DComponent>();
    EXPECT_EQ(loadedRB.Type, Rigidbody2DComponent::BodyType::Dynamic);
    EXPECT_TRUE(loadedRB.FixedRotation);

    auto& loadedBC = loadedEntity.GetComponent<BoxCollider2DComponent>();
    EXPECT_FLOAT_EQ(loadedBC.Size.x, 5.0f);
    EXPECT_FLOAT_EQ(loadedBC.Density, 0.8f);
}