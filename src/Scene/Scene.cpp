#include "Scene.hpp"
#include "Entity.hpp"
#include "Components.hpp"
#include "Renderer/Renderer.hpp"
#include <glm/glm.hpp>
#include "Renderer/Renderer2D.hpp"

namespace Voltra {

    Scene::Scene() {
    }

    Scene::~Scene() {
    }

    Entity Scene::CreateEntity(const std::string& name) {
        Entity entity = { m_Registry.create(), this };
        entity.AddComponent<TransformComponent>();
        auto& tag = entity.AddComponent<TagComponent>();
        tag.Tag = name.empty() ? "Entity" : name;
        return entity;
    }

    void Scene::DestroyEntity(Entity entity) {
        m_Registry.destroy(entity);
    }

    void Scene::OnUpdate(Timestep ts) {
        // Scripts (Logic)
        m_Registry.view<NativeScriptComponent>().each([=](auto entity, auto& nsc) {
            if (!nsc.Instance) {
                nsc.Instance = nsc.InstantiateScript();
                nsc.Instance->m_Entity = Entity{ entity, this };
                nsc.Instance->OnCreate();
            }
            nsc.Instance->OnUpdate(ts);
        });

        // Rendering
        CameraComponent* mainCamera = nullptr;
        glm::mat4 cameraTransform;

        auto view = m_Registry.view<TransformComponent, CameraComponent>();
        for (auto entity : view) {
            auto [transform, camera] = view.get<TransformComponent, CameraComponent>(entity);
            if (camera.Primary) {
                mainCamera = &camera;
                cameraTransform = transform.Transform;
                break;
            }
        }

        if (mainCamera) {
            // Synchronize camera position
            glm::vec3 pos = glm::vec3(cameraTransform[3]);
            mainCamera->Camera.SetPosition(pos);
            
            Renderer2D::BeginScene(mainCamera->Camera);

            // Draw all entities with SpriteRenderer and Transform
            auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
            for (auto entity : group) {
                auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
                
                // If it has a texture, we use the texture overload
                if (sprite.Texture) {
                    // TODO: DrawQuad(transform, texture, tint)
                    Renderer2D::DrawQuad(transform.Transform, sprite.Texture);
                } 
                else {
                    Renderer2D::DrawQuad(transform.Transform, sprite.Color);
                }
            }

            Renderer2D::EndScene();
        }
    }

}