#include <gtest/gtest.h>
#include "Scene/Scene.hpp"
#include "Scene/Entity.hpp"
#include "Scene/Components.hpp"

class TestScript : public Voltra::ScriptableEntity {
public:
    static bool Updated;
    void OnUpdate(Voltra::Timestep ts) override { Updated = true; }
};
bool TestScript::Updated = false;

TEST(ECSTest, NativeScripting) {
    Voltra::Scene scene;
    Voltra::Entity entity = scene.CreateEntity();
    entity.AddComponent<Voltra::NativeScriptComponent>().Bind<TestScript>();

    scene.OnUpdate(Voltra::Timestep(0.016f));

    EXPECT_TRUE(TestScript::Updated);
}

TEST(ECSTest, CreateEntity) {
    Voltra::Scene scene;
    Voltra::Entity entity = scene.CreateEntity("TestEntity");

    EXPECT_TRUE(entity.HasComponent<Voltra::TagComponent>());
    EXPECT_TRUE(entity.HasComponent<Voltra::TransformComponent>());
    
    auto& tag = entity.GetComponent<Voltra::TagComponent>();
    EXPECT_EQ(tag.Tag, "TestEntity");
}

TEST(ECSTest, AddRemoveComponent) {
    Voltra::Scene scene;
    Voltra::Entity entity = scene.CreateEntity();

    // Transform is added by default in CreateEntity
    EXPECT_TRUE(entity.HasComponent<Voltra::TransformComponent>());

    entity.RemoveComponent<Voltra::TransformComponent>();
    EXPECT_FALSE(entity.HasComponent<Voltra::TransformComponent>());
}