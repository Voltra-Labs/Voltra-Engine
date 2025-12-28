#include "Scene.hpp"
#include "Entity.hpp"
#include "Components.hpp"
#include "Renderer/Renderer.hpp"
#include <glm/glm.hpp>
#include <glm/gtc/matrix_transform.hpp>
#include "Renderer/Renderer2D.hpp"

#include <box2d/box2d.h>

#include "Renderer/EditorCamera.hpp"


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

    Entity Scene::GetPrimaryCameraEntity() {
        auto view = m_Registry.view<CameraComponent>();
        for (auto entity : view) {
            const auto& camera = view.get<CameraComponent>(entity);
            if (camera.Primary)
                return Entity{ entity, this };
        }
        return {};
    }

    static b2BodyType VoltraBodyTypeToBox2D(Rigidbody2DComponent::BodyType bodyType) {
        switch (bodyType) {
            case Rigidbody2DComponent::BodyType::Static:    return b2_staticBody;
            case Rigidbody2DComponent::BodyType::Dynamic:   return b2_dynamicBody;
            case Rigidbody2DComponent::BodyType::Kinematic: return b2_kinematicBody;
        }
        return b2_staticBody;
    }

    void Scene::OnRuntimeStart() {
        m_PhysicsWorld = new b2World({ 0.0f, -9.8f });

        auto view = m_Registry.view<Rigidbody2DComponent>();
        for (auto e : view) {
            Entity entity = { e, this };
            auto& transform = entity.GetComponent<TransformComponent>();
            auto& rb = entity.GetComponent<Rigidbody2DComponent>();

            b2BodyDef bodyDef;
            bodyDef.type = VoltraBodyTypeToBox2D(rb.Type);
            bodyDef.position.Set(transform.Translation.x, transform.Translation.y);
            bodyDef.angle = transform.Rotation.z;

            b2Body* body = m_PhysicsWorld->CreateBody(&bodyDef);
            body->SetFixedRotation(rb.FixedRotation);
            rb.RuntimeBody = body;

            if (entity.HasComponent<BoxCollider2DComponent>()) {
                auto& bc = entity.GetComponent<BoxCollider2DComponent>();

                b2PolygonShape boxShape;
                boxShape.SetAsBox(bc.Size.x * transform.Scale.x, bc.Size.y * transform.Scale.y, b2Vec2(bc.Offset.x, bc.Offset.y), 0.0f); 
                
                b2FixtureDef fixtureDef;
                fixtureDef.shape = &boxShape;
                fixtureDef.density = bc.Density;
                fixtureDef.friction = bc.Friction;
                fixtureDef.restitution = bc.Restitution;
                fixtureDef.restitutionThreshold = bc.RestitutionThreshold;
                
                body->CreateFixture(&fixtureDef);
            }
        }
    }

    void Scene::OnRuntimeStop() {
        delete m_PhysicsWorld;
        m_PhysicsWorld = nullptr;
    }

    void Scene::OnUpdate(Timestep ts) {
        // Update Physics
        const int32_t velocityIterations = 6;
        const int32_t positionIterations = 2;
        if (m_PhysicsWorld) {
            m_PhysicsWorld->Step(ts, velocityIterations, positionIterations);

            // Sync back to Transform
            auto view = m_Registry.view<Rigidbody2DComponent>();
            for (auto e : view) {
                Entity entity = { e, this };
                auto& rb = entity.GetComponent<Rigidbody2DComponent>();
                
                if (rb.RuntimeBody) {
                    b2Body* body = (b2Body*)rb.RuntimeBody;
                    const auto& position = body->GetPosition();
                    auto& transform = entity.GetComponent<TransformComponent>();
                    transform.Translation.x = position.x;
                    transform.Translation.y = position.y;
                    transform.Rotation.z = body->GetAngle();
                }
            }
        }

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
                cameraTransform = transform.GetTransform();
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
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Texture);
                } 
                else {
                    Renderer2D::DrawQuad(transform.GetTransform(), sprite.Color);
                }
            }

            Renderer2D::EndScene();
        }
    }

    void Scene::OnUpdateEditor(Timestep ts, EditorCamera& camera) {
        // Rendering
        Renderer2D::BeginScene(camera);

        // Draw all entities with SpriteRenderer and Transform
        auto group = m_Registry.group<TransformComponent>(entt::get<SpriteRendererComponent>);
        for (auto entity : group) {
            auto [transform, sprite] = group.get<TransformComponent, SpriteRendererComponent>(entity);
            
            // If it has a texture, we use the texture overload
            if (sprite.Texture) {
                Renderer2D::DrawQuad(transform.GetTransform(), sprite.Texture);
            } 
            else {
                Renderer2D::DrawQuad(transform.GetTransform(), sprite.Color);
            }
        }

        Renderer2D::EndScene();
    }

}
